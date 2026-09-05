use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::model::{
    call::CallReference,
    edge::{CodeEdge, RelationKind},
    import::ImportReference,
    module::ResolvedModule,
    node::{CodeNode, NodeKind},
};

/// Navigable project graph with O(1)-average lookup indexes.
///
/// Indexes keep vector offsets instead of references. This avoids self-referential
/// data and lets the graph remain serializable for SQLite and the HTTP API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGraph {
    pub root: String,
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<CodeEdge>,
    pub imports: Vec<ImportReference>,
    pub modules: Vec<ResolvedModule>,
    pub unresolved_calls: Vec<CallReference>,
    #[serde(skip)]
    node_by_id: HashMap<String, usize>,
    #[serde(skip)]
    nodes_by_name: HashMap<String, Vec<usize>>,
    #[serde(skip)]
    nodes_by_path: HashMap<String, Vec<usize>>,
    #[serde(skip)]
    nodes_by_kind: HashMap<NodeKind, Vec<usize>>,
    #[serde(skip)]
    nodes_by_scope: HashMap<crate::model::project::SourceScope, Vec<usize>>,
    #[serde(skip)]
    outgoing_by_source: HashMap<String, Vec<usize>>,
    #[serde(skip)]
    incoming_by_target: HashMap<String, Vec<usize>>,
}

impl ProjectGraph {
    pub fn new(
        root: String,
        nodes: Vec<CodeNode>,
        edges: Vec<CodeEdge>,
        imports: Vec<ImportReference>,
        modules: Vec<ResolvedModule>,
        unresolved_calls: Vec<CallReference>,
    ) -> Self {
        let mut graph = Self {
            root,
            nodes,
            edges,
            imports,
            modules,
            unresolved_calls,
            node_by_id: HashMap::new(),
            nodes_by_name: HashMap::new(),
            nodes_by_path: HashMap::new(),
            nodes_by_kind: HashMap::new(),
            nodes_by_scope: HashMap::new(),
            outgoing_by_source: HashMap::new(),
            incoming_by_target: HashMap::new(),
        };
        graph.rebuild_indexes();
        graph
    }

    pub fn rebuild_indexes(&mut self) {
        self.node_by_id.clear();
        self.nodes_by_name.clear();
        self.nodes_by_path.clear();
        self.nodes_by_kind.clear();
        self.nodes_by_scope.clear();
        self.outgoing_by_source.clear();
        self.incoming_by_target.clear();
        for (index, node) in self.nodes.iter().enumerate() {
            self.node_by_id.insert(node.id.clone(), index);
            self.nodes_by_name
                .entry(node.name.to_lowercase())
                .or_default()
                .push(index);
            if let Some(path) = &node.path {
                self.nodes_by_path
                    .entry(path.clone())
                    .or_default()
                    .push(index);
            }
            self.nodes_by_kind.entry(node.kind).or_default().push(index);
            self.nodes_by_scope
                .entry(node.source_scope)
                .or_default()
                .push(index);
        }
        for (index, edge) in self.edges.iter().enumerate() {
            self.outgoing_by_source
                .entry(edge.source_id.clone())
                .or_default()
                .push(index);
            self.incoming_by_target
                .entry(edge.target_id.clone())
                .or_default()
                .push(index);
        }
    }

