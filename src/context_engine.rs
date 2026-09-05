use crate::{
    ai::context_builder::{redact_sensitive_line, sensitive_path},
    features::Feature,
    findings::Finding,
    graph::{
        project_graph::ProjectGraph,
        search::{SearchQuery, search},
    },
    library::LibraryEntry,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextIntent {
    ExplainNode,
    ExplainFeature,
    ProjectDocumentation,
    FeatureDocumentation,
    QualityReview,
    SecurityReview,
    OptimizationReview,
    ImpactReview,
    FeatureDetection,
    FeatureSpecGeneration,
    FeaturePort,
    PrepareTask,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntity {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: Option<String>,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnippet {
    pub node_id: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPack {
    pub intent: ContextIntent,
    pub project_id: String,
    pub project_summary: String,
    pub selected_entities: Vec<ContextEntity>,
    pub graph_evidence: Vec<String>,
    pub source_snippets: Vec<ContextSnippet>,
    pub tests: Vec<String>,
    pub findings: Vec<Finding>,
    pub library_matches: Vec<LibraryEntry>,
    pub feature_matches: Vec<Feature>,
    pub token_estimate: usize,
    pub max_tokens: usize,
    pub truncated: bool,
    pub provenance: Vec<String>,
}
pub struct ContextPackRequest<'a> {
    pub project_id: &'a str,
    pub graph: &'a ProjectGraph,
    pub intent: ContextIntent,
    pub task: &'a str,
    pub explicit_ids: &'a [String],
    pub requested_tokens: Option<usize>,
    pub library: &'a [LibraryEntry],
    pub features: &'a [Feature],
    pub findings: &'a [Finding],
}
pub struct AiCacheWrite<'a> {
    pub cache_key: &'a str,
    pub project_id: &'a str,
    pub analysis_type: &'a str,
    pub content_hash: &'a str,
    pub prompt_version: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    pub result: &'a serde_json::Value,
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct TokenBudgetManager {
    pub configured_max: usize,
    pub reserve: usize,
}
impl Default for TokenBudgetManager {
    fn default() -> Self {
        Self {
            configured_max: 200_000,
            reserve: 20_000,
        }
    }
}
impl TokenBudgetManager {
    pub fn context_cap(self, requested: Option<usize>) -> usize {
        requested
            .unwrap_or(20_000)
            .min(self.configured_max.saturating_sub(self.reserve))
            .max(1_000)
    }
    pub fn estimate(text: &str) -> usize {
        text.chars().count().div_ceil(4)
    }
}
pub fn ai_cache_key(
    content_hash: &str,
    analysis_type: &str,
    prompt_version: &str,
    provider: &str,
    model: &str,
) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    for value in [content_hash, analysis_type, prompt_version, provider, model] {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn build_context_pack(request: ContextPackRequest<'_>) -> ContextPack {
    let ContextPackRequest {
        project_id,
        graph,
        intent,
        task,
        explicit_ids,
        requested_tokens,
        library,
        features,
        findings,
    } = request;
    let budget = TokenBudgetManager::default().context_cap(requested_tokens);
    let mut ids = explicit_ids.to_vec();
    ids.extend(
        search(
            graph,
            &SearchQuery {
                q: Some(task.into()),
                limit: Some(16),
                ..Default::default()
            },
        )
        .into_iter()
        .map(|hit| hit.node.id),
    );
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    let mut selected = Vec::new();
    let mut evidence = Vec::new();
    let mut snippets = Vec::new();
    let mut tokens = 0usize;
    let mut truncated = false;
    for id in ids.into_iter().take(30) {
        let Some(node) = graph.find_node(&id) else {
            continue;
        };
        if !node.source_scope.library_eligible() {
            continue;
        }
        selected.push(ContextEntity {
            id: node.id.clone(),
            name: node.name.clone(),
            kind: node.kind.as_str().into(),
            path: node.path.clone(),
            reason: if explicit_ids.contains(&node.id) {
                "explicit selection".into()
            } else {
                "ranked task relevance".into()
            },
        });
        for edge in graph
            .incoming_edges(&node.id)
            .into_iter()
            .chain(graph.outgoing_edges(&node.id))
            .take(20)
        {
            evidence.push(format!(
                "{} --{}--> {}",
                edge.source_id,
                edge.relation.as_str(),
                edge.target_id
            ));
        }
        let Some(path) = node.path.as_deref() else {
            continue;
        };
        if sensitive_path(path) {
            continue;
        }
        let Ok(root) = Path::new(&graph.root).canonicalize() else {
            continue;
        };
        let Ok(absolute) = root.join(path).canonicalize() else {
            continue;
        };
        if !absolute.starts_with(&root) {
            continue;
        }
        let source = fs::read_to_string(absolute).unwrap_or_default();
        let start = node.start_line.unwrap_or(1);
        let end = node.end_line.unwrap_or(start).min(start + 120);
        let snippet = source
            .lines()
            .skip(start.saturating_sub(1))
            .take(end.saturating_sub(start) + 1)
            .map(redact_sensitive_line)
            .collect::<Vec<_>>()
            .join("\n");
        let estimate = TokenBudgetManager::estimate(&snippet);
        if tokens + estimate > budget {
            truncated = true;
            break;
        }
        tokens += estimate;
        snippets.push(ContextSnippet {
            node_id: node.id.clone(),
            path: path.into(),
            start_line: start,
            end_line: end,
            source: snippet,
        });
    }
    evidence.sort();
    evidence.dedup();
    evidence.truncate(200);
    let needle = task.to_lowercase();
    let library_matches = library
        .iter()
        .filter(|entry| {
            format!(
                "{} {} {}",
                entry.display_name,
                entry.description,
                entry.tags.join(" ")
            )
            .to_lowercase()
            .split_whitespace()
            .any(|term| needle.contains(term))
        })
        .take(10)
        .cloned()
        .collect();
    let feature_matches = features
        .iter()
        .filter(|feature| {
            needle.contains(&feature.name.to_lowercase())
                || feature
                    .name
                    .to_lowercase()
                    .split_whitespace()
                    .any(|term| needle.contains(term))
        })
        .take(10)
        .cloned()
        .collect::<Vec<_>>();
    let selected_ids = selected
        .iter()
        .map(|entity| entity.id.as_str())
        .collect::<HashSet<_>>();
    let relevant_findings = findings
        .iter()
        .filter(|finding| {
            finding
                .node_ids
                .iter()
                .any(|id| selected_ids.contains(id.as_str()))
        })
        .take(50)
        .cloned()
        .collect();
    let tests = selected
        .iter()
        .filter_map(|entity| entity.path.as_deref())
        .filter(|path| {
            let path = path.to_lowercase();
            path.contains("test") || path.contains("spec")
        })
        .map(str::to_owned)
        .collect();
    ContextPack {
        intent,
        project_id: project_id.into(),
        project_summary: format!(
            "{} nodes, {} edges, {} unresolved calls",
            graph.nodes.len(),
            graph.edges.len(),
            graph.unresolved_calls.len()
        ),
        selected_entities: selected,
        graph_evidence: evidence,
        source_snippets: snippets,
        tests,
        findings: relevant_findings,
        library_matches,
        feature_matches,
        token_estimate: tokens,
        max_tokens: budget,
        truncated,
        provenance: vec![
            "persisted ProjectGraph".into(),
            "first-party source excerpts".into(),
            "deterministic search and neighborhood selection".into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::project_graph::ProjectGraph;
    #[test]
    fn budget_never_exceeds_hard_cap() {
        let manager = TokenBudgetManager::default();
        assert_eq!(manager.context_cap(Some(900_000)), 180_000);
        let graph = ProjectGraph::new(".".into(), vec![], vec![], vec![], vec![], vec![]);
        let pack = build_context_pack(ContextPackRequest {
            project_id: "p",
            graph: &graph,
            intent: ContextIntent::PrepareTask,
            task: "task",
            explicit_ids: &[],
            requested_tokens: Some(900_000),
            library: &[],
            features: &[],
            findings: &[],
        });
        assert!(pack.max_tokens <= 180_000);
    }
}
