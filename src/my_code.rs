use crate::{
    graph::project_graph::ProjectGraph,
    library::candidate_for,
    model::{edge::RelationKind, node::CodeNode},
};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
pub struct MyCodeItem {
    pub node: CodeNode,
    pub incoming_usages: usize,
    pub outgoing_dependencies: usize,
    pub tests: usize,
    pub complexity: Option<usize>,
    pub coupling: usize,
    pub reuse_score: Option<u8>,
    pub knowledge_value: Option<u8>,
    pub library_status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct MyCodePage {
    pub items: Vec<MyCodeItem>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

pub fn list_my_code(
    graph: &ProjectGraph,
    library_node_ids: &HashSet<String>,
    query: Option<&str>,
    kind: Option<&str>,
    offset: usize,
    limit: usize,
) -> MyCodePage {
    let needle = query.unwrap_or_default().trim().to_lowercase();
    let requested_kind = kind.unwrap_or("all").to_lowercase();
    let mut nodes = graph
        .nodes
        .iter()
        .filter(|node| node.source_scope.library_eligible())
        .filter(|node| node.path.is_some())
        .filter(|node| {
            let special = match requested_kind.as_str() {
                "candidates" => candidate_for(graph, node).is_some(),
                "untested" => {
                    matches!(
                        node.kind,
                        crate::model::node::NodeKind::Function
                            | crate::model::node::NodeKind::Method
                            | crate::model::node::NodeKind::Class
                            | crate::model::node::NodeKind::Service
                    ) && !graph
                        .incoming_edges(&node.id)
                        .iter()
                        .filter_map(|edge| graph.find_node(&edge.source_id))
                        .any(|caller| {
                            caller.path.as_deref().is_some_and(|path| {
                                let path = path.to_lowercase();
                                path.contains("test") || path.contains("spec")
                            })
                        })
                }
                "complex" => {
                    candidate_for(graph, node).is_some_and(|candidate| candidate.complexity >= 10)
                }
                "high_coupling" => graph.outgoing_edges(&node.id).len() >= 10,
                "recently_modified" => node
                    .path
                    .as_deref()
                    .and_then(|path| {
                        std::fs::metadata(std::path::Path::new(&graph.root).join(path)).ok()
                    })
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| std::time::SystemTime::now().duration_since(modified).ok())
                    .is_some_and(|age| age.as_secs() <= 7 * 24 * 60 * 60),
                _ => false,
            };
            requested_kind == "all"
                || special
                || node.kind.as_str().to_lowercase() == requested_kind
                || (requested_kind == "functions"
                    && matches!(node.kind, crate::model::node::NodeKind::Function))
                || (requested_kind == "methods"
                    && matches!(node.kind, crate::model::node::NodeKind::Method))
                || (requested_kind == "classes"
                    && matches!(node.kind, crate::model::node::NodeKind::Class))
        })
        .filter(|node| {
            needle.is_empty()
                || format!("{} {}", node.name, node.path.as_deref().unwrap_or_default())
                    .to_lowercase()
                    .contains(&needle)
        })
        .collect::<Vec<_>>();
    nodes.sort_by_key(|node| (node.path.as_deref().unwrap_or_default(), node.start_line));
    let total = nodes.len();
    let items = nodes
        .into_iter()
        .skip(offset)
        .take(limit.clamp(1, 200))
        .map(|node| {
            let incoming = graph.incoming_edges(&node.id);
            let outgoing = graph.outgoing_edges(&node.id);
            let tests = incoming
                .iter()
                .filter(|edge| edge.relation == RelationKind::Calls)
                .filter_map(|edge| graph.find_node(&edge.source_id))
                .filter(|caller| {
                    caller.path.as_deref().is_some_and(|path| {
                        let path = path.to_lowercase();
                        path.contains("test") || path.contains("spec")
                    })
                })
                .count();
            let candidate = candidate_for(graph, node);
            MyCodeItem {
                node: node.clone(),
                incoming_usages: incoming
                    .iter()
                    .filter(|edge| edge.relation == RelationKind::Calls)
                    .count(),
                outgoing_dependencies: outgoing
                    .iter()
                    .filter(|edge| {
                        matches!(
                            edge.relation,
                            RelationKind::Calls
                                | RelationKind::Uses
                                | RelationKind::Imports
                                | RelationKind::DependsOn
                        )
                    })
                    .count(),
                tests,
                complexity: candidate.as_ref().map(|candidate| candidate.complexity),
                coupling: outgoing.len(),
                reuse_score: candidate.as_ref().map(|candidate| candidate.reuse_score),
                knowledge_value: candidate
                    .as_ref()
                    .map(|candidate| candidate.knowledge_value),
                library_status: if library_node_ids.contains(&node.id) {
                    "saved"
                } else if candidate.is_some() {
                    "candidate"
                } else {
                    "not_eligible"
                },
            }
        })
        .collect();
    MyCodePage {
        items,
        total,
        offset,
        limit: limit.clamp(1, 200),
    }
}

#[cfg(test)]
mod tests {
    use super::list_my_code;
    use crate::{
        graph::project_graph::ProjectGraph,
        model::{
            node::{CodeNode, NodeKind},
            project::SourceScope,
        },
    };
    use std::collections::HashSet;

    #[test]
    fn excludes_external_and_generated_nodes() {
        let node = |id: &str, scope| CodeNode {
            id: id.into(),
            kind: NodeKind::Function,
            name: id.into(),
            path: Some(format!("{id}.rs")),
            language: None,
            start_line: Some(1),
            end_line: Some(2),
            owner: None,
            source_scope: scope,
        };
        let graph = ProjectGraph::new(
            ".".into(),
            vec![
                node("ours", SourceScope::Project),
                node("dependency", SourceScope::ExternalDependency),
                node("generated", SourceScope::Generated),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let page = list_my_code(&graph, &HashSet::new(), None, None, 0, 50);
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].node.name, "ours");
    }
}