    pub fn find_node(&self, node_id: &str) -> Option<&CodeNode> {
        self.node_by_id
            .get(node_id)
            .and_then(|index| self.nodes.get(*index))
    }
    pub fn find_nodes_by_name(&self, name: &str) -> Vec<&CodeNode> {
        self.nodes_by_name
            .get(&name.to_lowercase())
            .into_iter()
            .flatten()
            .filter_map(|index| self.nodes.get(*index))
            .collect()
    }
    pub fn find_nodes_by_kind(&self, kind: NodeKind) -> Vec<&CodeNode> {
        self.nodes_by_kind
            .get(&kind)
            .into_iter()
            .flatten()
            .filter_map(|index| self.nodes.get(*index))
            .collect()
    }
    pub fn find_nodes_by_scope(&self, scope: crate::model::project::SourceScope) -> Vec<&CodeNode> {
        self.nodes_by_scope
            .get(&scope)
            .into_iter()
            .flatten()
            .filter_map(|index| self.nodes.get(*index))
            .collect()
    }
    pub fn find_nodes_by_path(&self, path: &str) -> Vec<&CodeNode> {
        self.nodes_by_path
            .get(path)
            .into_iter()
            .flatten()
            .filter_map(|index| self.nodes.get(*index))
            .collect()
    }
    pub fn outgoing_edges(&self, node_id: &str) -> Vec<&CodeEdge> {
        self.outgoing_by_source
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|index| self.edges.get(*index))
            .collect()
    }
    pub fn incoming_edges(&self, node_id: &str) -> Vec<&CodeEdge> {
        self.incoming_by_target
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|index| self.edges.get(*index))
            .collect()
    }
    pub fn outgoing_edges_by_relation(
        &self,
        node_id: &str,
        relation: RelationKind,
    ) -> Vec<&CodeEdge> {
        self.outgoing_edges(node_id)
            .into_iter()
            .filter(|edge| edge.relation == relation)
            .collect()
    }
    pub fn incoming_edges_by_relation(
        &self,
        node_id: &str,
        relation: RelationKind,
    ) -> Vec<&CodeEdge> {
        self.incoming_edges(node_id)
            .into_iter()
            .filter(|edge| edge.relation == relation)
            .collect()
    }
    pub fn related_nodes_from(&self, node_id: &str) -> Vec<&CodeNode> {
        self.outgoing_edges(node_id)
            .into_iter()
            .filter_map(|edge| self.find_node(&edge.target_id))
            .collect()
    }
    pub fn related_nodes_to(&self, node_id: &str) -> Vec<&CodeNode> {
        self.incoming_edges(node_id)
            .into_iter()
            .filter_map(|edge| self.find_node(&edge.source_id))
            .collect()
    }
    pub fn count_nodes_by_kind(&self, kind: NodeKind) -> usize {
        self.nodes.iter().filter(|node| node.kind == kind).count()
    }

    pub fn neighbors(&self, node_id: &str) -> Vec<&CodeNode> {
        let mut seen = HashSet::new();
        self.related_nodes_from(node_id)
            .into_iter()
            .chain(self.related_nodes_to(node_id))
            .filter(|node| seen.insert(node.id.as_str()))
            .collect()
    }

    /// Breadth-first traversal. Incoming traversal powers impact analysis; outgoing
    /// traversal explains a code flow. `visited` prevents cycles from looping.
    pub fn traverse(&self, start: &str, depth: usize, incoming: bool) -> Vec<(usize, &CodeNode)> {
        let mut result = Vec::new();
        let mut visited = HashSet::from([start]);
        let mut queue = VecDeque::from([(start, 0_usize)]);
        while let Some((current, level)) = queue.pop_front() {
            if level >= depth {
                continue;
            }
            let edges = if incoming {
                self.incoming_edges(current)
            } else {
                self.outgoing_edges(current)
            };
            for edge in edges {
                let next = if incoming {
                    &edge.source_id
                } else {
                    &edge.target_id
                };
                if visited.insert(next.as_str())
                    && let Some(node) = self.find_node(next)
                {
                    result.push((level + 1, node));
                    queue.push_back((next, level + 1));
                }
            }
        }
        result
    }

    pub fn subgraph(&self, node_ids: &HashSet<String>) -> Self {
        let nodes = self
            .nodes
            .iter()
            .filter(|node| node_ids.contains(&node.id))
            .cloned()
            .collect();
        let edges = self
            .edges
            .iter()
            .filter(|edge| node_ids.contains(&edge.source_id) && node_ids.contains(&edge.target_id))
            .cloned()
            .collect();
        Self::new(
            self.root.clone(),
            nodes,
            edges,
            self.imports.clone(),
            self.modules.clone(),
            self.unresolved_calls.clone(),
        )
    }
}
