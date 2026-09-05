use crate::{
    graph::project_graph::ProjectGraph,
    model::{edge::RelationKind, node::NodeKind},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

static EXTERNAL_INPUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(request\s*(?:->|\.)|\$_(?:get|post|request|files)|req\.(?:body|query|params)|query_params|search_params|input\s*\(|argv\b|payload\s*(?:\[|\.))",
    )
    .expect("static external input regex")
});
static SQL_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(select|insert\s+into|update\s+\w+|delete\s+from|from\s+\w+|where\s+)\b")
        .expect("static SQL regex")
});
static CRYPTO_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(crypt|cipher|encrypt|decrypt|password|credential|hmac|security|auth|secret|token)\w*\b",
    )
    .expect("static crypto context regex")
});
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Quality,
    Architecture,
    Performance,
    Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    #[default]
    Open,
    Accepted,
    Ignored,
    Fixed,
    FalsePositive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSpan {
    pub path: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub snippet: Option<String>,
    pub label: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub fingerprint: String,
    pub project_id: String,
    pub category: FindingCategory,
    pub severity: FindingSeverity,
    pub title: String,
    pub description: String,
    pub evidence: Vec<String>,
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub path: Option<String>,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    #[serde(default)]
    pub primary_span: Option<SourceSpan>,
    #[serde(default)]
    pub evidence_spans: Vec<SourceSpan>,
    pub detector: String,
    pub confidence: f32,
    #[serde(default)]
    pub status: FindingStatus,
    pub source_hash: String,
    pub ai_analysis: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiFindingAnalysis {
    pub summary: String,
    pub risk: String,
    pub remediation: Vec<String>,
    pub evidence: Vec<String>,
    pub confidence: f32,
}

pub fn finding_analysis_schema() -> serde_json::Value {
    serde_json::json!({"type":"object","additionalProperties":false,
        "required":["summary","risk","remediation","evidence","confidence"],
        "properties":{"summary":{"type":"string"},"risk":{"type":"string"},
        "remediation":{"type":"array","items":{"type":"string"}},"evidence":{"type":"array","items":{"type":"string"}},
        "confidence":{"type":"number","minimum":0,"maximum":1}}})
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSecurityReview {
    pub executive_summary: String,
    pub prioritized_risks: Vec<String>,
    pub attack_paths: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub limitations: Vec<String>,
}

pub fn security_review_schema() -> serde_json::Value {
    let values = || serde_json::json!({"type":"array","items":{"type":"string"}});
    serde_json::json!({"type":"object","additionalProperties":false,"required":["executive_summary","prioritized_risks","attack_paths","recommended_actions","limitations"],"properties":{"executive_summary":{"type":"string"},"prioritized_risks":values(),"attack_paths":values(),"recommended_actions":values(),"limitations":values()}})
}

fn stable_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct FindingInput<'a> {
    project_id: &'a str,
    category: FindingCategory,
    severity: FindingSeverity,
    title: &'a str,
    description: String,
    evidence: Vec<String>,
    node_ids: Vec<String>,
    edge_ids: Vec<String>,
    path: Option<String>,
    start_line: Option<usize>,
    end_line: Option<usize>,
    detector: &'a str,
    confidence: f32,
    source_hash: String,
}

fn finding(input: FindingInput<'_>) -> Finding {
    let path = input.path.as_deref().unwrap_or_default();
    let node = input
        .node_ids
        .first()
        .map(String::as_str)
        .unwrap_or_default();
    let fingerprint = stable_hash(&[input.detector, path, node, &input.source_hash]);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let primary_span = input.path.as_ref().map(|path| SourceSpan {
        path: path.clone(),
        start_line: input.start_line.unwrap_or(1),
        start_column: 1,
        end_line: input.end_line.or(input.start_line).unwrap_or(1),
        end_column: 1,
        snippet: None,
        label: Some(input.title.into()),
        role: Some("primary".into()),
    });
    Finding {
        id: format!("finding:{}", &fingerprint[..24]),
        fingerprint,
        project_id: input.project_id.into(),
        category: input.category,
        severity: input.severity,
        title: input.title.into(),
        description: input.description,
        evidence: input.evidence,
        node_ids: input.node_ids,
        edge_ids: input.edge_ids,
        path: input.path,
        start_line: input.start_line,
        end_line: input.end_line,
        evidence_spans: primary_span.clone().into_iter().collect(),
        primary_span,
        detector: input.detector.into(),
        confidence: input.confidence,
        status: FindingStatus::Open,
        source_hash: input.source_hash,
        ai_analysis: None,
        created_at: now,
        updated_at: now,
    }
}

pub fn detect_findings(project_id: &str, graph: &ProjectGraph) -> Vec<Finding> {
    let mut findings = Vec::new();
    let call_pairs = graph
        .edges
        .iter()
        .filter(|edge| edge.relation == RelationKind::Calls)
        .map(|edge| (edge.source_id.as_str(), edge.target_id.as_str()))
        .collect::<HashSet<_>>();
    let mut emitted_cycles = HashSet::new();
    let mut sources = HashMap::<String, Vec<String>>::new();
    for node in graph
        .nodes
        .iter()
        .filter(|node| node.source_scope.library_eligible() && node.path.is_some())
    {
        let path = node.path.as_deref().unwrap_or_default();
        let normalized_path = path.to_lowercase().replace('\\', "/");
        let is_test_or_fixture = normalized_path.contains("/test")
            || normalized_path.starts_with("test")
            || normalized_path.contains("/fixtures/")
            || normalized_path.contains(".spec.")
            || normalized_path.contains(".test.");
        let lines = node
            .end_line
            .unwrap_or(0)
            .saturating_sub(node.start_line.unwrap_or(0))
            .saturating_add(1);
        let outgoing = graph.outgoing_edges(&node.id);
        let incoming = graph.incoming_edges(&node.id);
        let source_hash = stable_hash(&[
            path,
            &node.start_line.unwrap_or(0).to_string(),
            &node.end_line.unwrap_or(0).to_string(),
        ]);
        if matches!(
            node.kind,
            NodeKind::Function | NodeKind::Method | NodeKind::Constructor
        ) && lines >= 120
            && !is_test_or_fixture
        {
            findings.push(finding(FindingInput {
                project_id,
                category: FindingCategory::Quality,
                severity: if lines >= 250 {
                    FindingSeverity::High
                } else {
                    FindingSeverity::Medium
                },
                title: "Fonction très longue",
                description: format!(
                    "{} contient {lines} lignes et devient difficile à relire et à tester isolément.",
                    node.name
                ),
                evidence: vec![format!("{lines} lignes de code source")],
                node_ids: vec![node.id.clone()],
                edge_ids: vec![],
                path: node.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
                detector: "long_function",
                confidence: 0.98,
                source_hash: source_hash.clone(),
            }));
        }
        if matches!(
            node.kind,
            NodeKind::Class | NodeKind::Service | NodeKind::Controller
        ) && (800..=5_000).contains(&lines)
            && !is_test_or_fixture
        {
            findings.push(finding(FindingInput {
                project_id,
                category: FindingCategory::Quality,
                severity: FindingSeverity::High,
                title: "Classe à responsabilités excessives",
                description: format!(
                    "{} contient {lines} lignes et semble regrouper trop de responsabilités.",
                    node.name
                ),
                evidence: vec![format!("{lines} lignes de code source")],
                node_ids: vec![node.id.clone()],
                edge_ids: vec![],
                path: node.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
                detector: "god_class_module",
                confidence: 0.76,
                source_hash: source_hash.clone(),
            }));
        }
        let fan_out = outgoing
            .iter()
            .filter(|edge| {
                matches!(
                    edge.relation,
                    RelationKind::Calls
                        | RelationKind::Uses
                        | RelationKind::DependsOn
                        | RelationKind::Imports
                )
            })
            .count();
        if fan_out >= 20 && !is_test_or_fixture {
            findings.push(finding(FindingInput {
                project_id,
                category: FindingCategory::Quality,
                severity: FindingSeverity::Medium,
                title: "Couplage sortant élevé",
                description: format!(
                    "{} possède {fan_out} dépendances sortantes résolues.",
                    node.name
                ),
                evidence: vec![format!("dépendances sortantes : {fan_out}")],
                node_ids: vec![node.id.clone()],
                edge_ids: outgoing.iter().map(|edge| edge.id.clone()).collect(),
                path: node.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
                detector: "high_fan_out",
                confidence: 0.9,
                source_hash: source_hash.clone(),
            }));
        }
        let fan_in = incoming
            .iter()
            .filter(|edge| edge.relation == RelationKind::Calls)
            .count();
        let looks_like_accessor = ["get", "set", "is", "has"].iter().any(|prefix| {
            node.name
                .strip_prefix(prefix)
                .and_then(|suffix| suffix.chars().next())
                .is_some_and(|character| character.is_uppercase() || character == '_')
        });
        if fan_in >= 30 && lines > 10 && !looks_like_accessor && !is_test_or_fixture {
            findings.push(finding(FindingInput {
                project_id,
                category: FindingCategory::Architecture,
                severity: FindingSeverity::Medium,
                title: "Point de changement à fort impact",
                description: format!(
                    "{} possède {fan_in} appelants résolus ; toute modification peut avoir un impact important.",
                    node.name
                ),
                evidence: vec![format!("appelants résolus : {fan_in}")],
                node_ids: vec![node.id.clone()],
                edge_ids: incoming.iter().map(|edge| edge.id.clone()).collect(),
                path: node.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
                detector: "high_fan_in",
                confidence: 0.9,
                source_hash: source_hash.clone(),
            }));
        }
        if matches!(
            node.kind,
            NodeKind::EntryPoint
                | NodeKind::Route
                | NodeKind::ApiEndpoint
                | NodeKind::Handler
                | NodeKind::Controller
                | NodeKind::Service
                | NodeKind::Worker
                | NodeKind::Job
        ) {
            let deepest = resolved_call_depth(graph, &node.id, 8);
            if deepest >= 7 {
                findings.push(finding(FindingInput {
                    project_id,
                    category: FindingCategory::Performance,
                    severity: FindingSeverity::Low,
                    title: "Chaîne d’appels profonde",
                    description: format!(
                        "{} atteint une profondeur d’appels résolus d’au moins {deepest}.",
                        node.name
                    ),
                    evidence: vec![format!("profondeur résolue : {deepest}")],
                    node_ids: vec![node.id.clone()],
                    edge_ids: vec![],
                    path: node.path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                    detector: "deep_call_chain",
                    confidence: 0.7,
                    source_hash: source_hash.clone(),
                }));
            }
        }
        for edge in outgoing
            .iter()
            .filter(|edge| edge.relation == RelationKind::Calls)
        {
            if edge.source_id != edge.target_id
                && call_pairs.contains(&(edge.target_id.as_str(), edge.source_id.as_str()))
            {
                let mut ids = [edge.source_id.clone(), edge.target_id.clone()];
                ids.sort();
                if emitted_cycles.insert(ids.clone()) {
                    findings.push(finding(FindingInput {
                        project_id,
                        category: FindingCategory::Architecture,
                        severity: FindingSeverity::Medium,
                        title: "Dépendance d’appel circulaire",
                        description:
                            "Deux symboles du projet s’appellent directement l’un l’autre.".into(),
                        evidence: vec![format!("{} ↔ {}", ids[0], ids[1])],
                        node_ids: ids.to_vec(),
                        edge_ids: vec![edge.id.clone()],
                        path: node.path.clone(),
                        start_line: node.start_line,
                        end_line: node.end_line,
                        detector: "direct_call_cycle",
                        confidence: 0.96,
                        source_hash: source_hash.clone(),
                    }));
                }
            }
        }
        if !matches!(
            node.kind,
            NodeKind::Function | NodeKind::Method | NodeKind::Constructor
        ) {
            continue;
        }
        let source = sources.entry(path.into()).or_insert_with(|| {
            fs::read_to_string(Path::new(&graph.root).join(path))
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        });
        let snippet = source
            .iter()
            .skip(node.start_line.unwrap_or(1).saturating_sub(1))
            .take(lines.min(500))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        let lower = snippet.to_lowercase();
        let complexity = 1 + [
            " if ", " else ", " for ", " while ", " match ", " case ", " catch ", "&&", "||",
        ]
        .iter()
        .map(|token| lower.matches(token).count())
        .sum::<usize>();
        if complexity >= 25 {
            findings.push(finding(FindingInput {
                project_id,
                category: FindingCategory::Quality,
                severity: if complexity >= 30 {
                    FindingSeverity::High
                } else {
                    FindingSeverity::Medium
                },
                title: "Complexité cyclomatique élevée",
                description: format!(
                    "{} présente une complexité de branchement estimée à {complexity}.",
                    node.name
                ),
                evidence: vec![format!("complexité de branchement estimée : {complexity}")],
                node_ids: vec![node.id.clone()],
                edge_ids: vec![],
                path: node.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
                detector: "cyclomatic_complexity",
                confidence: 0.78,
                source_hash: source_hash.clone(),
            }));
        }
        let signature = snippet.lines().take(5).collect::<Vec<_>>().join(" ");
        if let Some(parameters) = signature
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(parameters, _)| {
                parameters
                    .split(',')
                    .filter(|value| !value.trim().is_empty())
                    .count()
            })
            .filter(|count| *count >= 7)
        {
            let mut detected = finding(FindingInput {
                project_id,
                category: FindingCategory::Quality,
                severity: FindingSeverity::Medium,
                title: "Trop de paramètres",
                description: format!("{} déclare environ {parameters} paramètres.", node.name),
                evidence: vec![format!("nombre de paramètres : {parameters}")],
                node_ids: vec![node.id.clone()],
                edge_ids: vec![],
                path: node.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
                detector: "too_many_parameters",
                confidence: 0.82,
                source_hash: source_hash.clone(),
            });
            let signature_lines = snippet.lines().take(5).collect::<Vec<_>>();
            let start = signature_lines
                .iter()
                .enumerate()
                .find_map(|(index, line)| line.find('(').map(|column| (index, column)));
            let end = signature_lines
                .iter()
                .enumerate()
                .find_map(|(index, line)| line.find(')').map(|column| (index, column)));
            if let (Some((start_index, start_column)), Some((end_index, end_column))) = (start, end)
            {
                let span = SourceSpan {
                    path: path.into(),
                    start_line: node.start_line.unwrap_or(1) + start_index,
                    start_column: signature_lines[start_index][..start_column].chars().count() + 1,
                    end_line: node.start_line.unwrap_or(1) + end_index,
                    end_column: signature_lines[end_index][..end_column].chars().count() + 2,
                    snippet: Some(signature),
                    label: Some(format!("Signature avec {parameters} paramètres")),
                    role: Some("signature".into()),
                };
                detected.primary_span = Some(span.clone());
                detected.evidence_spans = vec![span];
            }
            findings.push(detected);
        }
        let security_patterns = [
            (
                "hardcoded_secret",
                "Secret potentiellement codé en dur",
                FindingSeverity::High,
                ["password =", "api_key =", "secret =", "token ="].as_slice(),
                0.72,
            ),
            (
                "command_injection",
                "Risque potentiel d’injection de commande",
                FindingSeverity::High,
                ["system(", "exec(", "command::new", "child_process.exec"].as_slice(),
                0.68,
            ),
            (
                "unsafe_deserialization",
                "Désérialisation potentiellement dangereuse",
                FindingSeverity::High,
                ["unserialize(", "pickle.loads", "yaml.load("].as_slice(),
                0.82,
            ),
            (
                "sql_injection",
                "Risque potentiel d’injection SQL",
                FindingSeverity::High,
                [
                    "->query(",
                    "->executequery(",
                    "->executestatement(",
                    "rawquery(",
                    "query_raw(",
                    "format!(\"select",
                ]
                .as_slice(),
                0.74,
            ),
            (
                "sensitive_logging",
                "Journalisation potentielle de données sensibles",
                FindingSeverity::Medium,
                [
                    "log(password",
                    "log(token",
                    "console.log(password",
                    "println!(\"token",
                ]
                .as_slice(),
                0.64,
            ),
            (
                "path_traversal",
                "Risque potentiel de traversée de chemin",
                FindingSeverity::High,
                [
                    "read_to_string(",
                    "file_get_contents(",
                    "send_file(",
                    "path.join(",
                ]
                .as_slice(),
                0.48,
            ),
            (
                "weak_crypto",
                "Algorithme cryptographique faible",
                FindingSeverity::Medium,
                ["md5(", "sha1(", "des::", "mode_ecb", "cipher::ecb"].as_slice(),
                0.78,
            ),
            (
                "open_redirect",
                "Redirection ouverte potentielle",
                FindingSeverity::Medium,
                ["redirect($", "redirect(request", "location.href ="].as_slice(),
                0.52,
            ),
            (
                "unsafe_html",
                "Sortie HTML potentiellement non échappée",
                FindingSeverity::High,
                [
                    "dangerouslysetinnerhtml",
                    "|raw",
                    "mark_safe(",
                    "innerhtml =",
                ]
                .as_slice(),
                0.72,
            ),
        ];
        for (detector, title, severity, patterns, confidence) in security_patterns {
            let source_lines = snippet.lines().collect::<Vec<_>>();
            if let Some((pattern, matched_index, matched_column)) =
                patterns.iter().find_map(|pattern| {
                    source_lines.iter().enumerate().find_map(|(index, line)| {
                        pattern_column_outside_literal(&line.to_lowercase(), pattern)
                            .map(|column| (*pattern, index, column))
                    })
                })
            {
                let matched_line = source_lines.get(matched_index).copied().unwrap_or_default();
                let context_start = matched_index.saturating_sub(3);
                let local_context = source_lines
                    .iter()
                    .skip(context_start)
                    .take(7)
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
                    .to_lowercase();
                let has_external_input = EXTERNAL_INPUT_RE.is_match(&local_context);
                let has_sql = SQL_CONTEXT_RE.is_match(&local_context);
                let has_dynamic_sql = local_context.contains(".$")
                    || local_context.contains("${")
                    || local_context.contains("format!(")
                    || local_context.contains("sprintf(")
                    || local_context.contains("+ request")
                    || local_context.contains("+ req.");
                let has_crypto_context = CRYPTO_CONTEXT_RE.is_match(&local_context);
                let credible = match detector {
                    "hardcoded_secret" => matched_line
                        .split_once('=')
                        .map(|(_, value)| {
                            let value = value.trim();
                            (value.starts_with('"') || value.starts_with('\''))
                                && value.trim_matches(['"', '\'', ';', ',', ' ']).len() >= 8
                        })
                        .unwrap_or(false),
                    "sql_injection" => has_sql && has_dynamic_sql && has_external_input,
                    "path_traversal" => {
                        has_external_input
                            && !local_context.contains("uploadedfile")
                            && !local_context.contains("getpathname(")
                            && !local_context.contains("canonicalize(")
                            && !local_context.contains("realpath(")
                    }
                    "command_injection" | "open_redirect" => has_external_input,
                    "weak_crypto" => has_crypto_context,
                    "unsafe_html" => pattern == "dangerouslysetinnerhtml" || has_external_input,
                    _ => true,
                };
                if !credible {
                    continue;
                }
                let absolute_line = node.start_line.unwrap_or(1) + matched_index;
                let mut detected = finding(FindingInput {
                    project_id,
                    category: FindingCategory::Security,
                    severity,
                    title,
                    description: format!(
                        "{} contient un motif qui nécessite une vérification de sécurité ciblée.",
                        node.name
                    ),
                    evidence: vec![format!("motif détecté : {pattern}")],
                    node_ids: vec![node.id.clone()],
                    edge_ids: vec![],
                    path: node.path.clone(),
                    start_line: node.start_line,
                    end_line: node.end_line,
                    detector,
                    confidence,
                    source_hash: stable_hash(&[&snippet]),
                });
                let sink = SourceSpan {
                    path: path.into(),
                    start_line: absolute_line,
                    start_column: matched_column,
                    end_line: absolute_line,
                    end_column: matched_column + pattern.chars().count(),
                    snippet: Some(matched_line.trim().into()),
                    label: Some(format!("Motif {pattern}")),
                    role: Some("sink".into()),
                };
                let source = source_lines
                    .iter()
                    .enumerate()
                    .skip(context_start)
                    .take(7)
                    .find(|(index, line)| {
                        *index != matched_index && EXTERNAL_INPUT_RE.is_match(line)
                    })
                    .map(|(index, line)| SourceSpan {
                        path: path.into(),
                        start_line: node.start_line.unwrap_or(1) + index,
                        start_column: 1,
                        end_line: node.start_line.unwrap_or(1) + index,
                        end_column: line.chars().count().saturating_add(1),
                        snippet: Some(line.trim().into()),
                        label: Some("Entrée potentiellement contrôlée".into()),
                        role: Some("source".into()),
                    });
                detected.primary_span = Some(sink.clone());
                detected.evidence_spans = source.into_iter().chain([sink]).collect();
                findings.push(detected);
            }
        }
        if (lower.contains("for ") || lower.contains("foreach") || lower.contains("while "))
            && (lower.contains("->query(") || lower.contains("fetch(") || lower.contains("reqwest"))
        {
            let mut detected = finding(FindingInput {
                project_id,
                category: FindingCategory::Performance,
                severity: FindingSeverity::Medium,
                title: "Appel d’entrée/sortie dans une boucle",
                description: format!(
                    "{} combine une boucle avec un accès base de données ou réseau.",
                    node.name
                ),
                evidence: vec![
                    "une boucle et un appel d’E/S apparaissent dans la même fonction".into(),
                ],
                node_ids: vec![node.id.clone()],
                edge_ids: vec![],
                path: node.path.clone(),
                start_line: node.start_line,
                end_line: node.end_line,
                detector: "io_inside_loop",
                confidence: 0.55,
                source_hash: stable_hash(&[&snippet]),
            });
            let source_lines = snippet.lines().collect::<Vec<_>>();
            let span = |index: usize, line: &str, label: &str, role: &str| SourceSpan {
                path: path.into(),
                start_line: node.start_line.unwrap_or(1) + index,
                start_column: 1,
                end_line: node.start_line.unwrap_or(1) + index,
                end_column: line.chars().count().saturating_add(1),
                snippet: Some(line.trim().into()),
                label: Some(label.into()),
                role: Some(role.into()),
            };
            let loop_span = source_lines.iter().enumerate().find(|(_, line)| {
                let line = line.to_lowercase();
                line.contains("for ") || line.contains("foreach") || line.contains("while ")
            });
            let io_span = source_lines.iter().enumerate().find(|(_, line)| {
                let line = line.to_lowercase();
                line.contains("->query(") || line.contains("fetch(") || line.contains("reqwest")
            });
            detected.evidence_spans = loop_span
                .map(|(index, line)| span(index, line, "Boucle", "control"))
                .into_iter()
                .chain(
                    io_span.map(|(index, line)| span(index, line, "Appel d’entrée/sortie", "sink")),
                )
                .collect();
            detected.primary_span = detected.evidence_spans.last().cloned();
            findings.push(detected);
        }
    }
    for candidate in crate::taint::analyze(graph) {
        let Some(sink) = candidate.spans.last() else {
            continue;
        };
        if let Some(existing) = findings.iter_mut().find(|finding| {
            finding.category == FindingCategory::Security
                && finding.primary_span.as_ref().is_some_and(|span| {
                    span.path == sink.path && span.start_line == sink.start_line
                })
        }) {
            if candidate.spans.len() > existing.evidence_spans.len() {
                existing.evidence_spans = candidate.spans;
                existing.edge_ids.extend(candidate.edge_ids);
                existing.node_ids.extend(candidate.node_ids);
                existing.node_ids.sort();
                existing.node_ids.dedup();
                existing.edge_ids.sort();
                existing.edge_ids.dedup();
                existing.evidence.push("Propagation de données heuristique sur des appels résolus ; conditions et nettoyages non prouvés.".into());
            }
            continue;
        }
        let mut detected = finding(FindingInput {
            project_id, category: FindingCategory::Security, severity: FindingSeverity::Medium,
            title: "Entrée externe propagée vers un appel sensible",
            description: format!("Un chemin de propagation candidat atteint {}. Analyse heuristique bornée à quatre appels ; les alias, conditions et sanitizers ne sont pas complètement résolus.",candidate.sink),
            evidence: vec!["Les arêtes interprocédurales existent dans ProjectGraph. La propagation des arguments et affectations reste heuristique ; aucune exploitabilité n’est affirmée.".into()],
            node_ids: candidate.node_ids, edge_ids: candidate.edge_ids, path:Some(sink.path.clone()),start_line:Some(sink.start_line),end_line:Some(sink.end_line),detector:"bounded_interprocedural_taint",confidence:0.45,source_hash:stable_hash(&[&serde_json::to_string(&candidate.spans).unwrap_or_default()]),
        });
        detected.primary_span = Some(sink.clone());
        detected.evidence_spans = candidate.spans;
        findings.push(detected);
    }
    findings
}

