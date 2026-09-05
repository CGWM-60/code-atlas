//! Read-only MCP stdio server backed by the persisted Code Atlas registry.
//!
//! No tool accepts a filesystem path. Every lookup starts from a registered
//! project, node, feature, or library identifier so an MCP client cannot use the
//! server as an arbitrary file reader.

use crate::{
    api_explorer::{api_coverage, discover_api_endpoints},
    context_engine::{ContextIntent, ContextPackRequest, build_context_pack},
    documentation::{
        PROJECT_DOC_PROMPT_VERSION, build_project_documentation, project_knowledge_hash,
    },
    features::{build_feature_spec, feature_code, feature_graph, similarity},
    findings::{FindingCategory, detect_findings},
    flow::{FlowDirection, FlowTraceRequest, trace_flow},
    graph::{
        impact::analyze_impact,
        project_graph::ProjectGraph,
        project_map::build_project_map,
        search::{SearchQuery, search},
    },
    library::source_for_node,
    my_code::list_my_code,
    porting::{TargetProfile, build_porting_plan},
    storage::Repository,
};
use anyhow::{Context, Result, anyhow};
use serde_json::{Map, Value, json};
use std::{io::Write, path::Path};
use tokio::io::{AsyncBufReadExt, BufReader};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub fn status(repository: &Repository) -> Result<Value> {
    let projects = repository.list()?;
    let catalog = tools();
    Ok(json!({
        "status":"operational",
        "transport":"stdio",
        "protocol_version":PROTOCOL_VERSION,
        "server_version":env!("CARGO_PKG_VERSION"),
        "database":"connected",
        "registered_projects":projects.len(),
        "tool_count":catalog.len(),
        "tools":catalog.iter().filter_map(|tool| tool.get("name")).collect::<Vec<_>>(),
        "command":"code-atlas mcp --db <chemin-vers-code-atlas.sqlite>",
        "checked_at":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs()
    }))
}

pub async fn serve_stdio(database: &Path) -> Result<()> {
    if let Some(parent) = database.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let repository = Repository::open(database)?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle_request(&repository, &request),
            Err(error) => Some(error_response(Value::Null, -32700, &error.to_string())),
        };
        if let Some(response) = response {
            let mut stdout = std::io::stdout().lock();
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle_request(repository: &Repository, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str)?;
    id.as_ref()?;
    let id = id.unwrap_or(Value::Null);
    match method {
        "initialize" => Some(json!({
            "jsonrpc":"2.0","id":id,"result":{
                "protocolVersion":PROTOCOL_VERSION,
                "capabilities":{"tools":{"listChanged":false}},
                "serverInfo":{"name":"code-atlas","version":env!("CARGO_PKG_VERSION")}
            }
        })),
        "ping" => Some(json!({"jsonrpc":"2.0","id":id,"result":{}})),
        "tools/list" => Some(json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools()}})),
        "tools/call" => {
            let params = request.get("params").and_then(Value::as_object);
            let name = params
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str);
            let arguments = params
                .and_then(|value| value.get("arguments"))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            match name {
                Some(name) if tools().iter().any(|tool| tool["name"] == name) => {
                    Some(match call_tool(repository, name, &arguments) {
                        Ok(value) => tool_response(id, value, false),
                        Err(error) => tool_response(id, json!({"error":error.to_string()}), true),
                    })
                }
                Some(name) => Some(error_response(id, -32602, &format!("Unknown tool: {name}"))),
                None => Some(error_response(id, -32602, "missing tool name")),
            }
        }
        _ => Some(error_response(id, -32601, "method not found")),
    }
}

fn tool_response(id: Value, value: Value, is_error: bool) -> Value {
    let text = serde_json::to_string(&value).unwrap_or_else(|_| "{}".into());
    json!({"jsonrpc":"2.0","id":id,"result":{
        "content":[{"type":"text","text":text}],
        "structuredContent":value,"isError":is_error
    }})
}
fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name":name,"title":name.replace('_', " "),"description":description,
        "inputSchema":input_schema,
        "annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}
    })
}
fn project_schema(extra: Value, extra_required: &[&str]) -> Value {
    let mut properties = extra.as_object().cloned().unwrap_or_default();
    properties.insert("project_id".into(), json!({"type":"string"}));
    let mut required = vec!["project_id"];
    required.extend_from_slice(extra_required);
    schema(Value::Object(properties), &required)
}

