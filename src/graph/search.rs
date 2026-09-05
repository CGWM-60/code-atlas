use crate::{
    graph::project_graph::ProjectGraph,
    model::node::{CodeNode, NodeKind},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub kind: Option<NodeKind>,
    pub language: Option<String>,
    pub path: Option<String>,
    pub limit: Option<usize>,
}
#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub score: usize,
    pub node: CodeNode,
}

pub fn search(graph: &ProjectGraph, query: &SearchQuery) -> Vec<SearchHit> {
    let needle = query.q.as_deref().unwrap_or_default().trim().to_lowercase();
    let mut hits: Vec<_> = graph
        .nodes
        .iter()
        .filter_map(|node| {
            if query.kind.is_some_and(|kind| node.kind != kind) {
                return None;
            }
            if query.language.as_ref().is_some_and(|lang| {
                node.language.map(|v| v.as_str().to_lowercase()) != Some(lang.to_lowercase())
            }) {
                return None;
            }
            if query
                .path
                .as_ref()
                .is_some_and(|path| !node.path.as_deref().unwrap_or_default().contains(path))
            {
                return None;
            }
            let name = node.name.to_lowercase();
            let path = node.path.as_deref().unwrap_or_default().to_lowercase();
            let score = if needle.is_empty() {
                1
            } else if name == needle {
                100
            } else if name.starts_with(&needle) {
                80
            } else if name.contains(&needle) {
                60
            } else if path.contains(&needle) {
                35
            } else if subsequence(&needle, &name) {
                15
            } else {
                return None;
            };
            Some(SearchHit {
                score,
                node: node.clone(),
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.node.name.cmp(&b.node.name))
    });
    hits.truncate(query.limit.unwrap_or(100).min(500));
    hits
}
fn subsequence(needle: &str, value: &str) -> bool {
    let mut chars = needle.chars();
    let mut current = chars.next();
    for ch in value.chars() {
        if current == Some(ch) {
            current = chars.next();
        }
        if current.is_none() {
            return true;
        }
    }
    current.is_none()
}