/// Security signatures appearing only in documentation, examples or detector
/// catalogues are evidence about text, not about executable behavior.
fn pattern_column_outside_literal(line: &str, pattern: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
            continue;
        }
        if quote.is_none() && line[index..].starts_with(pattern) {
            return Some(line[..index].chars().count() + 1);
        }
    }
    None
}

#[cfg(test)]
fn pattern_outside_literal(line: &str, pattern: &str) -> bool {
    pattern_column_outside_literal(line, pattern).is_some()
}

fn resolved_call_depth(graph: &ProjectGraph, start: &str, maximum: usize) -> usize {
    let mut deepest = 0;
    let mut visited = HashSet::from([start]);
    let mut queue = VecDeque::from([(start, 0_usize)]);
    while let Some((current, depth)) = queue.pop_front() {
        if depth >= maximum {
            continue;
        }
        for edge in graph
            .outgoing_edges(current)
            .into_iter()
            .filter(|edge| edge.relation == RelationKind::Calls)
        {
            if visited.insert(edge.target_id.as_str()) {
                let next_depth = depth + 1;
                deepest = deepest.max(next_depth);
                queue.push_back((edge.target_id.as_str(), next_depth));
            }
        }
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{node::CodeNode, project::SourceScope};
    #[test]
    fn detects_long_first_party_code_but_not_external_code() {
        let node = |id: &str, scope| CodeNode {
            id: id.into(),
            kind: NodeKind::Function,
            name: id.into(),
            path: Some(format!("{id}.rs")),
            language: None,
            start_line: Some(1),
            end_line: Some(140),
            owner: None,
            source_scope: scope,
        };
        let graph = ProjectGraph::new(
            ".".into(),
            vec![
                node("ours", SourceScope::Project),
                node("external", SourceScope::ExternalDependency),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let findings = detect_findings("p", &graph);
        assert!(
            findings
                .iter()
                .any(|finding| finding.detector == "long_function" && finding.node_ids == ["ours"])
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding.node_ids == ["external"])
        );
    }

    #[test]
    fn security_patterns_inside_catalogue_literals_are_not_findings() {
        assert!(!pattern_outside_literal(
            r#"[\"password =\", \"unserialize(\", \"dangerouslysetinnerhtml\"]"#,
            "password ="
        ));
        assert!(!pattern_outside_literal(
            r#"let patterns = [\"unserialize(\"];"#,
            "unserialize("
        ));
        assert!(pattern_outside_literal(
            "let digest = md5(user_input);",
            "md5("
        ));
        assert!(!pattern_outside_literal(
            r#"let libellé = \"md5(\";"#,
            "md5("
        ));
    }

    #[test]
    fn security_detection_rejects_http_query_and_non_crypto_hashing() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("http.php"),
            "$client->request('GET', $url, ['query' => $params]);\n",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("dedupe.php"),
            "$dedupeKey = sha1($campaignId . $recipientId);\n",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("sql.php"),
            "$sql = \"SELECT * FROM users WHERE id = \".$_GET['id'];\n$db->query($sql);\n",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("upload.php"),
            "$uploadedFile = $request->files->get('file');\n$content = file_get_contents($uploadedFile->getPathname());\n",
        )
        .unwrap();
        let node = |id: &str, path: &str, end_line| CodeNode {
            id: id.into(),
            kind: NodeKind::Function,
            name: id.into(),
            path: Some(path.into()),
            language: None,
            start_line: Some(1),
            end_line: Some(end_line),
            owner: None,
            source_scope: SourceScope::Project,
        };
        let graph = ProjectGraph::new(
            directory.path().to_string_lossy().into_owned(),
            vec![
                node("http", "http.php", 1),
                node("dedupe", "dedupe.php", 1),
                node("sql", "sql.php", 2),
                node("upload", "upload.php", 2),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let findings = detect_findings("p", &graph);
        assert!(!findings.iter().any(|finding| {
            finding.category == FindingCategory::Security
                && matches!(
                    finding.node_ids.first().map(String::as_str),
                    Some("http" | "dedupe" | "upload")
                )
        }));
        let sql = findings
            .iter()
            .find(|finding| finding.detector == "sql_injection")
            .expect("dynamic SQL fed by external input must be reported");
        assert_eq!(sql.node_ids, ["sql"]);
        assert_eq!(sql.primary_span.as_ref().unwrap().start_line, 2);
    }
}