fn tools() -> Vec<Value> {
    let none = schema(json!({}), &[]);
    let project = project_schema(json!({}), &[]);
    let node = project_schema(json!({"node_id":{"type":"string"}}), &["node_id"]);
    vec![
        tool(
            "atlas_semantic_search",
            "Search local concept vectors; heuristic, not neural embeddings.",
            project_schema(json!({"query":{"type":"string"}}), &["query"]),
        ),
        tool(
            "atlas_hybrid_search",
            "Combine lexical, symbol, graph and local concept similarity scores.",
            project_schema(json!({"query":{"type":"string"}}), &["query"]),
        ),
        tool(
            "atlas_get_test_plan",
            "Inspect existing tests and proposed scenarios; coverage remains unverified.",
            project_schema(json!({"feature_id":{"type":"string"}}), &[]),
        ),
        tool(
            "atlas_estimate_change",
            "Estimate candidate scope and heuristic time ranges.",
            project_schema(json!({"task":{"type":"string"}}), &["task"]),
        ),
        tool(
            "atlas_get_git_diff",
            "Read a Git diff without modifying the repository.",
            project_schema(
                json!({"base":{"type":"string"},"head":{"type":"string"}}),
                &[],
            ),
        ),
        tool(
            "atlas_review_diff",
            "Read diff lines and candidate graph/Feature impacts.",
            project_schema(
                json!({"base":{"type":"string"},"head":{"type":"string"}}),
                &[],
            ),
        ),
        tool(
            "atlas_list_projects",
            "List registered Code Atlas projects.",
            none,
        ),
        tool(
            "atlas_get_project",
            "Get registered project metadata.",
            project.clone(),
        ),
        tool(
            "atlas_get_architecture",
            "Get the compact deterministic project architecture map.",
            project.clone(),
        ),
        tool(
            "atlas_search",
            "Search ranked code symbols in a registered project.",
            project_schema(
                json!({"query":{"type":"string"},"kind":{"type":"string"},"language":{"type":"string"},"path":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":200}}),
                &["query"],
            ),
        ),
        tool(
            "atlas_get_node",
            "Get a node with source and graph relationships.",
            node.clone(),
        ),
        tool(
            "atlas_get_source",
            "Get source belonging to a registered first-party node.",
            node.clone(),
        ),
        tool(
            "atlas_get_callers",
            "Get incoming callers and relationships for a node.",
            node.clone(),
        ),
        tool(
            "atlas_get_callees",
            "Get outgoing callees and relationships for a node.",
            node.clone(),
        ),
        tool(
            "atlas_get_dependencies",
            "Get direct incoming and outgoing dependencies.",
            node.clone(),
        ),
        tool(
            "atlas_get_impact",
            "Traverse incoming impact from a node.",
            project_schema(
                json!({"node_id":{"type":"string"},"depth":{"type":"integer","minimum":1,"maximum":8}}),
                &["node_id"],
            ),
        ),
        tool(
            "atlas_list_features",
            "List persisted feature candidates for a project.",
            project.clone(),
        ),
        tool(
            "atlas_get_feature",
            "Get one persisted feature and its specification.",
            schema(json!({"feature_id":{"type":"string"}}), &["feature_id"]),
        ),
        tool(
            "atlas_get_feature_graph",
            "Get the validated first-party subgraph for a feature.",
            schema(json!({"feature_id":{"type":"string"}}), &["feature_id"]),
        ),
        tool(
            "atlas_get_feature_code",
            "Get bounded source snippets for a feature.",
            schema(json!({"feature_id":{"type":"string"}}), &["feature_id"]),
        ),
        tool(
            "atlas_get_feature_spec",
            "Get or deterministically derive a neutral feature specification.",
            schema(json!({"feature_id":{"type":"string"}}), &["feature_id"]),
        ),
        tool(
            "atlas_find_similar_feature",
            "Rank similar features across all registered projects.",
            schema(
                json!({"feature_id":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":50}}),
                &["feature_id"],
            ),
        ),
        tool(
            "atlas_list_my_code",
            "List paginated first-party symbols only.",
            project_schema(
                json!({"query":{"type":"string"},"kind":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}}),
                &[],
            ),
        ),
        tool(
            "atlas_find_reusable_function",
            "Search the persistent Function Library.",
            schema(json!({"query":{"type":"string"}}), &["query"]),
        ),
        tool(
            "atlas_get_library_function",
            "Get a Function Library entry by id.",
            schema(json!({"library_id":{"type":"string"}}), &["library_id"]),
        ),
        tool(
            "atlas_get_quality_findings",
            "Get deterministic quality, architecture, and performance findings.",
            project.clone(),
        ),
        tool(
            "atlas_get_security_findings",
            "Get deterministic security findings.",
            project.clone(),
        ),
        tool(
            "atlas_get_project_docs",
            "Get cached or deterministic project documentation.",
            project.clone(),
        ),
        tool(
            "atlas_get_context",
            "Build a token-budgeted grounded ContextPack.",
            project_schema(
                json!({"task":{"type":"string"},"intent":{"type":"string"},"node_ids":{"type":"array","items":{"type":"string"}},"max_tokens":{"type":"integer","minimum":1000,"maximum":180000}}),
                &["task"],
            ),
        ),
        tool(
            "atlas_prepare_task",
            "Prepare grounded implementation context for a task.",
            project_schema(
                json!({"task":{"type":"string"},"node_ids":{"type":"array","items":{"type":"string"}},"max_tokens":{"type":"integer","minimum":1000,"maximum":180000}}),
                &["task"],
            ),
        ),
        tool(
            "atlas_prepare_feature_port",
            "Build a target-aware porting plan without writing files.",
            schema(
                json!({"feature_id":{"type":"string"},"target_profile":{"type":"string"},"target_project_id":{"type":"string"},"constraints":{"type":"array","items":{"type":"string"}}}),
                &["feature_id", "target_profile"],
            ),
        ),
        tool(
            "atlas_list_api_endpoints",
            "List automatically discovered HTTP, SSE and WebSocket endpoints.",
            project.clone(),
        ),
        tool(
            "atlas_get_api_endpoint",
            "Get one discovered endpoint with request, response, source and Feature metadata.",
            project_schema(json!({"endpoint_id":{"type":"string"}}), &["endpoint_id"]),
        ),
        tool(
            "atlas_trace_api_endpoint",
            "Trace the weighted first-party flow downstream from an API endpoint.",
            project_schema(
                json!({"endpoint_id":{"type":"string"},"max_depth":{"type":"integer","minimum":1,"maximum":20}}),
                &["endpoint_id"],
            ),
        ),
        tool(
            "atlas_get_api_contract",
            "Get the discovered request and response contract for an endpoint.",
            project_schema(json!({"endpoint_id":{"type":"string"}}), &["endpoint_id"]),
        ),
        tool(
            "atlas_get_api_security",
            "Get security evidence associated with an API endpoint.",
            project_schema(json!({"endpoint_id":{"type":"string"}}), &["endpoint_id"]),
        ),
        tool(
            "atlas_prepare_api_test",
            "Prepare a deterministic contract-test specification without sending a request.",
            project_schema(
                json!({"endpoint_id":{"type":"string"},"base_url":{"type":"string"}}),
                &["endpoint_id"],
            ),
        ),
    ]
}

fn mcp_api_endpoints(
    repository: &Repository,
    project_id: &str,
    graph: &ProjectGraph,
) -> Result<Vec<crate::api_explorer::ApiEndpoint>> {
    let features = repository.list_features(project_id)?;
    let findings = repository.replace_findings(project_id, &detect_findings(project_id, graph))?;
    let mut endpoints = discover_api_endpoints(project_id, graph, &features, &findings);
    if let Some(document) = repository.get_openapi_document(project_id)? {
        crate::api_explorer::merge_openapi(&mut endpoints, &document);
    }
    Ok(endpoints)
}

fn string_arg<'a>(args: &'a Map<String, Value>, name: &str) -> Result<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("{name} is required"))
}
fn project_graph(
    repository: &Repository,
    args: &Map<String, Value>,
) -> Result<(String, ProjectGraph)> {
    let id = string_arg(args, "project_id")?.to_owned();
    let graph = repository
        .load(&id)?
        .ok_or_else(|| anyhow!("project not found"))?;
    Ok((id, graph))
}
fn feature_with_graph(
    repository: &Repository,
    args: &Map<String, Value>,
) -> Result<(crate::features::Feature, ProjectGraph)> {
    let id = string_arg(args, "feature_id")?;
    let feature = repository
        .get_feature(id)?
        .ok_or_else(|| anyhow!("feature not found"))?;
    let graph = repository
        .load(&feature.project_id)?
        .ok_or_else(|| anyhow!("project not found"))?;
    Ok((feature, graph))
}
fn node_value(graph: &ProjectGraph, node_id: &str) -> Result<Value> {
    let node = graph
        .find_node(node_id)
        .ok_or_else(|| anyhow!("node not found"))?;
    Ok(json!({
        "node":node,
        "incoming":graph.incoming_edges(node_id),
        "outgoing":graph.outgoing_edges(node_id),
        "source":source_for_node(graph,node)
    }))
}
fn parse_intent(value: Option<&str>, fallback: ContextIntent) -> Result<ContextIntent> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    serde_json::from_value(json!(value)).context("invalid context intent")
}
fn context_value(
    repository: &Repository,
    args: &Map<String, Value>,
    fallback: ContextIntent,
) -> Result<Value> {
    let (id, graph) = project_graph(repository, args)?;
    let task = string_arg(args, "task")?;
    let intent = parse_intent(args.get("intent").and_then(Value::as_str), fallback)?;
    let node_ids: Vec<String> = args
        .get("node_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let library = repository
        .list_library(None)?
        .into_iter()
        .filter(|entry| entry.source_project_id == id)
        .collect::<Vec<_>>();
    let features = repository.list_features(&id)?;
    let findings = repository.list_findings(&id, None)?;
    Ok(serde_json::to_value(build_context_pack(
        ContextPackRequest {
            project_id: &id,
            graph: &graph,
            intent,
            task,
            explicit_ids: &node_ids,
            requested_tokens: args
                .get("max_tokens")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
            library: &library,
            features: &features,
            findings: &findings,
        },
    ))?)
}

pub(crate) fn call_tool(
    repository: &Repository,
    name: &str,
    args: &Map<String, Value>,
) -> Result<Value> {
    match name {
        "atlas_hybrid_search" | "atlas_semantic_search" => {
            use crate::retrieval::{EmbeddingProvider, VectorStore};
            let (id, graph) = project_graph(repository, args)?;
            let features = repository.list_features(&id)?;
            let query = string_arg(args, "query")?;
            if name == "atlas_semantic_search" {
                crate::retrieval::rebuild_project(repository, &id, &graph, &features, false)?;
                return Ok(json!(repository.search_by_project(
                    &id,
                    &crate::retrieval::LocalEmbedding.embed(query),
                    None,
                    20
                )?));
            }
            Ok(
                json!({"items":crate::retrieval::HybridRetriever::search(repository, &id, &graph, &features, query, None, 20)?}),
            )
        }
        "atlas_get_test_plan" => {
            let (id, graph) = project_graph(repository, args)?;
            Ok(json!(crate::intelligence::cached_test_plan(
                repository, &id, &graph,
                &repository.list_features(&id)?,
                args.get("feature_id").and_then(Value::as_str)
            )?))
        }
        "atlas_estimate_change" => {
            let (id, graph) = project_graph(repository, args)?;
            Ok(json!(crate::intelligence::estimate(
                repository,
                &id,
                &graph,
                &repository.list_features(&id)?,
                string_arg(args, "task")?
            )?))
        }
        "atlas_get_git_diff" | "atlas_review_diff" => {
            let (id, graph) = project_graph(repository, args)?;
            Ok(json!(crate::intelligence::git_diff(
                &graph,
                &repository.list_features(&id)?,
                args.get("base").and_then(Value::as_str),
                args.get("head").and_then(Value::as_str)
            )?))
        }
        "atlas_list_projects" => Ok(serde_json::to_value(repository.list()?)?),
        "atlas_get_project" => {
            let id = string_arg(args, "project_id")?;
            let project = repository
                .list()?
                .into_iter()
                .find(|project| project.id == id)
                .ok_or_else(|| anyhow!("project not found"))?;
            Ok(serde_json::to_value(project)?)
        }
        "atlas_get_architecture" => {
            let (_, graph) = project_graph(repository, args)?;
            Ok(serde_json::to_value(build_project_map(&graph))?)
        }
        "atlas_search" => {
            let (_, graph) = project_graph(repository, args)?;
            let kind = args
                .get("kind")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .context("invalid node kind")?;
            Ok(serde_json::to_value(search(
                &graph,
                &SearchQuery {
                    q: Some(string_arg(args, "query")?.into()),
                    kind,
                    language: args
                        .get("language")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    path: args.get("path").and_then(Value::as_str).map(str::to_owned),
                    limit: args
                        .get("limit")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize),
                },
            ))?)
        }
        "atlas_get_node" => {
            let (_, graph) = project_graph(repository, args)?;
            node_value(&graph, string_arg(args, "node_id")?)
        }
        "atlas_get_source" => {
            let (_, graph) = project_graph(repository, args)?;
            let node = graph
                .find_node(string_arg(args, "node_id")?)
                .ok_or_else(|| anyhow!("node not found"))?;
            if !node.source_scope.library_eligible() {
                return Err(anyhow!("source is not first-party"));
            }
            Ok(
                json!({"node_id":node.id,"path":node.path,"start_line":node.start_line,"end_line":node.end_line,"source":source_for_node(&graph,node)}),
            )
        }
        "atlas_get_callers" => {
            let (_, graph) = project_graph(repository, args)?;
            let id = string_arg(args, "node_id")?;
            graph
                .find_node(id)
                .ok_or_else(|| anyhow!("node not found"))?;
            Ok(json!(graph.incoming_edges(id)))
        }
        "atlas_get_callees" => {
            let (_, graph) = project_graph(repository, args)?;
            let id = string_arg(args, "node_id")?;
            graph
                .find_node(id)
                .ok_or_else(|| anyhow!("node not found"))?;
            Ok(json!(graph.outgoing_edges(id)))
        }
        "atlas_get_dependencies" => {
            let (_, graph) = project_graph(repository, args)?;
            let id = string_arg(args, "node_id")?;
            graph
                .find_node(id)
                .ok_or_else(|| anyhow!("node not found"))?;
            Ok(json!({"incoming":graph.incoming_edges(id),"outgoing":graph.outgoing_edges(id)}))
        }
        "atlas_get_impact" => {
            let (_, graph) = project_graph(repository, args)?;
            let value = analyze_impact(
                &graph,
                string_arg(args, "node_id")?,
                args.get("depth")
                    .and_then(Value::as_u64)
                    .unwrap_or(3)
                    .min(8) as usize,
            )
            .ok_or_else(|| anyhow!("node not found"))?;
            Ok(serde_json::to_value(value)?)
        }
        "atlas_list_features" => {
            let (id, _) = project_graph(repository, args)?;
            Ok(json!({"items":repository.list_features(&id)?}))
        }
        "atlas_get_feature" => {
            let (feature, graph) = feature_with_graph(repository, args)?;
            let spec = repository
                .get_feature_spec(&feature.id)?
                .unwrap_or_else(|| build_feature_spec(&graph, &feature));
            Ok(json!({"feature":feature,"spec":spec}))
        }
        "atlas_get_feature_graph" => {
            let (feature, graph) = feature_with_graph(repository, args)?;
            Ok(serde_json::to_value(feature_graph(&graph, &feature))?)
        }
        "atlas_get_feature_code" => {
            let (feature, graph) = feature_with_graph(repository, args)?;
            Ok(json!({"items":feature_code(&graph,&feature)}))
        }
        "atlas_get_feature_spec" => {
            let (feature, graph) = feature_with_graph(repository, args)?;
            Ok(serde_json::to_value(
                repository
                    .get_feature_spec(&feature.id)?
                    .unwrap_or_else(|| build_feature_spec(&graph, &feature)),
            )?)
        }
        "atlas_find_similar_feature" => {
            let (feature, _) = feature_with_graph(repository, args)?;
            let mut values = repository
                .list()?
                .into_iter()
                .flat_map(|project| repository.list_features(&project.id).unwrap_or_default())
                .filter(|other| other.id != feature.id)
                .map(|other| json!({"score":similarity(&feature,&other),"feature":other}))
                .collect::<Vec<_>>();
            values.sort_by(|a, b| {
                b["score"]
                    .as_f64()
                    .partial_cmp(&a["score"].as_f64())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            values.truncate(
                args.get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(20)
                    .min(50) as usize,
            );
            Ok(json!({"items":values}))
        }
        "atlas_list_my_code" => {
            let (id, graph) = project_graph(repository, args)?;
            let library = repository
                .list_library(None)?
                .into_iter()
                .filter(|entry| entry.source_project_id == id)
                .map(|entry| entry.source_node_id)
                .collect();
            Ok(serde_json::to_value(list_my_code(
                &graph,
                &library,
                args.get("query").and_then(Value::as_str),
                args.get("kind").and_then(Value::as_str),
                args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize,
                args.get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(100)
                    .min(200) as usize,
            ))?)
        }
        "atlas_find_reusable_function" => Ok(serde_json::to_value(
            repository.list_library(Some(string_arg(args, "query")?))?,
        )?),
        "atlas_get_library_function" => Ok(serde_json::to_value(
            repository
                .get_library_entry(string_arg(args, "library_id")?)?
                .ok_or_else(|| anyhow!("library function not found"))?,
        )?),
        "atlas_get_quality_findings" | "atlas_get_security_findings" => {
            let (id, graph) = project_graph(repository, args)?;
            let all = repository.replace_findings(&id, &detect_findings(&id, &graph))?;
            let items = all
                .into_iter()
                .filter(|finding| {
                    if name == "atlas_get_security_findings" {
                        finding.category == FindingCategory::Security
                    } else {
                        finding.category != FindingCategory::Security
                    }
                })
                .collect::<Vec<_>>();
            Ok(json!({"items":items}))
        }
        "atlas_get_project_docs" => {
            let (id, graph) = project_graph(repository, args)?;
            let features = repository.list_features(&id)?;
            let specs = features
                .iter()
                .filter_map(|feature| repository.get_feature_spec(&feature.id).ok().flatten())
                .collect::<Vec<_>>();
            let hash = project_knowledge_hash(&graph, &features, &specs);
            let docs = repository
                .get_project_documentation(&id)?
                .filter(|docs| {
                    docs.content_hash == hash && docs.prompt_version == PROJECT_DOC_PROMPT_VERSION
                })
                .unwrap_or_else(|| {
                    let mut docs = build_project_documentation(&id, &graph);
                    docs.content_hash = hash;
                    docs.features = features
                        .iter()
                        .map(|feature| format!("{} — {}", feature.name, feature.description))
                        .collect();
                    docs
                });
            repository.save_project_documentation(&docs)?;
            Ok(serde_json::to_value(docs)?)
        }
        "atlas_get_context" => context_value(repository, args, ContextIntent::ExplainNode),
        "atlas_prepare_task" => context_value(repository, args, ContextIntent::PrepareTask),
        "atlas_prepare_feature_port" => {
            let (feature, graph) = feature_with_graph(repository, args)?;
            let spec = repository
                .get_feature_spec(&feature.id)?
                .unwrap_or_else(|| build_feature_spec(&graph, &feature));
            let profile: TargetProfile =
                serde_json::from_value(json!(string_arg(args, "target_profile")?))
                    .context("invalid target profile")?;
            let target_id = args
                .get("target_project_id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let target_graph = target_id
                .as_deref()
                .map(|id| repository.load(id))
                .transpose()?
                .flatten()
                .ok_or_else(|| anyhow!("target project not found"))
                .map(Some)
                .or_else(|error| {
                    if target_id.is_none() {
                        Ok(None)
                    } else {
                        Err(error)
                    }
                })?;
            let constraints = args
                .get("constraints")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            Ok(serde_json::to_value(build_porting_plan(
                &feature,
                &spec,
                profile,
                target_id,
                target_graph.as_ref(),
                constraints,
            ))?)
        }
        "atlas_list_api_endpoints" => {
            let (id, graph) = project_graph(repository, args)?;
            let endpoints = mcp_api_endpoints(repository, &id, &graph)?;
            Ok(json!({"coverage":api_coverage(&endpoints),"items":endpoints}))
        }
        "atlas_get_api_endpoint"
        | "atlas_get_api_contract"
        | "atlas_get_api_security"
        | "atlas_prepare_api_test" => {
            let (id, graph) = project_graph(repository, args)?;
            let endpoint_id = string_arg(args, "endpoint_id")?;
            let endpoints = mcp_api_endpoints(repository, &id, &graph)?;
            let endpoint = endpoints
                .into_iter()
                .find(|endpoint| endpoint.id == endpoint_id)
                .ok_or_else(|| anyhow!("endpoint not found"))?;
            match name {
                "atlas_get_api_contract" => Ok(json!({
                    "endpoint_id":endpoint.id,
                    "method":endpoint.method,
                    "path":endpoint.path,
                    "parameters":endpoint.parameters,
                    "request_content_types":endpoint.request_content_types,
                    "request_schema":endpoint.request_schema,
                    "responses":endpoint.responses,
                    "provenance":endpoint.provenance
                })),
                "atlas_get_api_security" => {
                    let findings = repository
                        .list_findings(&id, Some("security"))?
                        .into_iter()
                        .filter(|finding| endpoint.security_finding_ids.contains(&finding.id))
                        .collect::<Vec<_>>();
                    Ok(json!({
                        "endpoint_id":endpoint.id,
                        "auth_requirements":endpoint.auth_requirements,
                        "findings":findings
                    }))
                }
                "atlas_prepare_api_test" => {
                    let base_url = args
                        .get("base_url")
                        .and_then(Value::as_str)
                        .unwrap_or("http://127.0.0.1:3000");
                    let expected_status = endpoint
                        .responses
                        .first()
                        .map(|response| response.status)
                        .unwrap_or(200);
                    Ok(json!({
                        "name":format!("{} {}",endpoint.method,endpoint.path),
                        "request":{
                            "endpoint_id":endpoint.id,
                            "method":endpoint.method,
                            "url":format!("{}{}",base_url.trim_end_matches('/'),endpoint.path),
                            "headers":{},
                            "body":Value::Null,
                            "environment":"local",
                            "confirm_destructive":false
                        },
                        "assertions":[{"kind":"status","expected":expected_status}],
                        "framework":endpoint.framework,
                        "warning":"La requête n’a pas été exécutée."
                    }))
                }
                _ => Ok(serde_json::to_value(endpoint)?),
            }
        }
        "atlas_trace_api_endpoint" => {
            let (_, graph) = project_graph(repository, args)?;
            let endpoint_id = string_arg(args, "endpoint_id")?;
            Ok(serde_json::to_value(trace_flow(
                &graph,
                &FlowTraceRequest {
                    start_node_id: endpoint_id.into(),
                    end_node_id: None,
                    direction: FlowDirection::Downstream,
                    max_depth: args
                        .get("max_depth")
                        .and_then(Value::as_u64)
                        .unwrap_or(8)
                        .min(20) as usize,
                    relations: vec![],
                },
            ))?)
        }
        _ => Err(anyhow!("unknown tool")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lists_required_tools_and_rejects_unknown_projects() {
        let repository = Repository::in_memory().unwrap();
        let list = handle_request(
            &repository,
            &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        )
        .unwrap();
        let names = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"atlas_prepare_feature_port"));
        assert!(names.contains(&"atlas_get_context"));
        let call=handle_request(&repository,&json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"atlas_get_project","arguments":{"project_id":"missing"}}})).unwrap();
        assert_eq!(call["result"]["isError"], true);
    }
    #[test]
    fn initialize_negotiates_current_protocol() {
        let repository = Repository::in_memory().unwrap();
        let response=handle_request(&repository,&json!({"jsonrpc":"2.0","id":"a","method":"initialize","params":{"protocolVersion":PROTOCOL_VERSION,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).unwrap();
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            false
        );
    }
    #[test]
    fn status_uses_the_real_tool_catalog() {
        let repository = Repository::in_memory().unwrap();
        let value = status(&repository).unwrap();
        assert_eq!(value["status"], "operational");
        assert_eq!(value["tool_count"].as_u64().unwrap(), tools().len() as u64);
        assert!(
            value["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|name| name == "atlas_get_context")
        );
    }
}
