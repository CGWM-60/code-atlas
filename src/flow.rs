use crate::{
    graph::{architecture::classify_role, project_graph::ProjectGraph},
    model::{
        edge::{CodeEdge, RelationKind},
        node::{CodeNode, NodeKind},
        project::SourceScope,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, HashSet},
};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowDirection {
    Downstream,
    Upstream,
    Between,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlowTraceRequest {
    pub start_node_id: String,
    pub end_node_id: Option<String>,
    pub direction: FlowDirection,
    #[serde(default = "default_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub relations: Vec<String>,
}

fn default_depth() -> usize {
    8
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowStep {
    pub index: usize,
    pub node: CodeNode,
    pub relation_from_previous: Option<String>,
    pub cumulative_cost: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowTrace {
    pub found: bool,
    pub direction: String,
    pub start_node_id: String,
    pub end_node_id: Option<String>,
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<CodeEdge>,
    pub steps: Vec<FlowStep>,
    pub total_cost: u32,
    pub message: String,
}

fn relation_weight(relation: RelationKind) -> u32 {
    match relation {
        RelationKind::RoutesTo | RelationKind::HandledBy => 2,
        RelationKind::Calls => 3,
        RelationKind::Reads | RelationKind::Writes => 4,
        RelationKind::Uses | RelationKind::Renders => 5,
        RelationKind::Creates | RelationKind::Emits | RelationKind::Listens => 6,
        RelationKind::DependsOn | RelationKind::Returns => 8,
        RelationKind::Imports => 14,
        RelationKind::Contains | RelationKind::Declares | RelationKind::HasMethod => 40,
        _ => 18,
    }
}

fn node_penalty(node: &CodeNode) -> u32 {
    let mut penalty = 0;
    if node.source_scope == SourceScope::ExternalDependency {
        penalty += 25;
    }
    if matches!(
        node.kind,
        NodeKind::Infrastructure | NodeKind::Config | NodeKind::Environment
    ) {
        penalty += 12;
    }
    if node.path.as_deref().is_some_and(|path| {
        let path = path.to_lowercase();
        path.contains("test") || path.contains("spec")
    }) {
        penalty += 15;
    }
    penalty
}

fn allowed(relation: RelationKind, filters: &HashSet<String>) -> bool {
    filters.is_empty() || filters.contains(&relation.as_str().to_lowercase())
}

pub fn trace_flow(graph: &ProjectGraph, request: &FlowTraceRequest) -> FlowTrace {
    if graph.find_node(&request.start_node_id).is_none() {
        return not_found(request, "Le nœud de départ n’existe pas dans ce projet.");
    }
    if matches!(request.direction, FlowDirection::Between)
        && request
            .end_node_id
            .as_deref()
            .is_none_or(|id| graph.find_node(id).is_none())
    {
        return not_found(
            request,
            "Le mode Between nécessite un nœud d’arrivée valide.",
        );
    }
    let incoming = matches!(request.direction, FlowDirection::Upstream);
    let filters = request
        .relations
        .iter()
        .map(|relation| relation.to_lowercase())
        .collect::<HashSet<_>>();
    let depth_limit = request.max_depth.clamp(1, 20);
    let mut queue = BinaryHeap::from([Reverse((0_u32, 0_usize, request.start_node_id.clone()))]);
    let mut distances = HashMap::from([(request.start_node_id.clone(), 0_u32)]);
    let mut depths = HashMap::from([(request.start_node_id.clone(), 0_usize)]);
    let mut previous = HashMap::<String, (String, String)>::new();
    while let Some(Reverse((cost, depth, current))) = queue.pop() {
        if cost > *distances.get(&current).unwrap_or(&u32::MAX) || depth >= depth_limit {
            continue;
        }
        if request.end_node_id.as_deref() == Some(current.as_str()) {
            break;
        }
        let edges = if incoming {
            graph.incoming_edges(&current)
        } else {
            graph.outgoing_edges(&current)
        };
        for edge in edges {
            if !allowed(edge.relation, &filters) {
                continue;
            }
            let next = if incoming {
                &edge.source_id
            } else {
                &edge.target_id
            };
            let Some(node) = graph.find_node(next) else {
                continue;
            };
            let next_cost = cost + relation_weight(edge.relation) + node_penalty(node);
            if next_cost < *distances.get(next).unwrap_or(&u32::MAX) {
                distances.insert(next.clone(), next_cost);
                depths.insert(next.clone(), depth + 1);
                previous.insert(next.clone(), (current.clone(), edge.id.clone()));
                queue.push(Reverse((next_cost, depth + 1, next.clone())));
            }
        }
    }
    let target = request
        .end_node_id
        .as_ref()
        .filter(|target| distances.contains_key(*target))
        .cloned()
        .or_else(|| select_terminal(graph, &distances, &depths, incoming, &request.start_node_id));
    let Some(target) = target else {
        return not_found(
            request,
            "Aucun chemin significatif n’a été trouvé avec ces relations et cette profondeur.",
        );
    };
    build_trace(graph, request, target, distances, previous)
}

fn select_terminal(
    graph: &ProjectGraph,
    distances: &HashMap<String, u32>,
    depths: &HashMap<String, usize>,
    incoming: bool,
    start: &str,
) -> Option<String> {
    distances
        .keys()
        .filter(|id| id.as_str() != start)
        .filter_map(|id| {
            let node = graph.find_node(id)?;
            let role = classify_role(node);
            let terminal_bonus = if incoming {
                match node.kind {
                    NodeKind::Page
                    | NodeKind::Route
                    | NodeKind::ApiEndpoint
                    | NodeKind::EntryPoint => 80_i64,
                    _ => 0,
                }
            } else {
                match node.kind {
                    NodeKind::Repository | NodeKind::Database | NodeKind::DatabaseTable => 100,
                    NodeKind::ExternalService | NodeKind::Queue | NodeKind::Event => 70,
                    _ => match role.zone() {
                        "PERSISTENCE" => 90,
                        "EXTERNAL SERVICES" => 60,
                        _ => 0,
                    },
                }
            };
            let depth = *depths.get(id).unwrap_or(&0) as i64;
            let cost = *distances.get(id).unwrap_or(&u32::MAX) as i64;
            Some((terminal_bonus + depth * 12 - cost / 4, id.clone()))
        })
        .max()
        .map(|(_, id)| id)
}

fn build_trace(
    graph: &ProjectGraph,
    request: &FlowTraceRequest,
    target: String,
    distances: HashMap<String, u32>,
    previous: HashMap<String, (String, String)>,
) -> FlowTrace {
    let mut ids = vec![target.clone()];
    let mut edge_ids = Vec::new();
    let mut current = target.clone();
    while current != request.start_node_id {
        let Some((parent, edge)) = previous.get(&current) else {
            return not_found(request, "Le chemin trouvé est incomplet.");
        };
        edge_ids.push(edge.clone());
        ids.push(parent.clone());
        current = parent.clone();
    }
    ids.reverse();
    edge_ids.reverse();
    let nodes = ids
        .iter()
        .filter_map(|id| graph.find_node(id).cloned())
        .collect::<Vec<_>>();
    let edges = edge_ids
        .iter()
        .filter_map(|id| graph.edges.iter().find(|edge| &edge.id == id).cloned())
        .collect::<Vec<_>>();
    let steps = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| FlowStep {
            index: index + 1,
            node: node.clone(),
            relation_from_previous: index
                .checked_sub(1)
                .and_then(|edge_index| edges.get(edge_index))
                .map(|edge| edge.relation.as_str().into()),
            cumulative_cost: *distances.get(&node.id).unwrap_or(&0),
        })
        .collect();
    FlowTrace {
        found: true,
        direction: format!("{:?}", request.direction).to_lowercase(),
        start_node_id: request.start_node_id.clone(),
        end_node_id: Some(target.clone()),
        total_cost: *distances.get(&target).unwrap_or(&0),
        nodes,
        edges,
        steps,
        message: "Chemin pondéré construit depuis les relations sémantiques du graphe.".into(),
    }
}

fn not_found(request: &FlowTraceRequest, message: &str) -> FlowTrace {
    FlowTrace {
        found: false,
        direction: format!("{:?}", request.direction).to_lowercase(),
        start_node_id: request.start_node_id.clone(),
        end_node_id: request.end_node_id.clone(),
        nodes: vec![],
        edges: vec![],
        steps: vec![],
        total_cost: 0,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::node::CodeNode;

    fn node(id: &str, kind: NodeKind) -> CodeNode {
        CodeNode {
            id: id.into(),
            kind,
            name: id.into(),
            path: Some(format!("{id}.rs")),
            language: None,
            start_line: Some(1),
            end_line: Some(2),
            owner: None,
            source_scope: SourceScope::Project,
        }
    }
    fn edge(id: &str, source: &str, target: &str, relation: RelationKind) -> CodeEdge {
        CodeEdge {
            id: id.into(),
            source_id: source.into(),
            target_id: target.into(),
            relation,
        }
    }

    #[test]
    fn traces_route_to_repository_and_avoids_contains_shortcut() {
        let graph = ProjectGraph::new(
            ".".into(),
            vec![
                node("route", NodeKind::ApiEndpoint),
                node("handler", NodeKind::Handler),
                node("repo", NodeKind::Repository),
            ],
            vec![
                edge("contains", "route", "repo", RelationKind::Contains),
                edge("handled", "route", "handler", RelationKind::HandledBy),
                edge("calls", "handler", "repo", RelationKind::Calls),
            ],
            vec![],
            vec![],
            vec![],
        );
        let result = trace_flow(
            &graph,
            &FlowTraceRequest {
                start_node_id: "route".into(),
                end_node_id: Some("repo".into()),
                direction: FlowDirection::Between,
                max_depth: 6,
                relations: vec![],
            },
        );
        assert!(result.found);
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges[0].relation, RelationKind::HandledBy);
    }

    #[test]
    fn missing_path_is_graceful() {
        let graph = ProjectGraph::new(
            ".".into(),
            vec![node("a", NodeKind::Page), node("b", NodeKind::Repository)],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let result = trace_flow(
            &graph,
            &FlowTraceRequest {
                start_node_id: "a".into(),
                end_node_id: Some("b".into()),
                direction: FlowDirection::Between,
                max_depth: 4,
                relations: vec![],
            },
        );
        assert!(!result.found);
    }
}
