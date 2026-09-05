use crate::{
    graph::{
        project_graph::ProjectGraph,
        search::{SearchQuery, search},
    },
    model::{node::CodeNode, project::SourceScope},
};
use std::{collections::HashSet, fs, path::Path};

pub const MAX_CONTEXT_BYTES: usize = 40_000;
const MAX_CONTEXT_NODES: usize = 20;
const MAX_SNIPPET_BYTES: usize = 12_000;
const MAX_SOURCE_LINE_CHARS: usize = 600;

#[derive(Debug, Clone)]
pub struct RetrievedContext {
    pub text: String,
    pub citations: Vec<String>,
}
pub fn build_context(
    graph: &ProjectGraph,
    question: &str,
    focus: Option<&str>,
) -> RetrievedContext {
    let mut selected: Vec<CodeNode> = focus
        .and_then(|id| graph.find_node(id))
        .into_iter()
        .cloned()
        .collect();
    selected.extend(
        search(
            graph,
            &SearchQuery {
                q: Some(question.into()),
                limit: Some(8),
                ..Default::default()
            },
        )
        .into_iter()
        .map(|hit| hit.node),
    );
    if let Some(id) = focus {
        selected.extend(graph.neighbors(id).into_iter().take(12).cloned());
    }
    // Preserve retrieval order: explicit focus, ranked search hits, then neighbors.
    let mut seen = HashSet::new();
    selected.retain(|node| seen.insert(node.id.clone()));
    let mut text = String::new();
    let mut citations = Vec::new();
    for node in selected.into_iter().take(MAX_CONTEXT_NODES) {
        let Some(path) = node.path.as_deref() else {
            continue;
        };
        if sensitive_path(path)
            || !matches!(
                node.source_scope,
                SourceScope::Project | SourceScope::WorkspacePackage
            )
        {
            continue;
        }
        let start = node.start_line.unwrap_or(1).saturating_sub(3).max(1);
        let end = node
            .end_line
            .unwrap_or(start)
            .saturating_add(3)
            .min(start + 120);
        let Ok(root) = Path::new(&graph.root).canonicalize() else {
            continue;
        };
        let Ok(absolute) = root.join(path).canonicalize() else {
            continue;
        };
        if !absolute.starts_with(&root) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&absolute) else {
            continue;
        };
        let mut snippet = String::new();
        let mut last_line = start;
        for (index, line) in source
            .lines()
            .enumerate()
            .filter(|(index, _)| *index + 1 >= start && *index < end)
        {
            let safe_line = redact_sensitive_line(line);
            let bounded_line = safe_line
                .chars()
                .take(MAX_SOURCE_LINE_CHARS)
                .collect::<String>();
            let suffix = if line.chars().count() > MAX_SOURCE_LINE_CHARS {
                " …[line truncated]"
            } else {
                ""
            };
            let rendered = format!("{:>5} | {bounded_line}{suffix}\n", index + 1);
            if snippet.len() + rendered.len() > MAX_SNIPPET_BYTES {
                break;
            }
            snippet.push_str(&rendered);
            last_line = index + 1;
        }
        if snippet.is_empty() {
            continue;
        }
        let citation = format!("{}:{}-{}", path, start, last_line);
        let block = format!(
            "\nSOURCE {citation}\nNODE {:?} {}\n{snippet}\n",
            node.kind, node.name
        );
        if text.len() + block.len() > MAX_CONTEXT_BYTES {
            break;
        }
        citations.push(citation);
        text.push_str(&block);
    }
    RetrievedContext { text, citations }
}
pub(crate) fn sensitive_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".env")
        || lower.contains("private_key")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.contains("credential")
        || lower.contains("secret")
}
pub(crate) fn redact_sensitive_line(line: &str) -> String {
    let lower = line.to_lowercase();
    let looks_like_assignment = line.contains('=') || line.contains(':');
    let sensitive_name = [
        "api_key",
        "apikey",
        "private_key",
        "client_secret",
        "password",
        "passwd",
        "access_token",
        "auth_token",
    ]
    .iter()
    .any(|name| lower.contains(name));
    if looks_like_assignment && sensitive_name {
        "[REDACTED SENSITIVE ASSIGNMENT]".into()
    } else {
        line.into()
    }
}
