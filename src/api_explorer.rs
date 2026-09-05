use crate::{
    features::Feature,
    findings::{Finding, FindingCategory},
    graph::project_graph::ProjectGraph,
    model::{edge::RelationKind, node::NodeKind},
};
use regex::Regex;
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiParameter {
    pub name: String,
    pub location: ApiParameterLocation,
    pub required: bool,
    pub schema_type: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponseMetadata {
    pub status: u16,
    pub content_type: Option<String>,
    pub schema: Option<Value>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub id: String,
    pub project_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub framework: String,
    pub transport: String,
    pub handler_node_id: Option<String>,
    pub source_path: Option<String>,
    pub source_start_line: Option<usize>,
    pub source_end_line: Option<usize>,
    pub auth_requirements: Vec<String>,
    pub parameters: Vec<ApiParameter>,
    pub request_content_types: Vec<String>,
    pub request_schema: Option<Value>,
    pub responses: Vec<ApiResponseMetadata>,
    pub tags: Vec<String>,
    pub feature_ids: Vec<String>,
    pub source_scope: String,
    pub provenance: String,
    pub security_finding_ids: Vec<String>,
    pub quality_finding_ids: Vec<String>,
    pub has_tests: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiCoverage {
    pub total: usize,
    pub by_method: BTreeMap<String, usize>,
    pub documented: usize,
    pub request_schema: usize,
    pub response_schema: usize,
    pub tested: usize,
    pub security_findings: usize,
    pub untested: usize,
}

pub fn discover_api_endpoints(
    project_id: &str,
    graph: &ProjectGraph,
    features: &[Feature],
    findings: &[Finding],
) -> Vec<ApiEndpoint> {
    let route_pattern = Regex::new(r"^(GET|POST|PUT|PATCH|DELETE|OPTIONS|HEAD|ANY)\s+(.+)$")
        .expect("static route regex");
    let path_parameter =
        Regex::new(r"(?:\{([^}/]+)\}|:([A-Za-z_][A-Za-z0-9_]*))").expect("static path regex");
    let mut endpoints = Vec::new();
    for node in graph.nodes.iter().filter(|node| {
        matches!(node.kind, NodeKind::ApiEndpoint | NodeKind::Route)
            && node.source_scope.library_eligible()
    }) {
        let Some(captures) = route_pattern.captures(node.name.trim()) else {
            continue;
        };
        let method = captures[1].to_string();
        let path = normalize_path(&captures[2]);
        let source = node
            .path
            .as_deref()
            .and_then(|path| fs::read_to_string(Path::new(&graph.root).join(path)).ok())
            .unwrap_or_default();
        let handler = graph
            .outgoing_edges(&node.id)
            .into_iter()
            .filter(|edge| {
                matches!(
                    edge.relation,
                    RelationKind::HandledBy
                        | RelationKind::RoutesTo
                        | RelationKind::Calls
                        | RelationKind::Uses
                )
            })
            .filter_map(|edge| graph.find_node(&edge.target_id))
            .find(|target| {
                matches!(
                    target.kind,
                    NodeKind::Handler
                        | NodeKind::Controller
                        | NodeKind::Function
                        | NodeKind::Method
                        | NodeKind::Service
                )
            });
        let mut parameters = path_parameter
            .captures_iter(&path)
            .filter_map(|capture| capture.get(1).or_else(|| capture.get(2)))
            .map(|name| ApiParameter {
                name: name.as_str().into(),
                location: ApiParameterLocation::Path,
                required: true,
                schema_type: "string".into(),
                source: "route".into(),
            })
            .collect::<Vec<_>>();
        parameters.extend(discover_query_parameters(&source));
        parameters.extend(discover_headers(&source));
        parameters.sort_by(|left, right| left.name.cmp(&right.name));
        parameters.dedup_by(|left, right| {
            left.name == right.name
                && std::mem::discriminant(&left.location) == std::mem::discriminant(&right.location)
        });
        let member_ids = features
            .iter()
            .filter(|feature| {
                feature.node_ids.contains(&node.id)
                    || handler.is_some_and(|handler| feature.node_ids.contains(&handler.id))
            })
            .map(|feature| feature.id.clone())
            .collect::<Vec<_>>();
        let related = findings
            .iter()
            .filter(|finding| {
                finding.node_ids.contains(&node.id)
                    || handler.is_some_and(|handler| finding.node_ids.contains(&handler.id))
            })
            .collect::<Vec<_>>();
        let auth = discover_auth(&source);
        let responses = discover_responses(&source);
        let request_content_types = discover_content_types(&source);
        let request_schema = if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
            discover_request_schema(&source)
        } else {
            None
        };
        let tag = path
            .trim_matches('/')
            .split('/')
            .find(|segment| !segment.is_empty() && !segment.starts_with('{'))
            .unwrap_or("root")
            .to_string();
        endpoints.push(ApiEndpoint {
            id: node.id.clone(),
            project_id: project_id.into(),
            method,
            path: path.clone(),
            name: node.name.clone(),
            framework: infer_framework(graph, node.path.as_deref(), &source),
            transport: if node.kind == NodeKind::Route
                && source.to_lowercase().contains("websocket")
            {
                "websocket".into()
            } else if source.to_lowercase().contains("text/event-stream") {
                "sse".into()
            } else {
                "http".into()
            },
            handler_node_id: handler.map(|handler| handler.id.clone()),
            source_path: node.path.clone(),
            source_start_line: node.start_line,
            source_end_line: node.end_line,
            auth_requirements: auth,
            parameters,
            request_content_types,
            request_schema,
            responses,
            tags: vec![tag],
            feature_ids: member_ids,
            source_scope: format!("{:?}", node.source_scope).to_lowercase(),
            provenance: "code".into(),
            security_finding_ids: related
                .iter()
                .filter(|finding| finding.category == FindingCategory::Security)
                .map(|finding| finding.id.clone())
                .collect(),
            quality_finding_ids: related
                .iter()
                .filter(|finding| finding.category != FindingCategory::Security)
                .map(|finding| finding.id.clone())
                .collect(),
            has_tests: handler.is_some_and(|handler| {
                graph.incoming_edges(&handler.id).into_iter().any(|edge| {
                    graph.find_node(&edge.source_id).is_some_and(|caller| {
                        caller.path.as_deref().is_some_and(|path| {
                            let path = path.to_lowercase();
                            path.contains("test") || path.contains("spec")
                        })
                    })
                })
            }),
        });
    }
    endpoints.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.method.cmp(&right.method))
    });
    endpoints.dedup_by(|left, right| left.method == right.method && left.path == right.path);
    endpoints
}

