use crate::{graph::project_graph::ProjectGraph, model::node::CodeNode};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ImpactLevel {
    pub depth: usize,
    pub nodes: Vec<CodeNode>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ImpactResult {
    pub source: CodeNode,
    pub direct: Vec<CodeNode>,
    pub transitive: Vec<ImpactLevel>,
}

pub fn analyze_impact(graph: &ProjectGraph, node_id: &str, depth: usize) -> Option<ImpactResult> {
    let source = graph.find_node(node_id)?.clone();
    let max_depth = depth.clamp(1, 20);
    let traversed = graph.traverse(node_id, max_depth, true);
    let direct = traversed
        .iter()
        .filter(|(level, _)| *level == 1)
        .map(|(_, node)| (*node).clone())
        .collect();
    let transitive = (2..=max_depth)
        .map(|level| ImpactLevel {
            depth: level,
            nodes: traversed
                .iter()
                .filter(|(item_level, _)| *item_level == level)
                .map(|(_, node)| (*node).clone())
                .collect(),
        })
        .collect();
    Some(ImpactResult {
        source,
        direct,
        transitive,
    })
}
