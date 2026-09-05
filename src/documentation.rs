use crate::graph::project_graph::ProjectGraph;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub const PROJECT_DOC_PROMPT_VERSION: &str = "project-docs-fr-v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocumentation {
    pub project_id: String,
    pub project_name: String,
    pub overview: String,
    pub purpose: String,
    pub architecture: Vec<String>,
    pub applications: Vec<String>,
    pub entry_points: Vec<String>,
    pub features: Vec<String>,
    pub domains: Vec<String>,
    pub main_flows: Vec<String>,
    pub modules: Vec<String>,
    pub apis: Vec<String>,
    pub data_model: Vec<String>,
    pub external_services: Vec<String>,
    pub workers: Vec<String>,
    pub events: Vec<String>,
    pub configuration: Vec<String>,
    pub security_notes: Vec<String>,
    pub development: Vec<String>,
    pub deployment: Vec<String>,
    pub known_limitations: Vec<String>,
    pub content_hash: String,
    pub prompt_version: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub cache_hit: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiProjectDocumentation {
    pub overview: String,
    pub purpose: String,
    pub architecture: Vec<String>,
    pub applications: Vec<String>,
    pub entry_points: Vec<String>,
    pub features: Vec<String>,
    pub domains: Vec<String>,
    pub main_flows: Vec<String>,
    pub modules: Vec<String>,
    pub apis: Vec<String>,
    pub data_model: Vec<String>,
    pub external_services: Vec<String>,
    pub workers: Vec<String>,
    pub events: Vec<String>,
    pub configuration: Vec<String>,
    pub security_notes: Vec<String>,
    pub development: Vec<String>,
    pub deployment: Vec<String>,
    pub known_limitations: Vec<String>,
}

pub fn project_documentation_schema() -> serde_json::Value {
    let fields = [
        "architecture",
        "applications",
        "entry_points",
        "features",
        "domains",
        "main_flows",
        "modules",
        "apis",
        "data_model",
        "external_services",
        "workers",
        "events",
        "configuration",
        "security_notes",
        "development",
        "deployment",
        "known_limitations",
    ];
    let mut properties = serde_json::Map::new();
    properties.insert("overview".into(), serde_json::json!({"type":"string"}));
    properties.insert("purpose".into(), serde_json::json!({"type":"string"}));
    for field in fields {
        properties.insert(
            field.into(),
            serde_json::json!({"type":"array","items":{"type":"string"}}),
        );
    }
    let mut required = vec!["overview", "purpose"];
    required.extend(fields);
    serde_json::json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}

pub fn apply_ai_documentation(
    mut base: ProjectDocumentation,
    value: AiProjectDocumentation,
    provider: &str,
    model: &str,
) -> ProjectDocumentation {
    base.overview = value.overview;
    base.purpose = value.purpose;
    base.architecture = value.architecture;
    base.applications = value.applications;
    base.entry_points = value.entry_points;
    base.features = value.features;
    base.domains = value.domains;
    base.main_flows = value.main_flows;
    base.modules = value.modules;
    base.apis = value.apis;
    base.data_model = value.data_model;
    base.external_services = value.external_services;
    base.workers = value.workers;
    base.events = value.events;
    base.configuration = value.configuration;
    base.security_notes = value.security_notes;
    base.development = value.development;
    base.deployment = value.deployment;
    base.known_limitations = value.known_limitations;
    base.provider = Some(provider.into());
    base.model = Some(model.into());
    base.cache_hit = false;
    base.updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    base
}