pub fn api_coverage(endpoints: &[ApiEndpoint]) -> ApiCoverage {
    let mut by_method = BTreeMap::new();
    for endpoint in endpoints {
        *by_method.entry(endpoint.method.clone()).or_default() += 1;
    }
    ApiCoverage {
        total: endpoints.len(),
        by_method,
        documented: endpoints
            .iter()
            .filter(|endpoint| !endpoint.responses.is_empty())
            .count(),
        request_schema: endpoints
            .iter()
            .filter(|endpoint| endpoint.request_schema.is_some())
            .count(),
        response_schema: endpoints
            .iter()
            .filter(|endpoint| {
                endpoint
                    .responses
                    .iter()
                    .any(|response| response.schema.is_some())
            })
            .count(),
        tested: endpoints
            .iter()
            .filter(|endpoint| endpoint.has_tests)
            .count(),
        security_findings: endpoints
            .iter()
            .filter(|endpoint| !endpoint.security_finding_ids.is_empty())
            .count(),
        untested: endpoints
            .iter()
            .filter(|endpoint| !endpoint.has_tests)
            .count(),
    }
}

fn normalize_path(path: &str) -> String {
    let path = path.trim().trim_matches('"').trim_matches('\'');
    if path.starts_with('/') {
        path.into()
    } else {
        format!("/{path}")
    }
}

