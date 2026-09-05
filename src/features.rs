use crate::{
    findings::Finding,
    graph::{
        architecture::{ArchitectureRole, classify_role, detect_domain},
        project_graph::ProjectGraph,
    },
    model::{
        edge::RelationKind,
        node::{CodeNode, NodeKind},
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    #[default]
    Detected,
    Accepted,
    Edited,
    Ignored,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipSource {
    Deterministic,
    Ai,
    User,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMembership {
    pub node_id: String,
    pub confidence: f32,
    pub reason: String,
    pub source: MembershipSource,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub external_dependency: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub confidence: f32,
    #[serde(default)]
    pub completeness: f32,
    #[serde(default)]
    pub missing_signals: Vec<String>,
    pub status: FeatureStatus,
    pub entry_point_node_ids: Vec<String>,
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub memberships: Vec<FeatureMembership>,
    pub routes: Vec<String>,
    pub pages: Vec<String>,
    pub services: Vec<String>,
    pub repositories: Vec<String>,
    pub models: Vec<String>,
    pub entities: Vec<String>,
    pub templates: Vec<String>,
    pub tests: Vec<String>,
    pub external_services: Vec<String>,
    pub business_rules: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub side_effects: Vec<String>,
    pub security_constraints: Vec<String>,
    pub source_hash: String,
    pub ai_provider: Option<String>,
    pub ai_model: Option<String>,
    pub prompt_version: String,
    pub created_at: i64,
    pub updated_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FeatureSpec {
    pub name: String,
    pub description: String,
    pub actors: Vec<String>,
    pub entry_points: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub business_rules: Vec<String>,
    pub operations: Vec<String>,
    pub data: Vec<String>,
    pub side_effects: Vec<String>,
    pub security: Vec<String>,
    pub error_cases: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub dependencies: Vec<String>,
    pub provenance: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct FeatureCodeSnippet {
    pub project_id: String,
    pub node_id: String,
    pub name: String,
    pub language: Option<String>,
    pub architecture_role: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub source: String,
    pub source_hash: String,
    pub membership_reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureSourceFile {
    pub path: String,
    pub language: Option<String>,
    pub architecture_role: String,
    pub source_hash: String,
    pub fragments: Vec<FeatureCodeSnippet>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureSourceSearchHit {
    pub path: String,
    pub node_id: String,
    pub symbol: String,
    pub line: usize,
    pub preview: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct FeatureBundle {
    pub feature: Feature,
    pub spec: FeatureSpec,
    pub snippets: Vec<FeatureCodeSnippet>,
    pub findings: Vec<Finding>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureLibraryEntry {
    pub id: String,
    pub source_feature_id: String,
    pub source_project_id: String,
    pub name: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub source_hash: String,
    pub spec: FeatureSpec,
    pub documentation: String,
    pub acceptance_criteria: Vec<String>,
    pub security_notes: Vec<String>,
    pub architecture_summary: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDocumentation {
    pub feature_id: String,
    pub purpose: String,
    pub actors: Vec<String>,
    pub entry_points: Vec<String>,
    pub flow: Vec<String>,
    pub business_rules: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub data: Vec<String>,
    pub side_effects: Vec<String>,
    pub security: Vec<String>,
    pub error_cases: Vec<String>,
    pub tests: Vec<String>,
    pub dependencies: Vec<String>,
    pub known_limitations: Vec<String>,
    pub content_hash: String,
    pub provenance: String,
}

pub fn build_feature_documentation(
    graph: &ProjectGraph,
    feature: &Feature,
    spec: &FeatureSpec,
) -> FeatureDocumentation {
    let flow = feature
        .edge_ids
        .iter()
        .filter_map(|id| graph.edges.iter().find(|edge| &edge.id == id))
        .take(100)
        .map(|edge| {
            format!(
                "{} —{}→ {}",
                edge.source_id,
                edge.relation.as_str(),
                edge.target_id
            )
        })
        .collect();
    FeatureDocumentation {
        feature_id: feature.id.clone(),
        purpose: spec.description.clone(),
        actors: spec.actors.clone(),
        entry_points: spec.entry_points.clone(),
        flow,
        business_rules: spec.business_rules.clone(),
        inputs: spec.inputs.clone(),
        outputs: spec.outputs.clone(),
        data: spec.data.clone(),
        side_effects: spec.side_effects.clone(),
        security: spec.security.clone(),
        error_cases: spec.error_cases.clone(),
        tests: spec.acceptance_criteria.clone(),
        dependencies: spec.dependencies.clone(),
        known_limitations: if feature.ai_provider.is_none() {
            vec!["Semantic decomposition has not been AI-enriched or user-accepted.".into()]
        } else {
            vec![]
        },
        content_hash: feature.source_hash.clone(),
        provenance: spec.provenance.clone(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiFeatureProposal {
    pub candidate_id: String,
    pub name: String,
    pub description: String,
    pub member_node_ids: Vec<String>,
    pub entry_point_node_ids: Vec<String>,
    pub business_rules: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub side_effects: Vec<String>,
    pub security_constraints: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiFeatureProposals {
    pub features: Vec<AiFeatureProposal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiFeatureCompletenessProposal {
    pub feature_id: String,
    pub additional_node_ids: Vec<String>,
    pub missing_signals: Vec<String>,
    pub completeness: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiFeatureCompletenessProposals {
    pub features: Vec<AiFeatureCompletenessProposal>,
}

pub fn feature_proposals_schema() -> serde_json::Value {
    let string_array = || serde_json::json!({"type":"array","items":{"type":"string"}});
    serde_json::json!({
        "type":"object","additionalProperties":false,"required":["features"],
        "properties":{"features":{"type":"array","items":{
            "type":"object","additionalProperties":false,
            "required":["candidate_id","name","description","member_node_ids","entry_point_node_ids","business_rules","inputs","outputs","side_effects","security_constraints","confidence"],
            "properties":{
                "candidate_id":{"type":"string"},"name":{"type":"string"},"description":{"type":"string"},
                "member_node_ids":string_array(),"entry_point_node_ids":string_array(),
                "business_rules":string_array(),"inputs":string_array(),"outputs":string_array(),
                "side_effects":string_array(),"security_constraints":string_array(),
                "confidence":{"type":"number","minimum":0,"maximum":1}
            }
        }}}
    })
}

pub fn feature_completeness_schema() -> serde_json::Value {
    let string_array = || serde_json::json!({"type":"array","items":{"type":"string"}});
    serde_json::json!({
        "type":"object","additionalProperties":false,"required":["features"],
        "properties":{"features":{"type":"array","items":{
            "type":"object","additionalProperties":false,
            "required":["feature_id","additional_node_ids","missing_signals","completeness"],
            "properties":{
                "feature_id":{"type":"string"},
                "additional_node_ids":string_array(),
                "missing_signals":string_array(),
                "completeness":{"type":"number","minimum":0,"maximum":1}
            }
        }}}
    })
}

pub fn compact_feature_completeness(
    graph: &ProjectGraph,
    features: &[Feature],
) -> serde_json::Value {
    serde_json::json!({"features":features.iter().take(60).map(|feature| {
        let members = feature.node_ids.iter().cloned().collect::<HashSet<_>>();
        let mut nearby = BTreeMap::<String, serde_json::Value>::new();
        for edge in &graph.edges {
            let candidate_id = if members.contains(&edge.source_id) && !members.contains(&edge.target_id) {
                Some((&edge.target_id, &edge.source_id))
            } else if members.contains(&edge.target_id) && !members.contains(&edge.source_id) {
                Some((&edge.source_id, &edge.target_id))
            } else { None };
            let Some((candidate_id, neighbor_id)) = candidate_id else { continue };
            let Some(node) = graph.find_node(candidate_id) else { continue };
            if !node.source_scope.library_eligible() { continue; }
            nearby.entry(candidate_id.clone()).or_insert_with(|| serde_json::json!({
                "id":node.id,"name":node.name,"kind":node.kind.as_str(),"path":node.path,
                "relation":edge.relation.as_str(),"neighbor_id":neighbor_id
            }));
            if nearby.len() >= 40 { break; }
        }
        serde_json::json!({
            "feature_id":feature.id,
            "name":feature.name,
            "deterministic_completeness":feature.completeness,
            "missing_signals":feature.missing_signals,
            "members":feature.node_ids,
            "nearby_excluded":nearby.into_values().collect::<Vec<_>>()
        })
    }).collect::<Vec<_>>()})
}

pub fn apply_ai_feature_completeness(
    graph: &ProjectGraph,
    features: &mut [Feature],
    proposals: AiFeatureCompletenessProposals,
) -> Result<(), String> {
    for proposal in proposals.features {
        let feature = features
            .iter_mut()
            .find(|feature| feature.id == proposal.feature_id)
            .ok_or_else(|| format!("unknown feature {}", proposal.feature_id))?;
        let existing = feature.node_ids.iter().cloned().collect::<HashSet<_>>();
        for node_id in proposal.additional_node_ids {
            let node = graph
                .find_node(&node_id)
                .ok_or_else(|| format!("unknown completeness node: {node_id}"))?;
            if !node.source_scope.library_eligible() {
                return Err(format!("external completeness member: {node_id}"));
            }
            let connecting = graph.edges.iter().find(|edge| {
                (edge.source_id == node_id && existing.contains(&edge.target_id))
                    || (edge.target_id == node_id && existing.contains(&edge.source_id))
            });
            let Some(connecting) = connecting else {
                return Err(format!("non-neighbor completeness member: {node_id}"));
            };
            if !feature.node_ids.contains(&node_id) {
                feature.node_ids.push(node_id.clone());
                feature.memberships.push(FeatureMembership {
                    node_id: node_id.clone(),
                    confidence: 0.72,
                    reason: format!(
                        "complément IA validé par la relation persistée {}",
                        connecting.relation.as_str()
                    ),
                    source: MembershipSource::Ai,
                    optional: false,
                    external_dependency: false,
                });
            }
        }
        feature.edge_ids = graph
            .edges
            .iter()
            .filter(|edge| {
                feature.node_ids.contains(&edge.source_id)
                    && feature.node_ids.contains(&edge.target_id)
            })
            .map(|edge| edge.id.clone())
            .collect();
        refresh_feature_facets(graph, feature);
        let deterministic_score = feature.completeness;
        feature.completeness = (deterministic_score * 0.75
            + proposal.completeness.clamp(0.0, 1.0) * 0.25)
            .clamp(0.0, 1.0);
        for signal in proposal.missing_signals.into_iter().take(10) {
            let signal = signal.trim().chars().take(240).collect::<String>();
            if !signal.is_empty() {
                feature
                    .missing_signals
                    .push(format!("Analyse IA : {signal}"));
            }
        }
        feature.missing_signals.sort();
        feature.missing_signals.dedup();
        feature.source_hash = hash(
            feature
                .node_ids
                .iter()
                .cloned()
                .chain(feature.edge_ids.iter().cloned()),
        );
        feature.prompt_version = "feature-detection-v2".into();
        validate_feature(graph, feature)?;
    }
    Ok(())
}

pub fn apply_ai_feature_proposals(
    graph: &ProjectGraph,
    candidates: &[Feature],
    proposals: AiFeatureProposals,
    provider: &str,
    model: &str,
) -> Result<Vec<Feature>, String> {
    let candidates_by_id = candidates
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut result = Vec::new();
    for proposal in proposals.features {
        let candidate = candidates_by_id
            .get(proposal.candidate_id.as_str())
            .ok_or_else(|| format!("unknown candidate {}", proposal.candidate_id))?;
        let allowed = candidate
            .node_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if proposal.member_node_ids.is_empty()
            || proposal
                .member_node_ids
                .iter()
                .any(|id| !allowed.contains(id.as_str()) || graph.find_node(id).is_none())
        {
            return Err(format!(
                "invalid member in proposal {}",
                proposal.candidate_id
            ));
        }
        if proposal.entry_point_node_ids.is_empty()
            || proposal
                .entry_point_node_ids
                .iter()
                .any(|id| !candidate.entry_point_node_ids.contains(id))
        {
            return Err(format!(
                "invalid entry point in proposal {}",
                proposal.candidate_id
            ));
        }
        let mut value = (*candidate).clone();
        value.name = proposal.name.trim().chars().take(120).collect();
        value.description = proposal.description.trim().chars().take(2_000).collect();
        value.node_ids = proposal.member_node_ids;
        value.entry_point_node_ids = proposal.entry_point_node_ids;
        value
            .memberships
            .retain(|membership| value.node_ids.contains(&membership.node_id));
        for membership in &mut value.memberships {
            membership.source = MembershipSource::Ai;
            membership.reason =
                "AI semantic selection constrained to deterministic candidate".into();
        }
        value.business_rules = proposal.business_rules;
        value.inputs = proposal.inputs;
        value.outputs = proposal.outputs;
        value.side_effects = proposal.side_effects;
        value.security_constraints = proposal.security_constraints;
        value.confidence =
            (value.confidence * 0.65 + proposal.confidence.clamp(0.0, 1.0) * 0.35).clamp(0.0, 1.0);
        value.ai_provider = Some(provider.into());
        value.ai_model = Some(model.into());
        value.prompt_version = "feature-detection-v1".into();
        validate_feature(graph, &value)?;
        result.push(value);
    }
    if result.is_empty() {
        return Err("AI returned no feature proposals".into());
    }
    Ok(result)
}

pub fn feature_spec_schema() -> serde_json::Value {
    let string_array = || serde_json::json!({"type":"array","items":{"type":"string"}});
    serde_json::json!({"type":"object","additionalProperties":false,
        "required":["name","description","actors","entry_points","inputs","outputs","business_rules","operations","data","side_effects","security","error_cases","acceptance_criteria","dependencies","provenance"],
        "properties":{"name":{"type":"string"},"description":{"type":"string"},"actors":string_array(),"entry_points":string_array(),"inputs":string_array(),"outputs":string_array(),"business_rules":string_array(),"operations":string_array(),"data":string_array(),"side_effects":string_array(),"security":string_array(),"error_cases":string_array(),"acceptance_criteria":string_array(),"dependencies":string_array(),"provenance":{"type":"string"}}})
}

pub fn compact_feature_candidates(
    graph: &ProjectGraph,
    candidates: &[Feature],
) -> serde_json::Value {
    serde_json::json!({"candidates":candidates.iter().take(60).map(|candidate|serde_json::json!({
        "candidate_id":candidate.id,"name_hint":candidate.name,"entry_point_node_ids":candidate.entry_point_node_ids,
        "members":candidate.node_ids.iter().take(30).filter_map(|id|graph.find_node(id)).map(|node|serde_json::json!({"id":node.id,"name":node.name,"kind":node.kind.as_str(),"path":node.path})).collect::<Vec<_>>(),
        "edges":candidate.edge_ids.iter().take(40).filter_map(|id|graph.edges.iter().find(|edge|&edge.id==id)).map(|edge|serde_json::json!({"source":edge.source_id,"relation":edge.relation.as_str(),"target":edge.target_id})).collect::<Vec<_>>()
    })).collect::<Vec<_>>()})
}

fn hash(parts: impl IntoIterator<Item = String>) -> String {
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
fn feature_root(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::EntryPoint
            | NodeKind::Route
            | NodeKind::ApiEndpoint
            | NodeKind::Page
            | NodeKind::Command
            | NodeKind::Cli
            | NodeKind::Worker
            | NodeKind::Job
            | NodeKind::Queue
            | NodeKind::Event
            | NodeKind::WebSocket
    )
}
fn feature_name(node: &CodeNode) -> String {
    let mut name = node.name.replace(['_', '-'], " ");
    for suffix in [
        "Controller",
        "Handler",
        "Command",
        "Page",
        "Route",
        "Endpoint",
        "Job",
        "Worker",
    ] {
        name = name.trim_end_matches(suffix).trim().to_owned();
    }
    if name.is_empty() {
        node.name.clone()
    } else {
        name.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
fn names(graph: &ProjectGraph, ids: &[String], kinds: &[NodeKind]) -> Vec<String> {
    let mut values = ids
        .iter()
        .filter_map(|id| graph.find_node(id))
        .filter(|node| kinds.contains(&node.kind))
        .map(|node| node.name.clone())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

pub fn detect_feature_candidates(project_id: &str, graph: &ProjectGraph) -> Vec<Feature> {
    let roots = graph
        .nodes
        .iter()
        .filter(|node| {
            node.source_scope.library_eligible()
                && feature_root(node.kind)
                && !(node.name.starts_with("ANY /")
                    && (node.name.contains("{0}") || node.name.contains("{1}")))
                && node.path.as_deref().is_none_or(|path| {
                    let path = path.to_lowercase().replace('\\', "/");
                    !path.contains("/tests/")
                        && !path.starts_with("tests/")
                        && !path.contains("/test/")
                        && !path.contains("/fixtures/")
                        && !path.ends_with(".md")
                        && !path.ends_with(".markdown")
                        && !path.contains("/vendor/")
                        && !path.contains("/node_modules/")
                        && !path.contains("/build/")
                })
        })
        .take(250)
        .collect::<Vec<_>>();
    let mut features = Vec::new();
    for root in roots {
        let root_role = classify_role(root);
        let root_domain = detect_domain(root, root_role);
        let mut queue = BinaryHeap::from([(100_i32, 0_usize, root.id.clone())]);
        let mut best_score = HashMap::from([(root.id.clone(), 100_i32)]);
        let mut reasons = HashMap::from([(
            root.id.clone(),
            format!("point d’entrée {} détecté", root.kind.as_str()),
        )]);
        let mut edge_ids = HashSet::new();
        let mut external_services = BTreeSet::new();
        while let Some((score, depth, id)) = queue.pop() {
            if score < 35 || depth >= 10 || best_score.len() >= 120 {
                continue;
            }
            for edge in graph.outgoing_edges(&id) {
                let Some(cost) = feature_relation_cost(edge.relation) else {
                    continue;
                };
                let Some(target) = graph.find_node(&edge.target_id) else {
                    continue;
                };
                if !target.source_scope.library_eligible() {
                    if matches!(
                        target.kind,
                        NodeKind::ExternalService | NodeKind::Package | NodeKind::Crate
                    ) {
                        external_services.insert(target.name.clone());
                    }
                    continue;
                }
                let target_domain = detect_domain(target, classify_role(target));
                let same_file = root.path.is_some() && root.path == target.path;
                let same_owner = root.owner.is_some() && root.owner == target.owner;
                let affinity_bonus = i32::from(same_file) * 8
                    + i32::from(same_owner) * 5
                    + i32::from(target_domain == root_domain) * 6;
                let next_score = (score - cost + affinity_bonus).min(99);
                if next_score < 35 {
                    continue;
                }
                let should_update = best_score
                    .get(&target.id)
                    .is_none_or(|previous| next_score > *previous);
                if should_update {
                    best_score.insert(target.id.clone(), next_score);
                    reasons.insert(
                        target.id.clone(),
                        format!(
                            "{} depuis {} via {}",
                            edge.relation.as_str().to_lowercase(),
                            graph
                                .find_node(&id)
                                .map(|node| node.name.as_str())
                                .unwrap_or(&id),
                            edge.id
                        ),
                    );
                    edge_ids.insert(edge.id.clone());
                    queue.push((next_score, depth + 1, target.id.clone()));
                }
            }
        }
        if best_score.len() == 1
            && let Some(path) = root.path.as_deref()
        {
            for node in graph
                .find_nodes_by_path(path)
                .into_iter()
                .filter(|node| node.source_scope.library_eligible())
                .filter(|node| {
                    matches!(
                        node.kind,
                        NodeKind::Function
                            | NodeKind::Method
                            | NodeKind::Component
                            | NodeKind::Handler
                            | NodeKind::Service
                            | NodeKind::Controller
                    )
                })
                .take(20)
            {
                best_score.insert(node.id.clone(), 55);
                reasons.insert(
                    node.id.clone(),
                    format!("symbole first-party du même fichier que {}", root.name),
                );
            }
        }
        // Tests are often callers rather than callees. Pull in only tests that
        // directly exercise an already selected first-party member.
        let selected = best_score.keys().cloned().collect::<HashSet<_>>();
        for test in graph.nodes.iter().filter(|node| {
            node.source_scope.library_eligible()
                && node.path.as_deref().is_some_and(|path| {
                    let path = path.to_lowercase();
                    path.contains("test") || path.contains("spec")
                })
        }) {
            if let Some(edge) = graph.outgoing_edges(&test.id).into_iter().find(|edge| {
                matches!(edge.relation, RelationKind::Calls | RelationKind::Uses)
                    && selected.contains(&edge.target_id)
            }) {
                best_score.insert(test.id.clone(), 62);
                reasons.insert(
                    test.id.clone(),
                    format!("test first-party couvrant {}", edge.target_id),
                );
                edge_ids.insert(edge.id.clone());
            }
        }
        let mut nodes = best_score.keys().cloned().collect::<Vec<_>>();
        nodes.sort();
        let mut edges = edge_ids.into_iter().collect::<Vec<_>>();
        edges.sort();
        let source_hash = hash(nodes.iter().cloned().chain(edges.iter().cloned()));
        let id = format!(
            "feature:{}",
            &hash([project_id.into(), root.id.clone()])[..24]
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let memberships = nodes
            .iter()
            .map(|node_id| FeatureMembership {
                node_id: node_id.clone(),
                confidence: best_score.get(node_id).copied().unwrap_or(50) as f32 / 100.0,
                reason: reasons
                    .get(node_id)
                    .cloned()
                    .unwrap_or_else(|| "relation déterministe persistée".into()),
                source: MembershipSource::Deterministic,
                optional: false,
                external_dependency: false,
            })
            .collect();
        let mut feature = Feature {
            id,
            project_id: project_id.into(),
            name: feature_name(root),
            description: format!(
                "Flux fonctionnel candidat ancré sur {} {}.",
                root.kind.as_str(),
                root.name
            ),
            confidence: (0.55 + (nodes.len().min(30) as f32 / 100.0)).min(0.85),
            completeness: 0.0,
            missing_signals: vec![],
            status: FeatureStatus::Detected,
            entry_point_node_ids: vec![root.id.clone()],
            routes: names(graph, &nodes, &[NodeKind::Route, NodeKind::ApiEndpoint]),
            pages: names(graph, &nodes, &[NodeKind::Page, NodeKind::Component]),
            services: names(
                graph,
                &nodes,
                &[NodeKind::Service, NodeKind::Handler, NodeKind::Controller],
            ),
            repositories: names(graph, &nodes, &[NodeKind::Repository]),
            models: names(graph, &nodes, &[NodeKind::Model]),
            entities: names(graph, &nodes, &[NodeKind::Entity]),
            templates: names(graph, &nodes, &[NodeKind::Template]),
            tests: nodes
                .iter()
                .filter_map(|id| graph.find_node(id))
                .filter(|node| {
                    node.path.as_deref().is_some_and(|path| {
                        let path = path.to_lowercase();
                        path.contains("test") || path.contains("spec")
                    })
                })
                .map(|node| node.name.clone())
                .collect(),
            external_services: external_services.into_iter().collect(),
            node_ids: nodes,
            edge_ids: edges,
            memberships,
            business_rules: vec![],
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            security_constraints: vec![],
            source_hash,
            ai_provider: None,
            ai_model: None,
            prompt_version: "feature-deterministic-v2".into(),
            created_at: now,
            updated_at: now,
        };
        refresh_feature_completeness(graph, &mut feature);
        features.push(feature);
    }
    group_route_features(project_id, graph, features)
}

fn feature_relation_cost(relation: RelationKind) -> Option<i32> {
    match relation {
        RelationKind::RoutesTo | RelationKind::HandledBy => Some(5),
        RelationKind::Calls => Some(8),
        RelationKind::Reads | RelationKind::Writes | RelationKind::Renders => Some(10),
        RelationKind::Uses
        | RelationKind::Creates
        | RelationKind::Emits
        | RelationKind::Listens => Some(14),
        RelationKind::DependsOn | RelationKind::Returns => Some(20),
        RelationKind::Imports => Some(28),
        _ => None,
    }
}

pub fn refresh_feature_completeness(graph: &ProjectGraph, feature: &mut Feature) {
    let nodes = feature
        .node_ids
        .iter()
        .filter_map(|id| graph.find_node(id))
        .collect::<Vec<_>>();
    let roles = nodes
        .iter()
        .map(|node| classify_role(node))
        .collect::<HashSet<_>>();
    let unresolved = graph
        .unresolved_calls
        .iter()
        .filter(|call| feature.node_ids.contains(&call.caller_id))
        .count();
    let mut score = 0.0_f32;
    let mut missing = Vec::new();
    if !feature.entry_point_node_ids.is_empty() {
        score += 0.15;
    } else {
        missing.push("aucun point d’entrée détecté".into());
    }
    if nodes.len() > 1 {
        score += 0.15;
    } else {
        missing.push("un seul nœud first-party relié".into());
    }
    score += (roles.len().min(5) as f32 / 5.0) * 0.20;
    if roles.len() < 2 {
        missing.push("une seule couche architecturale représentée".into());
    }
    if !feature.routes.is_empty() {
        score += 0.10;
    } else {
        missing.push("aucune route associée".into());
    }
    if !feature.services.is_empty()
        || roles.contains(&ArchitectureRole::Service)
        || roles.contains(&ArchitectureRole::Provider)
    {
        score += 0.10;
    } else {
        missing.push("aucun service applicatif détecté".into());
    }
    if !feature.repositories.is_empty()
        || roles.contains(&ArchitectureRole::Repository)
        || roles.contains(&ArchitectureRole::Database)
    {
        score += 0.10;
    } else {
        missing.push("aucune persistance détectée".into());
    }
    if !feature.tests.is_empty() || roles.contains(&ArchitectureRole::Test) {
        score += 0.10;
    } else {
        missing.push("aucun test relié".into());
    }
    if !feature.templates.is_empty()
        || roles.contains(&ArchitectureRole::Template)
        || roles.contains(&ArchitectureRole::Page)
    {
        score += 0.05;
    }
    if unresolved > 0 {
        score -= (unresolved.min(4) as f32) * 0.03;
        missing.push(format!(
            "{unresolved} référence(s) interne(s) non résolue(s)"
        ));
    }
    feature.completeness = score.clamp(0.0, 1.0);
    feature.missing_signals = missing;
}

pub fn refresh_feature_facets(graph: &ProjectGraph, feature: &mut Feature) {
    feature.routes = names(
        graph,
        &feature.node_ids,
        &[NodeKind::Route, NodeKind::ApiEndpoint],
    );
    feature.pages = names(
        graph,
        &feature.node_ids,
        &[NodeKind::Page, NodeKind::Component],
    );
    feature.services = names(
        graph,
        &feature.node_ids,
        &[NodeKind::Service, NodeKind::Handler, NodeKind::Controller],
    );
    feature.repositories = names(graph, &feature.node_ids, &[NodeKind::Repository]);
    feature.models = names(graph, &feature.node_ids, &[NodeKind::Model]);
    feature.entities = names(graph, &feature.node_ids, &[NodeKind::Entity]);
    feature.templates = names(graph, &feature.node_ids, &[NodeKind::Template]);
    feature.tests = feature
        .node_ids
        .iter()
        .filter_map(|id| graph.find_node(id))
        .filter(|node| {
            node.path.as_deref().is_some_and(|path| {
                let path = path.to_lowercase();
                path.contains("test") || path.contains("spec")
            })
        })
        .map(|node| node.name.clone())
        .collect();
    refresh_feature_completeness(graph, feature);
}

fn route_family(feature: &Feature) -> Option<(&'static str, &'static str)> {
    let route = feature.routes.first()?.to_lowercase();
    let path = route.split_whitespace().last().unwrap_or(&route);
    if path.contains("/api/mcp") {
        return Some(("mcp", "Serveur MCP"));
    }
    if path.contains("/api/fs/") {
        return Some(("project-selection", "Sélection de projet"));
    }
    if path.contains("/api/library") || path.contains("library-candidates") {
        return Some(("library", "Bibliothèque de code"));
    }
    if path.contains("/api/findings") || path.contains("/quality") || path.contains("/security") {
        return Some(("quality-security", "Qualité et sécurité"));
    }
    if path.contains("/features/") && path.contains("/port/") {
        return Some(("feature-porting", "Portage de Feature"));
    }
    if path.contains("/features") {
        return Some(("feature-intelligence", "Intelligence des Features"));
    }
    if path.contains("/docs") {
        return Some(("documentation", "Documentation projet"));
    }
    if path.contains("/nodes")
        || path.contains("/search")
        || path.contains("/impact")
        || path.contains("/context")
        || path.contains("/ai/query")
    {
        return Some(("code-exploration", "Exploration et impact du code"));
    }
    if path.contains("/api/projects") {
        return Some(("project-registry", "Registre et analyse des projets"));
    }
    if path.contains("/api/events") {
        return Some(("live-events", "Suivi d’analyse en temps réel"));
    }
    None
}

fn group_route_features(
    project_id: &str,
    graph: &ProjectGraph,
    features: Vec<Feature>,
) -> Vec<Feature> {
    let mut grouped = BTreeMap::<String, Feature>::new();
    let mut ungrouped = Vec::new();
    for feature in features {
        let Some((family, label)) = route_family(&feature) else {
            ungrouped.push(feature);
            continue;
        };
        let entry = grouped.entry(family.into()).or_insert_with(|| {
            let mut value = feature.clone();
            value.id = format!(
                "feature:{}",
                &hash([project_id.into(), family.into()])[..24]
            );
            value.name = label.into();
            value.description =
                format!("Capacité fonctionnelle regroupant les flux liés à {label}.");
            value
        });
        if entry.source_hash == feature.source_hash {
            continue;
        }
        for (target, values) in [
            (
                &mut entry.entry_point_node_ids,
                &feature.entry_point_node_ids,
            ),
            (&mut entry.node_ids, &feature.node_ids),
            (&mut entry.edge_ids, &feature.edge_ids),
            (&mut entry.routes, &feature.routes),
            (&mut entry.pages, &feature.pages),
            (&mut entry.services, &feature.services),
            (&mut entry.repositories, &feature.repositories),
            (&mut entry.models, &feature.models),
            (&mut entry.entities, &feature.entities),
            (&mut entry.templates, &feature.templates),
            (&mut entry.tests, &feature.tests),
            (&mut entry.external_services, &feature.external_services),
        ] {
            let existing = target.iter().cloned().collect::<HashSet<_>>();
            target.extend(
                values
                    .iter()
                    .filter(|value| !existing.contains(*value))
                    .cloned(),
            );
            target.sort();
        }
        let existing_members = entry
            .memberships
            .iter()
            .map(|membership| membership.node_id.clone())
            .collect::<HashSet<_>>();
        entry.memberships.extend(
            feature
                .memberships
                .into_iter()
                .filter(|membership| !existing_members.contains(&membership.node_id)),
        );
        entry.confidence = entry.confidence.max(feature.confidence);
        entry.source_hash = hash(
            entry
                .node_ids
                .iter()
                .cloned()
                .chain(entry.edge_ids.iter().cloned()),
        );
        entry.updated_at = entry.updated_at.max(feature.updated_at);
    }
    for feature in grouped.values_mut() {
        refresh_feature_completeness(graph, feature);
    }
    ungrouped.extend(grouped.into_values());
    ungrouped.sort_by(|left, right| left.name.cmp(&right.name));
    ungrouped
}

pub fn validate_feature(graph: &ProjectGraph, feature: &Feature) -> Result<(), String> {
    if feature.entry_point_node_ids.is_empty() {
        return Err("feature requires at least one entry point".into());
    }
    for id in &feature.node_ids {
        let node = graph
            .find_node(id)
            .ok_or_else(|| format!("unknown node: {id}"))?;
        if !node.source_scope.library_eligible() {
            return Err(format!("external or generated core member: {id}"));
        }
    }
    for id in &feature.edge_ids {
        if !graph.edges.iter().any(|edge| &edge.id == id) {
            return Err(format!("unknown edge: {id}"));
        }
    }
    if feature
        .entry_point_node_ids
        .iter()
        .any(|id| !feature.node_ids.contains(id))
    {
        return Err("entry point is not a feature member".into());
    }
    Ok(())
}

pub fn build_feature_spec(graph: &ProjectGraph, feature: &Feature) -> FeatureSpec {
    let operations = feature
        .node_ids
        .iter()
        .filter_map(|id| graph.find_node(id))
        .filter(|node| {
            matches!(
                node.kind,
                NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::Service
                    | NodeKind::Handler
                    | NodeKind::Controller
            )
        })
        .map(|node| node.name.clone())
        .collect();
    let data = feature
        .models
        .iter()
        .chain(&feature.entities)
        .chain(&feature.repositories)
        .cloned()
        .collect();
    FeatureSpec {
        name: feature.name.clone(),
        description: feature.description.clone(),
        actors: vec![],
        entry_points: feature
            .entry_point_node_ids
            .iter()
            .filter_map(|id| graph.find_node(id))
            .map(|node| node.name.clone())
            .collect(),
        inputs: feature.inputs.clone(),
        outputs: feature.outputs.clone(),
        business_rules: feature.business_rules.clone(),
        operations,
        data,
        side_effects: feature.side_effects.clone(),
        security: feature.security_constraints.clone(),
        error_cases: vec![],
        acceptance_criteria: feature
            .tests
            .iter()
            .map(|test| format!("Behavior covered by {test}"))
            .collect(),
        dependencies: feature
            .services
            .iter()
            .chain(&feature.repositories)
            .chain(&feature.external_services)
            .cloned()
            .collect(),
        provenance: "deterministic".into(),
    }
}

pub fn feature_code(graph: &ProjectGraph, feature: &Feature) -> Vec<FeatureCodeSnippet> {
    let mut files = HashMap::<String, String>::new();
    let mut seen_nodes = HashSet::new();
    let mut seen_ranges = HashSet::new();
    let mut node_ids = feature.entry_point_node_ids.clone();
    node_ids.extend(feature.node_ids.iter().cloned());
    let mut snippets = node_ids
        .iter()
        .filter(|id| seen_nodes.insert((*id).clone()))
        .filter_map(|id| graph.find_node(id))
        .filter_map(|node| Some((node, node.path.as_deref()?)))
        .filter(|(node, _)| node.source_scope.library_eligible())
        .take(200)
        .filter_map(|(node, path)| {
            let source = files.entry(path.into()).or_insert_with(|| {
                fs::read_to_string(Path::new(&graph.root).join(path)).unwrap_or_default()
            });
            let start = node.start_line.unwrap_or(1);
            let end = node.end_line.unwrap_or(start).min(start + 240);
            if !seen_ranges.insert((path.to_owned(), start, end)) {
                return None;
            }
            let snippet = source
                .lines()
                .skip(start.saturating_sub(1))
                .take(end.saturating_sub(start) + 1)
                .collect::<Vec<_>>()
                .join("\n");
            if snippet.is_empty() {
                None
            } else {
                Some(FeatureCodeSnippet {
                    project_id: feature.project_id.clone(),
                    node_id: node.id.clone(),
                    name: node.name.clone(),
                    language: node.language.map(|language| language.as_str().into()),
                    architecture_role: feature_source_group(classify_role(node)).into(),
                    path: path.into(),
                    start_line: start,
                    end_line: end,
                    source: snippet,
                    source_hash: hash([source.clone()]),
                    membership_reason: feature
                        .memberships
                        .iter()
                        .find(|membership| membership.node_id == node.id)
                        .map(|membership| membership.reason.clone())
                        .unwrap_or_else(|| "appartenance déterministe à la Feature".into()),
                })
            }
        })
        .collect::<Vec<_>>();

    // Some parsers only resolve a Feature to a file/module node. In that case,
    // expose a bounded source window instead of presenting an empty Code tab.
    if snippets.is_empty() {
        for node in feature
            .node_ids
            .iter()
            .filter_map(|id| graph.find_node(id))
            .filter(|node| node.source_scope.library_eligible())
        {
            let Some(path) = node.path.as_deref() else {
                continue;
            };
            let source = files.entry(path.into()).or_insert_with(|| {
                fs::read_to_string(Path::new(&graph.root).join(path)).unwrap_or_default()
            });
            if source.is_empty() {
                continue;
            }
            let end = source.lines().count().min(400);
            snippets.push(FeatureCodeSnippet {
                project_id: feature.project_id.clone(),
                node_id: node.id.clone(),
                name: node.name.clone(),
                language: node.language.map(|language| language.as_str().into()),
                architecture_role: feature_source_group(classify_role(node)).into(),
                path: path.into(),
                start_line: 1,
                end_line: end,
                source: source.lines().take(end).collect::<Vec<_>>().join("\n"),
                source_hash: hash([source.clone()]),
                membership_reason: feature
                    .memberships
                    .iter()
                    .find(|membership| membership.node_id == node.id)
                    .map(|membership| membership.reason.clone())
                    .unwrap_or_else(|| "fichier first-party de la Feature".into()),
            });
            if snippets.len() >= 20 {
                break;
            }
        }
    }
    snippets
}

fn feature_source_group(role: ArchitectureRole) -> &'static str {
    match role {
        ArchitectureRole::Application | ArchitectureRole::EntryPoint | ArchitectureRole::Cli => {
            "Entry Points"
        }
        ArchitectureRole::Frontend | ArchitectureRole::Page | ArchitectureRole::Component => {
            "Frontend"
        }
        ArchitectureRole::Api
        | ArchitectureRole::Route
        | ArchitectureRole::Controller
        | ArchitectureRole::Handler => "API",
        ArchitectureRole::Service | ArchitectureRole::Provider => "Application",
        ArchitectureRole::Model | ArchitectureRole::Entity => "Domain",
        ArchitectureRole::Repository | ArchitectureRole::Database => "Persistence",
        ArchitectureRole::Event
        | ArchitectureRole::Listener
        | ArchitectureRole::MessageHandler
        | ArchitectureRole::Queue => "Events",
        ArchitectureRole::Template => "Templates",
        ArchitectureRole::Test => "Tests",
        ArchitectureRole::Shared => "Shared",
        _ => "Other",
    }
}

pub fn feature_source_files(graph: &ProjectGraph, feature: &Feature) -> Vec<FeatureSourceFile> {
    let mut by_path = BTreeMap::<String, Vec<FeatureCodeSnippet>>::new();
    for snippet in feature_code(graph, feature) {
        by_path
            .entry(snippet.path.clone())
            .or_default()
            .push(snippet);
    }
    let mut files = by_path
        .into_iter()
        .filter_map(|(path, mut fragments)| {
            fragments.sort_by_key(|fragment| (fragment.start_line, fragment.end_line));
            let first = fragments.first()?;
            Some(FeatureSourceFile {
                path,
                language: first.language.clone(),
                architecture_role: first.architecture_role.clone(),
                source_hash: first.source_hash.clone(),
                fragments,
            })
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| {
        left.architecture_role
            .cmp(&right.architecture_role)
            .then(left.path.cmp(&right.path))
    });
    files
}

pub fn feature_source_search(
    graph: &ProjectGraph,
    feature: &Feature,
    query: &str,
) -> Vec<FeatureSourceSearchHit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return vec![];
    }
    let mut hits = Vec::new();
    for fragment in feature_code(graph, feature) {
        for (offset, line) in fragment.source.lines().enumerate() {
            if line.to_lowercase().contains(&query) {
                hits.push(FeatureSourceSearchHit {
                    path: fragment.path.clone(),
                    node_id: fragment.node_id.clone(),
                    symbol: fragment.name.clone(),
                    line: fragment.start_line + offset,
                    preview: line.trim().chars().take(240).collect(),
                });
                if hits.len() >= 200 {
                    return hits;
                }
            }
        }
    }
    hits
}

pub fn feature_entire_file(
    graph: &ProjectGraph,
    feature: &Feature,
    requested_path: &str,
) -> Result<String, String> {
    let allowed = feature.node_ids.iter().any(|id| {
        graph
            .find_node(id)
            .and_then(|node| node.path.as_deref())
            .is_some_and(|path| path == requested_path)
    });
    if !allowed {
        return Err("ce fichier n’appartient pas à la Feature".into());
    }
    let root = Path::new(&graph.root)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let candidate = root
        .join(requested_path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !candidate.starts_with(&root) {
        return Err("chemin source hors du projet".into());
    }
    let metadata = candidate.metadata().map_err(|error| error.to_string())?;
    if metadata.len() > 2_000_000 {
        return Err("fichier trop volumineux pour l’inspection".into());
    }
    fs::read_to_string(candidate).map_err(|error| error.to_string())
}

pub fn feature_graph(graph: &ProjectGraph, feature: &Feature) -> ProjectGraph {
    let ids = feature.node_ids.iter().cloned().collect::<HashSet<_>>();
    ProjectGraph::new(
        graph.root.clone(),
        graph
            .nodes
            .iter()
            .filter(|node| ids.contains(&node.id))
            .cloned()
            .collect(),
        graph
            .edges
            .iter()
            .filter(|edge| ids.contains(&edge.source_id) && ids.contains(&edge.target_id))
            .cloned()
            .collect(),
        vec![],
        vec![],
        vec![],
    )
}
pub fn similarity(left: &Feature, right: &Feature) -> f32 {
    fn tokens(feature: &Feature) -> BTreeSet<String> {
        std::iter::once(&feature.name)
            .chain(std::iter::once(&feature.description))
            .chain(&feature.business_rules)
            .chain(&feature.inputs)
            .chain(&feature.outputs)
            .chain(&feature.side_effects)
            .chain(&feature.security_constraints)
            .chain(&feature.routes)
            .flat_map(|value| value.split(|character: char| !character.is_alphanumeric()))
            .map(str::to_lowercase)
            .filter(|value| value.len() > 2)
            .collect()
    }
    fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f32 {
        let union = a.union(b).count();
        if union == 0 {
            0.0
        } else {
            a.intersection(b).count() as f32 / union as f32
        }
    }
    fn ratio(a: usize, b: usize) -> f32 {
        if a == 0 && b == 0 {
            1.0
        } else {
            a.min(b) as f32 / a.max(b).max(1) as f32
        }
    }
    let semantic = jaccard(&tokens(left), &tokens(right));
    let shape = (ratio(
        left.entry_point_node_ids.len(),
        right.entry_point_node_ids.len(),
    ) + ratio(left.node_ids.len(), right.node_ids.len())
        + ratio(left.edge_ids.len(), right.edge_ids.len())
        + ratio(left.tests.len(), right.tests.len()))
        / 4.0;
    let names = jaccard(
        &left
            .name
            .split_whitespace()
            .map(str::to_lowercase)
            .collect(),
        &right
            .name
            .split_whitespace()
            .map(str::to_lowercase)
            .collect(),
    );
    (semantic * 0.7 + shape * 0.2 + names * 0.1).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{node::CodeNode, project::SourceScope};
    #[test]
    fn feature_validation_rejects_external_members() {
        let root = CodeNode {
            id: "route".into(),
            kind: NodeKind::Route,
            name: "login".into(),
            path: Some("src/login.rs".into()),
            language: None,
            start_line: None,
            end_line: None,
            owner: None,
            source_scope: SourceScope::Project,
        };
        let external = CodeNode {
            id: "dep".into(),
            kind: NodeKind::Service,
            name: "dep".into(),
            path: Some("vendor/dep.rs".into()),
            language: None,
            start_line: None,
            end_line: None,
            owner: None,
            source_scope: SourceScope::ExternalDependency,
        };
        let graph = ProjectGraph::new(
            ".".into(),
            vec![root, external],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let mut feature = detect_feature_candidates("p", &graph).remove(0);
        feature.node_ids.push("dep".into());
        assert!(validate_feature(&graph, &feature).is_err());
    }

    #[test]
    fn ai_proposals_are_strictly_constrained_to_graph_candidates() {
        let node = |id: &str, kind| CodeNode {
            id: id.into(),
            kind,
            name: id.into(),
            path: Some(format!("src/{id}.rs")),
            language: None,
            start_line: Some(1),
            end_line: Some(5),
            owner: None,
            source_scope: SourceScope::Project,
        };
        let graph = ProjectGraph::new(
            ".".into(),
            vec![
                node("login", NodeKind::Route),
                node("auth", NodeKind::Service),
            ],
            vec![crate::model::edge::CodeEdge::new(
                "login".into(),
                "auth".into(),
                crate::model::edge::RelationKind::Calls,
            )],
            vec![],
            vec![],
            vec![],
        );
        let candidates = detect_feature_candidates("p", &graph);
        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        let proposal = AiFeatureProposals {
            features: vec![AiFeatureProposal {
                candidate_id: candidate.id.clone(),
                name: "Authentication".into(),
                description: "Authenticate a user".into(),
                member_node_ids: candidate.node_ids.clone(),
                entry_point_node_ids: candidate.entry_point_node_ids.clone(),
                business_rules: vec!["credentials required".into()],
                inputs: vec!["credentials".into()],
                outputs: vec!["session".into()],
                side_effects: vec![],
                security_constraints: vec!["rate limit".into()],
                confidence: 0.9,
            }],
        };
        let generated =
            apply_ai_feature_proposals(&graph, &candidates, proposal, "Mock", "structured-v1")
                .unwrap();
        assert_eq!(generated[0].name, "Authentication");
        assert_eq!(generated[0].ai_provider.as_deref(), Some("Mock"));
        let invalid = AiFeatureProposals {
            features: vec![AiFeatureProposal {
                candidate_id: candidate.id.clone(),
                name: "Invalid".into(),
                description: String::new(),
                member_node_ids: vec!["invented".into()],
                entry_point_node_ids: candidate.entry_point_node_ids.clone(),
                business_rules: vec![],
                inputs: vec![],
                outputs: vec![],
                side_effects: vec![],
                security_constraints: vec![],
                confidence: 1.0,
            }],
        };
        assert!(apply_ai_feature_proposals(&graph, &candidates, invalid, "Mock", "model").is_err());
    }

    #[test]
    fn completeness_pass_accepts_only_persisted_first_party_neighbors() {
        let node = |id: &str, kind| CodeNode {
            id: id.into(),
            kind,
            name: id.into(),
            path: Some(format!("src/{id}.rs")),
            language: None,
            start_line: Some(1),
            end_line: Some(5),
            owner: None,
            source_scope: SourceScope::Project,
        };
        let graph = ProjectGraph::new(
            ".".into(),
            vec![
                node("route", NodeKind::Route),
                node("service", NodeKind::Service),
                node("repository", NodeKind::Repository),
                node("unrelated", NodeKind::Model),
            ],
            vec![
                crate::model::edge::CodeEdge::new(
                    "route".into(),
                    "service".into(),
                    RelationKind::Calls,
                ),
                crate::model::edge::CodeEdge::new(
                    "service".into(),
                    "repository".into(),
                    RelationKind::Calls,
                ),
            ],
            vec![],
            vec![],
            vec![],
        );
        let mut features = detect_feature_candidates("p", &graph);
        let feature_id = features[0].id.clone();
        features[0].node_ids.retain(|id| id != "repository");
        features[0]
            .memberships
            .retain(|membership| membership.node_id != "repository");
        apply_ai_feature_completeness(
            &graph,
            &mut features,
            AiFeatureCompletenessProposals {
                features: vec![AiFeatureCompletenessProposal {
                    feature_id: feature_id.clone(),
                    additional_node_ids: vec!["repository".into()],
                    missing_signals: vec!["aucun test d’intégration observé".into()],
                    completeness: 0.8,
                }],
            },
        )
        .unwrap();
        assert!(features[0].node_ids.contains(&"repository".into()));
        assert!(features[0].memberships.iter().any(|membership| {
            membership.node_id == "repository" && membership.source == MembershipSource::Ai
        }));

        let rejected = apply_ai_feature_completeness(
            &graph,
            &mut features,
            AiFeatureCompletenessProposals {
                features: vec![AiFeatureCompletenessProposal {
                    feature_id,
                    additional_node_ids: vec!["unrelated".into()],
                    missing_signals: vec![],
                    completeness: 1.0,
                }],
            },
        );
        assert!(rejected.is_err());
    }
}