pub fn project_content_hash(graph: &ProjectGraph) -> String {
    let mut hasher = Sha256::new();
    for node in &graph.nodes {
        hasher.update(node.id.as_bytes());
        hasher.update(node.kind.as_str().as_bytes());
    }
    for edge in &graph.edges {
        hasher.update(edge.id.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub fn project_knowledge_hash(
    graph: &ProjectGraph,
    features: &[crate::features::Feature],
    specs: &[crate::features::FeatureSpec],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(project_content_hash(graph));
    for feature in features {
        hasher.update(feature.id.as_bytes());
        hasher.update(feature.source_hash.as_bytes());
        hasher.update(feature.updated_at.to_le_bytes());
    }
    for spec in specs {
        hasher.update(serde_json::to_vec(spec).unwrap_or_default());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn build_project_documentation(project_id: &str, graph: &ProjectGraph) -> ProjectDocumentation {
    let mut by_kind = BTreeMap::<String, Vec<String>>::new();
    for node in graph
        .nodes
        .iter()
        .filter(|node| node.source_scope.library_eligible())
    {
        by_kind
            .entry(node.kind.as_str().into())
            .or_default()
            .push(match node.path.as_deref() {
                Some(path) => format!("{} — {path}", node.name),
                None => node.name.clone(),
            });
    }
    for values in by_kind.values_mut() {
        values.sort();
        values.dedup();
        values.truncate(100)
    }
    let take = |kinds: &[&str]| {
        kinds
            .iter()
            .flat_map(|kind| by_kind.get(*kind).cloned().unwrap_or_default())
            .take(150)
            .collect::<Vec<_>>()
    };
    let project_name = Path::new(&graph.root)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_owned();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let languages = graph
        .nodes
        .iter()
        .filter_map(|node| node.language)
        .map(|language| language.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let first_party_nodes = graph
        .nodes
        .iter()
        .filter(|node| node.source_scope.library_eligible())
        .count();
    let first_party_files = graph
        .nodes
        .iter()
        .filter(|node| node.source_scope.library_eligible() && node.kind.as_str() == "File")
        .count();
    let mut architecture = vec![
        format!(
            "{first_party_nodes} nœuds de code propriétaire répartis dans {first_party_files} fichiers."
        ),
        format!(
            "{} relations résolues et {} appels encore non résolus.",
            graph.edges.len(),
            graph.unresolved_calls.len()
        ),
        format!(
            "Langages détectés : {}.",
            languages.iter().copied().collect::<Vec<_>>().join(", ")
        ),
    ];
    architecture.extend(take(&[
        "ArchitectureZone",
        "ArchitectureGroup",
        "Application",
        "Workspace",
        "Package",
        "Crate",
    ]));
    let main_flows = graph
        .edges
        .iter()
        .filter(|edge| {
            !matches!(
                edge.relation,
                crate::model::edge::RelationKind::Contains
                    | crate::model::edge::RelationKind::Declares
            )
        })
        .filter_map(|edge| {
            let source = graph.find_node(&edge.source_id)?;
            let target = graph.find_node(&edge.target_id)?;
            Some(format!(
                "{} —{}→ {}",
                source.name,
                edge.relation.as_str(),
                target.name
            ))
        })
        .take(150)
        .collect();
    ProjectDocumentation{
        project_id:project_id.into(),project_name:project_name.clone(),
        overview:format!("{project_name} contient {first_party_nodes} nœuds de code propriétaire et {} relations résolues dans {} langage(s).",graph.edges.len(),languages.len()),
        purpose:"Documentation construite à partir du graphe persistant, des points d’entrée, des flux, des Features et du code source. Les intentions métier non explicites dans le code sont signalées comme telles.".into(),
        architecture,
        applications:take(&["Project","Workspace","Package","Crate"]),entry_points:take(&["EntryPoint","Page","Route","ApiEndpoint","Cli","Command"]),
        features:Vec::new(),domains:take(&["Module","Service","Controller","Handler"]),
        main_flows,
        modules:take(&["Module","File","Service"]),apis:take(&["Route","ApiEndpoint","Controller","Handler"]),data_model:take(&["Model","Entity","Database","DatabaseTable","Repository"]),external_services:take(&["ExternalService"]),workers:take(&["Worker","Job","Queue"]),events:take(&["Event","Listener","EventListener","EventSubscriber","MessageHandler"]),configuration:take(&["Config","Environment"]),
        security_notes:vec!["Examiner les constats de sécurité déterministes et leurs preuves avant tout déploiement.".into()],development:languages.into_iter().map(|language|format!("Le projet contient du code {language}.")).collect(),deployment:take(&["Infrastructure"]),known_limitations:vec![format!("{} référence(s) d’appel restent non résolues ; les analyses d’impact correspondantes peuvent être incomplètes.",graph.unresolved_calls.len())],
        content_hash:project_content_hash(graph),prompt_version:PROJECT_DOC_PROMPT_VERSION.into(),provider:None,model:None,cache_hit:false,created_at:now,updated_at:now,
    }
}

impl ProjectDocumentation {
    pub fn markdown(&self) -> String {
        fn section(title: &str, items: &[String]) -> String {
            if items.is_empty() {
                format!("## {title}\n\n_Not detected._\n\n")
            } else {
                format!(
                    "## {title}\n\n{}\n\n",
                    items
                        .iter()
                        .map(|item| format!("- {item}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        }
        let mut value = format!(
            "# {}\n\n{}\n\n## Purpose\n\n{}\n\n",
            self.project_name, self.overview, self.purpose
        );
        for (title, items) in [
            ("Architecture", &self.architecture),
            ("Applications", &self.applications),
            ("Entry Points", &self.entry_points),
            ("Features", &self.features),
            ("Domains", &self.domains),
            ("Main Flows", &self.main_flows),
            ("Modules", &self.modules),
            ("APIs", &self.apis),
            ("Data Model", &self.data_model),
            ("External Services", &self.external_services),
            ("Workers", &self.workers),
            ("Events", &self.events),
            ("Configuration", &self.configuration),
            ("Security Notes", &self.security_notes),
            ("Development", &self.development),
            ("Deployment", &self.deployment),
            ("Known Limitations", &self.known_limitations),
        ] {
            value.push_str(&section(title, items));
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::project_graph::ProjectGraph;
    #[test]
    fn docs_have_stable_hash_and_markdown() {
        let graph = ProjectGraph::new("/tmp/demo".into(), vec![], vec![], vec![], vec![], vec![]);
        let a = build_project_documentation("p", &graph);
        let b = build_project_documentation("p", &graph);
        assert_eq!(a.content_hash, b.content_hash);
        assert!(a.markdown().contains("# demo"));
    }
}