fn discover_query_parameters(source: &str) -> Vec<ApiParameter> {
    let patterns = [
        Regex::new(r#"query(?:_params)?(?:\.get|\[)\(?[\"']([A-Za-z_][A-Za-z0-9_-]*)"#)
            .expect("static query regex"),
        Regex::new(r#"\.query\(\s*[\"']([A-Za-z_][A-Za-z0-9_-]*)"#).expect("static query regex"),
    ];
    patterns
        .iter()
        .flat_map(|pattern| pattern.captures_iter(source))
        .map(|capture| ApiParameter {
            name: capture[1].into(),
            location: ApiParameterLocation::Query,
            required: false,
            schema_type: "string".into(),
            source: "code".into(),
        })
        .collect()
}

fn discover_headers(source: &str) -> Vec<ApiParameter> {
    let lower = source.to_lowercase();
    let mut names = Vec::new();
    for (needle, name) in [
        ("authorization", "Authorization"),
        ("content-type", "Content-Type"),
        ("x-api-key", "X-API-Key"),
        ("x-request-id", "X-Request-ID"),
    ] {
        if lower.contains(needle) {
            names.push(ApiParameter {
                name: name.into(),
                location: ApiParameterLocation::Header,
                required: needle == "authorization",
                schema_type: "string".into(),
                source: "code".into(),
            });
        }
    }
    names
}

fn discover_auth(source: &str) -> Vec<String> {
    let lower = source.to_lowercase();
    let mut auth = BTreeSet::new();
    if lower.contains("bearer") || lower.contains("authorization") {
        auth.insert("Bearer Token".to_string());
    }
    if lower.contains("basic_auth") || lower.contains("basicauth") {
        auth.insert("Basic Auth".to_string());
    }
    if lower.contains("api_key") || lower.contains("x-api-key") {
        auth.insert("API Key".to_string());
    }
    if lower.contains("cookie") || lower.contains("session") {
        auth.insert("Cookie/Session".to_string());
    }
    auth.into_iter().collect()
}

fn discover_content_types(source: &str) -> Vec<String> {
    let lower = source.to_lowercase();
    let mut values = BTreeSet::new();
    if lower.contains("json") {
        values.insert("application/json".into());
    }
    if lower.contains("multipart") {
        values.insert("multipart/form-data".into());
    }
    if lower.contains("form") {
        values.insert("application/x-www-form-urlencoded".into());
    }
    values.into_iter().collect()
}

fn discover_request_schema(source: &str) -> Option<Value> {
    let patterns = [
        r"(?:Json|web::Json)\s*<\s*([A-Za-z_][A-Za-z0-9_:<>]*)\s*>",
        r"(?:body|payload)\s*:\s*([A-Z][A-Za-z0-9_.$<>]*)",
        r"(?:Body|RequestBody)\s*\(\s*([A-Za-z_][A-Za-z0-9_.$]*)",
    ];
    discover_named_schema(source, &patterns, "request")
}

fn discover_response_schema(source: &str) -> Option<Value> {
    let patterns = [
        r"(?:Json|web::Json)\s*\(\s*([A-Z][A-Za-z0-9_:<>]*)",
        r"(?:ResponseEntity|Promise)\s*<\s*([A-Za-z_][A-Za-z0-9_.$<>]*)\s*>",
        r"(?:response_model\s*=|responses?\s*:\s*)\s*([A-Za-z_][A-Za-z0-9_.$]*)",
    ];
    discover_named_schema(source, &patterns, "response")
}

fn discover_named_schema(source: &str, patterns: &[&str], role: &str) -> Option<Value> {
    for pattern in patterns {
        let pattern = Regex::new(pattern).expect("static schema regex");
        let Some(capture) = pattern.captures(source) else {
            continue;
        };
        let Some(name) = capture.get(1).map(|value| value.as_str()) else {
            continue;
        };
        if matches!(
            name.to_lowercase().as_str(),
            "string" | "str" | "value" | "json"
        ) {
            continue;
        }
        return Some(json!({
            "type":"object",
            "properties":{},
            "additionalProperties":true,
            "x-code-atlas-type":name,
            "x-code-atlas-role":role,
            "x-code-atlas-confidence":"named_type_only"
        }));
    }
    None
}

fn discover_responses(source: &str) -> Vec<ApiResponseMetadata> {
    let status_pattern = Regex::new(r"\b([1-5][0-9]{2})\b").expect("static status regex");
    let mut statuses = status_pattern
        .captures_iter(source)
        .filter_map(|capture| capture[1].parse::<u16>().ok())
        .filter(|status| (100..600).contains(status))
        .collect::<BTreeSet<_>>();
    let lower = source.to_lowercase();
    for (needle, status) in [
        ("statuscode::ok", 200),
        ("statuscode::created", 201),
        ("statuscode::bad_request", 400),
        ("statuscode::unauthorized", 401),
        ("statuscode::not_found", 404),
        ("statuscode::internal_server_error", 500),
    ] {
        if lower.contains(needle) {
            statuses.insert(status);
        }
    }
    let schema = discover_response_schema(source);
    if statuses.is_empty() && schema.is_some() {
        statuses.insert(200);
    }
    statuses
        .into_iter()
        .map(|status| ApiResponseMetadata {
            status,
            content_type: lower.contains("json").then(|| "application/json".into()),
            schema: schema.clone(),
            source: "code".into(),
        })
        .collect()
}

fn infer_framework(graph: &ProjectGraph, path: Option<&str>, source: &str) -> String {
    let path = path.unwrap_or_default().to_lowercase();
    let source = source.to_lowercase();
    let root = graph.root.to_lowercase();
    if path.contains("app/api/") || source.contains("nextrequest") {
        "Next.js".into()
    } else if source.contains("router::new") || source.contains("axum::") {
        "Axum".into()
    } else if source.contains("actix_web") {
        "Actix".into()
    } else if source.contains("fastapi") || source.contains("@app.") {
        "FastAPI".into()
    } else if source.contains("django") {
        "Django".into()
    } else if source.contains("gin.") {
        "Gin".into()
    } else if source.contains("echo.") {
        "Echo".into()
    } else if source.contains("#[route") || root.contains("symfony") {
        "Symfony".into()
    } else if root.contains("laravel") || path.contains("routes/api.php") {
        "Laravel".into()
    } else if source.contains("@controller") || source.contains("@nestjs") {
        "NestJS".into()
    } else if source.contains("express") || source.contains("router.") {
        "Express".into()
    } else {
        "Generic".into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiEnvironmentKind {
    Local,
    Development,
    Staging,
    Production,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnvironment {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub kind: ApiEnvironmentKind,
    pub base_url: String,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub sensitive_variables: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSavedRequest {
    pub id: String,
    pub project_id: String,
    pub collection_id: Option<String>,
    pub name: String,
    pub favorite: bool,
    pub request: ApiExecuteRequest,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCollection {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub request_ids: Vec<String>,
    #[serde(default)]
    pub variable_extractions: BTreeMap<String, String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiHistoryEntry {
    pub id: String,
    pub project_id: String,
    pub endpoint_id: Option<String>,
    pub method: String,
    pub url: String,
    pub environment: ApiEnvironmentKind,
    pub status: Option<u16>,
    pub duration_ms: Option<u128>,
    pub response_size: Option<usize>,
    pub request_metadata: Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiExecuteRequest {
    pub endpoint_id: Option<String>,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
    pub environment: ApiEnvironmentKind,
    #[serde(default)]
    pub confirm_destructive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiExecutionResponse {
    pub status: u16,
    pub duration_ms: u128,
    pub size: usize,
    pub headers: BTreeMap<String, String>,
    pub cookies: Vec<String>,
    pub body: String,
    pub content_type: Option<String>,
    pub truncated: bool,
}

pub async fn execute_request(request: &ApiExecuteRequest) -> Result<ApiExecutionResponse, String> {
    let method = Method::from_bytes(request.method.as_bytes())
        .map_err(|_| "Méthode HTTP invalide".to_string())?;
    let url = reqwest::Url::parse(&request.url).map_err(|_| "URL invalide".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Seuls les schémas HTTP et HTTPS sont autorisés".into());
    }
    let destructive = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    let remote = !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    let protected_environment = matches!(
        request.environment,
        ApiEnvironmentKind::Production | ApiEnvironmentKind::Custom
    );
    if destructive
        && (protected_environment || (remote && method == Method::DELETE))
        && !request.confirm_destructive
    {
        return Err(
            "Confirmation explicite requise pour cette requête destructive distante".into(),
        );
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|error| error.to_string())?;
    let mut builder = client.request(method, url);
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }
    let started = Instant::now();
    let response = builder.send().await.map_err(|error| error.to_string())?;
    let duration_ms = started.elapsed().as_millis();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let cookies = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(mask_cookie)
        .collect();
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    let size = bytes.len();
    let truncated = size > 2_000_000;
    let body = String::from_utf8_lossy(&bytes[..bytes.len().min(2_000_000)]).into_owned();
    Ok(ApiExecutionResponse {
        status,
        duration_ms,
        size,
        headers,
        cookies,
        body,
        content_type,
        truncated,
    })
}

fn mask_cookie(cookie: &str) -> String {
    cookie
        .split_once('=')
        .map(|(name, rest)| {
            let attributes = rest
                .split_once(';')
                .map(|(_, attributes)| format!(";{attributes}"))
                .unwrap_or_default();
            format!("{name}=••••{attributes}")
        })
        .unwrap_or_else(|| "cookie masqué".into())
}

pub fn masked_request(request: &ApiExecuteRequest) -> Value {
    let headers = request
        .headers
        .iter()
        .map(|(name, value)| {
            let sensitive = matches!(
                name.to_lowercase().as_str(),
                "authorization" | "x-api-key" | "proxy-authorization" | "cookie" | "set-cookie"
            );
            (
                name.clone(),
                if sensitive {
                    "••••".into()
                } else {
                    value.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    json!({
        "endpoint_id":request.endpoint_id,
        "method":request.method,
        "url":request.url,
        "headers":headers,
        "environment":request.environment
    })
}

pub fn request_without_secrets(request: &ApiExecuteRequest) -> ApiExecuteRequest {
    let headers = request
        .headers
        .iter()
        .filter(|(name, _)| {
            !matches!(
                name.to_lowercase().as_str(),
                "authorization" | "x-api-key" | "proxy-authorization" | "cookie" | "set-cookie"
            )
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    ApiExecuteRequest {
        endpoint_id: request.endpoint_id.clone(),
        method: request.method.clone(),
        url: request.url.clone(),
        headers,
        body: request.body.clone(),
        environment: request.environment.clone(),
        confirm_destructive: false,
    }
}

pub fn request_as_curl(request: &ApiExecuteRequest) -> String {
    let mut parts = vec![
        "curl".to_string(),
        "-X".into(),
        shell_quote(&request.method),
        shell_quote(&request.url),
    ];
    for (name, value) in &request.headers {
        parts.push("-H".into());
        parts.push(shell_quote(&format!("{name}: {value}")));
    }
    if let Some(body) = &request.body {
        parts.push("--data-raw".into());
        parts.push(shell_quote(body));
    }
    parts.join(" ")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAssertion {
    pub kind: String,
    pub key: Option<String>,
    pub expected: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiContractTest {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub request: ApiExecuteRequest,
    pub assertions: Vec<ApiAssertion>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAssertionResult {
    pub assertion: ApiAssertion,
    pub passed: bool,
    pub actual: Value,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiContractRun {
    pub id: String,
    pub test_id: String,
    pub passed: bool,
    pub results: Vec<ApiAssertionResult>,
    pub response: ApiExecutionResponse,
    pub created_at: i64,
}

pub fn evaluate_assertions(
    response: &ApiExecutionResponse,
    assertions: &[ApiAssertion],
) -> Vec<ApiAssertionResult> {
    let parsed_json = serde_json::from_str::<Value>(&response.body).ok();
    assertions
        .iter()
        .map(|assertion| {
            let (passed, actual, message) = match assertion.kind.as_str() {
                "status" => {
                    let actual = json!(response.status);
                    (
                        assertion.expected.as_ref() == Some(&actual),
                        actual,
                        "Code de statut".into(),
                    )
                }
                "header_exists" => {
                    let key = assertion.key.as_deref().unwrap_or_default().to_lowercase();
                    let exists = response
                        .headers
                        .keys()
                        .any(|name| name.to_lowercase() == key);
                    (exists, json!(exists), format!("En-tête {key}"))
                }
                "header_equals" => {
                    let key = assertion.key.as_deref().unwrap_or_default().to_lowercase();
                    let actual = response
                        .headers
                        .iter()
                        .find(|(name, _)| name.to_lowercase() == key)
                        .map(|(_, value)| json!(value))
                        .unwrap_or(Value::Null);
                    (
                        assertion.expected.as_ref() == Some(&actual),
                        actual,
                        format!("Valeur de l’en-tête {key}"),
                    )
                }
                "json_path_exists" | "json_path_equals" | "json_type" => {
                    let actual = assertion
                        .key
                        .as_deref()
                        .and_then(|path| {
                            parsed_json.as_ref().and_then(|json| json_path(json, path))
                        })
                        .cloned()
                        .unwrap_or(Value::Null);
                    let passed = match assertion.kind.as_str() {
                        "json_path_exists" => !actual.is_null(),
                        "json_type" => assertion
                            .expected
                            .as_ref()
                            .is_some_and(|expected| expected.as_str() == Some(json_type(&actual))),
                        _ => assertion.expected.as_ref() == Some(&actual),
                    };
                    (passed, actual, "Chemin JSON".into())
                }
                "response_time" => {
                    let maximum = assertion
                        .expected
                        .as_ref()
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as u128;
                    (
                        response.duration_ms <= maximum,
                        json!(response.duration_ms),
                        "Temps de réponse".into(),
                    )
                }
                "body_contains" => {
                    let expected = assertion
                        .expected
                        .as_ref()
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    (
                        response.body.contains(expected),
                        json!(response.body.contains(expected)),
                        "Contenu du corps".into(),
                    )
                }
                _ => (false, Value::Null, "Assertion inconnue".into()),
            };
            ApiAssertionResult {
                assertion: assertion.clone(),
                passed,
                actual,
                message,
            }
        })
        .collect()
}

fn json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path
        .trim_start_matches('$')
        .trim_start_matches('.')
        .split('.')
    {
        if segment.is_empty() {
            continue;
        }
        current = current.get(segment)?;
    }
    Some(current)
}

pub fn extract_response_variable(body: &str, path: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(body).ok()?;
    let extracted = json_path(&value, path)?;
    extracted
        .as_str()
        .map(str::to_owned)
        .or_else(|| (!extracted.is_null()).then(|| extracted.to_string()))
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn openapi_document(project_name: &str, endpoints: &[ApiEndpoint]) -> Value {
    let mut paths = serde_json::Map::new();
    for endpoint in endpoints
        .iter()
        .filter(|endpoint| endpoint.transport == "http")
    {
        let operation = json!({
            "operationId":endpoint.id,
            "tags":endpoint.tags,
            "parameters":endpoint.parameters.iter().map(|parameter|json!({
                "name":parameter.name,
                "in":format!("{:?}",parameter.location).to_lowercase(),
                "required":parameter.required,
                "schema":{"type":parameter.schema_type},
                "x-code-atlas-source":parameter.source
            })).collect::<Vec<_>>(),
            "responses": if endpoint.responses.is_empty() {
                json!({"default":{"description":"Réponse non déterminée depuis le code"}})
            } else {
                Value::Object(endpoint.responses.iter().map(|response|(
                    response.status.to_string(),
                    json!({"description":"Réponse détectée","content":response.content_type.as_ref().map(|content_type|json!({content_type:{"schema":response.schema}}))})
                )).collect())
            },
            "x-code-atlas-provenance":endpoint.provenance,
            "x-code-atlas-handler":endpoint.handler_node_id
        });
        paths
            .entry(endpoint.path.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()))
            .as_object_mut()
            .expect("path object")
            .insert(endpoint.method.to_lowercase(), operation);
    }
    json!({
        "openapi":"3.1.0",
        "info":{"title":project_name,"version":"1.0.0","description":"Contrat découvert par Code Atlas ; les schémas inconnus ne sont pas inventés."},
        "paths":paths
    })
}

pub fn merge_openapi(endpoints: &mut [ApiEndpoint], document: &Value) -> usize {
    let mut merged = 0;
    let Some(paths) = document.get("paths").and_then(Value::as_object) else {
        return 0;
    };
    for endpoint in endpoints {
        let Some(operation) = paths
            .get(&endpoint.path)
            .and_then(Value::as_object)
            .and_then(|path| path.get(&endpoint.method.to_lowercase()))
        else {
            continue;
        };
        endpoint.provenance = "both".into();
        if let Some(tags) = operation.get("tags").and_then(Value::as_array) {
            let existing = endpoint.tags.iter().cloned().collect::<BTreeSet<_>>();
            let additions = tags
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .filter(|tag| !existing.contains(tag))
                .collect::<Vec<_>>();
            endpoint.tags.extend(additions);
        }
        if let Some(parameters) = operation.get("parameters").and_then(Value::as_array) {
            for parameter in parameters {
                let Some(name) = parameter.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if endpoint
                    .parameters
                    .iter()
                    .any(|existing| existing.name == name)
                {
                    continue;
                }
                let location = match parameter.get("in").and_then(Value::as_str) {
                    Some("path") => ApiParameterLocation::Path,
                    Some("header") => ApiParameterLocation::Header,
                    Some("cookie") => ApiParameterLocation::Cookie,
                    _ => ApiParameterLocation::Query,
                };
                endpoint.parameters.push(ApiParameter {
                    name: name.into(),
                    location,
                    required: parameter
                        .get("required")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    schema_type: parameter
                        .pointer("/schema/type")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .into(),
                    source: "openapi".into(),
                });
            }
        }
        endpoint.request_schema = operation
            .pointer("/requestBody/content/application~1json/schema")
            .cloned()
            .or_else(|| endpoint.request_schema.clone());
        merged += 1;
    }
    merged
}

pub fn new_id(prefix: &str, value: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let digest = Sha256::digest(format!("{prefix}\0{value}\0{now}").as_bytes());
    format!(
        "{prefix}:{}",
        digest
            .iter()
            .take(12)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{node::CodeNode, project::SourceScope};

    #[test]
    fn assertions_report_pass_and_fail() {
        let response = ApiExecutionResponse {
            status: 200,
            duration_ms: 12,
            size: 20,
            headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
            cookies: vec![],
            body: r#"{"token":"abc","user":{"id":7}}"#.into(),
            content_type: Some("application/json".into()),
            truncated: false,
        };
        let results = evaluate_assertions(
            &response,
            &[
                ApiAssertion {
                    kind: "status".into(),
                    key: None,
                    expected: Some(json!(200)),
                },
                ApiAssertion {
                    kind: "json_path_exists".into(),
                    key: Some("$.token".into()),
                    expected: None,
                },
                ApiAssertion {
                    kind: "json_path_equals".into(),
                    key: Some("$.user.id".into()),
                    expected: Some(json!(8)),
                },
            ],
        );
        assert!(results[0].passed);
        assert!(results[1].passed);
        assert!(!results[2].passed);
    }

    #[test]
    fn collection_variables_are_extracted_and_secrets_are_not_persisted() {
        assert_eq!(
            extract_response_variable(
                r#"{"session":{"token":"secret-token"},"user":{"id":7}}"#,
                "$.session.token"
            )
            .as_deref(),
            Some("secret-token")
        );
        assert_eq!(
            extract_response_variable(r#"{"user":{"id":7}}"#, "$.user.id").as_deref(),
            Some("7")
        );
        let sanitized = request_without_secrets(&ApiExecuteRequest {
            endpoint_id: None,
            method: "GET".into(),
            url: "http://127.0.0.1/profile".into(),
            headers: BTreeMap::from([
                ("Authorization".into(), "Bearer secret-token".into()),
                ("Accept".into(), "application/json".into()),
            ]),
            body: None,
            environment: ApiEnvironmentKind::Local,
            confirm_destructive: true,
        });
        assert!(!sanitized.headers.contains_key("Authorization"));
        assert_eq!(
            sanitized.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
        assert!(!sanitized.confirm_destructive);
    }

    #[test]
    fn production_destructive_requests_need_confirmation() {
        let request = ApiExecuteRequest {
            endpoint_id: None,
            method: "DELETE".into(),
            url: "https://example.com/users/1".into(),
            headers: BTreeMap::new(),
            body: None,
            environment: ApiEnvironmentKind::Production,
            confirm_destructive: false,
        };
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        assert!(runtime.block_on(execute_request(&request)).is_err());
    }

    #[test]
    fn openapi_merge_preserves_code_provenance() {
        let mut endpoint = ApiEndpoint {
            id: "e".into(),
            project_id: "p".into(),
            method: "GET".into(),
            path: "/users/{id}".into(),
            name: "GET /users/{id}".into(),
            framework: "Generic".into(),
            transport: "http".into(),
            handler_node_id: None,
            source_path: None,
            source_start_line: None,
            source_end_line: None,
            auth_requirements: vec![],
            parameters: vec![],
            request_content_types: vec![],
            request_schema: None,
            responses: vec![],
            tags: vec![],
            feature_ids: vec![],
            source_scope: "project".into(),
            provenance: "code".into(),
            security_finding_ids: vec![],
            quality_finding_ids: vec![],
            has_tests: false,
        };
        let merged = merge_openapi(
            std::slice::from_mut(&mut endpoint),
            &json!({"paths":{"/users/{id}":{"get":{"tags":["Users"],"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}}]}}}}),
        );
        assert_eq!(merged, 1);
        assert_eq!(endpoint.provenance, "both");
        assert_eq!(endpoint.parameters.len(), 1);
    }

    #[test]
    fn discovery_keeps_code_provenance_and_named_schemas() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("src")).unwrap();
        std::fs::write(
            directory.path().join("src/api.rs"),
            r#"use axum::Json;
async fn create(Json(payload): Json<CreateUser>) -> Json<UserView> {
    let page = query_params.get("page");
    Json(UserView::from(payload))
}"#,
        )
        .unwrap();
        let node = |id: &str, kind: NodeKind, name: &str| CodeNode {
            id: id.into(),
            kind,
            name: name.into(),
            path: Some("src/api.rs".into()),
            language: None,
            start_line: Some(1),
            end_line: Some(5),
            owner: None,
            source_scope: SourceScope::Project,
        };
        let graph = ProjectGraph::new(
            directory.path().to_string_lossy().into_owned(),
            vec![
                node("route", NodeKind::ApiEndpoint, "POST /users/{id}"),
                node("handler", NodeKind::Handler, "create"),
            ],
            vec![crate::model::edge::CodeEdge::new(
                "route".into(),
                "handler".into(),
                RelationKind::HandledBy,
            )],
            vec![],
            vec![],
            vec![],
        );
        let endpoints = discover_api_endpoints("p", &graph, &[], &[]);
        let endpoint = &endpoints[0];
        assert_eq!(endpoint.framework, "Axum");
        assert_eq!(endpoint.provenance, "code");
        assert_eq!(
            endpoint.request_schema.as_ref().unwrap()["x-code-atlas-type"],
            "CreateUser"
        );
        assert!(
            endpoint
                .parameters
                .iter()
                .any(|parameter| parameter.name == "id")
        );
        assert!(
            endpoint
                .parameters
                .iter()
                .any(|parameter| parameter.name == "page")
        );
        assert!(
            endpoint
                .responses
                .iter()
                .any(|response| response.status == 200)
        );
    }

    #[tokio::test]
    async fn executes_against_a_real_local_http_server() {
        let app = axum::Router::new().route(
            "/health",
            axum::routing::get(|| async { axum::Json(json!({"status":"ok"})) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let response = execute_request(&ApiExecuteRequest {
            endpoint_id: None,
            method: "GET".into(),
            url: format!("http://{address}/health"),
            headers: BTreeMap::new(),
            body: None,
            environment: ApiEnvironmentKind::Local,
            confirm_destructive: false,
        })
        .await
        .unwrap();
        server.abort();
        assert_eq!(response.status, 200);
        assert!(response.body.contains("ok"));
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
    }
}
