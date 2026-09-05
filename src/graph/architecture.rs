use crate::{
    graph::project_graph::ProjectGraph,
    model::{
        edge::{CodeEdge, RelationKind},
        node::{CodeNode, NodeKind},
        project::SourceScope,
    },
};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

/// Stack-agnostic architecture vocabulary. Framework adapters enrich this model;
/// no adapter owns the projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ArchitectureRole {
    Application,
    EntryPoint,
    Frontend,
    Api,
    Route,
    Controller,
    Handler,
    Command,
    Service,
    Repository,
    Model,
    Entity,
    Database,
    Provider,
    Component,
    Page,
    Worker,
    Job,
    Queue,
    Event,
    Listener,
    MessageHandler,
    Template,
    Cli,
    Infrastructure,
    Test,
    Shared,
    External,
    Unknown,
}

impl ArchitectureRole {
    pub fn zone(self) -> &'static str {
        match self {
            Self::Application | Self::EntryPoint => "ENTRY POINTS",
            Self::Frontend | Self::Component | Self::Page | Self::Template => "UI / FRONTEND",
            Self::Api | Self::Route | Self::Controller | Self::Handler => "API / ROUTING",
            Self::Service | Self::Provider => "APPLICATION",
            Self::Model | Self::Entity => "DOMAIN",
            Self::Repository | Self::Database => "PERSISTENCE",
            Self::Queue | Self::Event | Self::Listener | Self::MessageHandler => "EVENTS / QUEUES",
            Self::Worker | Self::Job => "WORKERS / JOBS",
            Self::Command | Self::Cli => "CLI",
            Self::External => "EXTERNAL SERVICES",
            Self::Infrastructure => "INFRASTRUCTURE",
            Self::Test => "TESTS",
            Self::Shared => "SHARED",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn projected_kind(self, original: NodeKind) -> NodeKind {
        match self {
            Self::EntryPoint => NodeKind::EntryPoint,
            Self::Controller => NodeKind::Controller,
            Self::Handler => NodeKind::Handler,
            Self::Command => NodeKind::Command,
            Self::Service => NodeKind::Service,
            Self::Repository => NodeKind::Repository,
            Self::Model => NodeKind::Model,
            Self::Entity => NodeKind::Entity,
            Self::Database => NodeKind::Database,
            Self::Provider => NodeKind::Provider,
            Self::Component => NodeKind::Component,
            Self::Page => NodeKind::Page,
            Self::Worker => NodeKind::Worker,
            Self::Job => NodeKind::Job,
            Self::Queue => NodeKind::Queue,
            Self::Event => NodeKind::Event,
            Self::Listener => NodeKind::Listener,
            Self::MessageHandler => NodeKind::MessageHandler,
            Self::Template => NodeKind::Template,
            Self::Cli => NodeKind::Cli,
            Self::Infrastructure => NodeKind::Infrastructure,
            Self::Shared => NodeKind::Shared,
            Self::External => NodeKind::ExternalService,
            _ => original,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoleAssignment {
    pub role: ArchitectureRole,
    pub domain: String,
    pub confidence: u8,
    pub signals: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct ArchitectureAnalysis {
    pub roles: HashMap<String, RoleAssignment>,
    pub importance: HashMap<String, i32>,
    pub flows: Vec<CodeEdge>,
    pub route_owners: HashMap<String, String>,
}

pub fn classify_architecture_roles(graph: &ProjectGraph) -> HashMap<String, RoleAssignment> {
    let mut roles = graph
        .nodes
        .iter()
        .map(|node| {
            let (role, confidence, signals) = classify_role_with_evidence(node);
            (
                node.id.clone(),
                RoleAssignment {
                    role,
                    domain: detect_domain(node, role),
                    confidence,
                    signals,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    // Adapter and graph relations correct the heuristic first pass only when
    // they provide stronger, unambiguous evidence.
    for edge in &graph.edges {
        let role = match edge.relation {
            RelationKind::HandledBy => Some(ArchitectureRole::Handler),
            RelationKind::Listens => Some(ArchitectureRole::Listener),
            RelationKind::EntryPoint => Some(ArchitectureRole::EntryPoint),
            _ => None,
        };
        let Some(role) = role else { continue };
        let Some(target) = graph.find_node(&edge.target_id) else {
            continue;
        };
        let Some(assignment) = roles.get_mut(&edge.target_id) else {
            continue;
        };
        if target.kind == NodeKind::Function || assignment.role == ArchitectureRole::Unknown {
            assignment.role = role;
            assignment.domain = detect_domain(target, role);
            assignment.confidence = 95;
            assignment.signals = vec!["validated graph relation"];
        }
    }
    roles
}

pub fn score_architectural_importance(
    graph: &ProjectGraph,
    roles: &HashMap<String, RoleAssignment>,
) -> HashMap<String, i32> {
    graph
        .nodes
        .iter()
        .map(|node| {
            let assignment = roles.get(&node.id);
            let base = match assignment.map(|value| value.role) {
                Some(ArchitectureRole::EntryPoint) => 100,
                Some(ArchitectureRole::Api | ArchitectureRole::Route) => 88,
                Some(ArchitectureRole::Controller | ArchitectureRole::Handler) => 82,
                Some(
                    ArchitectureRole::Worker | ArchitectureRole::Job | ArchitectureRole::Command,
                ) => 78,
                Some(
                    ArchitectureRole::Service
                    | ArchitectureRole::Provider
                    | ArchitectureRole::MessageHandler,
                ) => 72,
                Some(ArchitectureRole::Repository | ArchitectureRole::Database) => 68,
                Some(
                    ArchitectureRole::Frontend
                    | ArchitectureRole::Component
                    | ArchitectureRole::Page,
                ) => 64,
                Some(
                    ArchitectureRole::Model
                    | ArchitectureRole::Entity
                    | ArchitectureRole::Queue
                    | ArchitectureRole::Event
                    | ArchitectureRole::Listener,
                ) => 58,
                Some(
                    ArchitectureRole::Application
                    | ArchitectureRole::Infrastructure
                    | ArchitectureRole::External,
                ) => 50,
                Some(ArchitectureRole::Template | ArchitectureRole::Shared) => 38,
                Some(ArchitectureRole::Test) => 15,
                _ => 5,
            };
            let meaningful_degree = graph
                .incoming_edges(&node.id)
                .into_iter()
                .chain(graph.outgoing_edges(&node.id))
                .filter(|edge| {
                    !matches!(
                        edge.relation,
                        RelationKind::Contains | RelationKind::Declares | RelationKind::HasMethod
                    )
                })
                .count()
                .min(12) as i32;
            let confidence = assignment.map_or(0, |value| i32::from(value.confidence) / 10);
            (
                node.id.clone(),
                base + meaningful_degree * 2 + confidence - noise_penalty(node),
            )
        })
        .collect()
}

pub fn select_architecture_flows(
    graph: &ProjectGraph,
    roles: &HashMap<String, RoleAssignment>,
) -> (Vec<CodeEdge>, HashMap<String, String>) {
    let owner_by_symbol = owner_anchors(graph, roles);
    let mut flows = Vec::new();
    let mut route_owners = HashMap::new();
    for edge in &graph.edges {
        let source = architectural_anchor(&edge.source_id, &owner_by_symbol);
        let target = architectural_anchor(&edge.target_id, &owner_by_symbol);
        if source == target {
            continue;
        }
        let relation = match edge.relation {
            RelationKind::Calls | RelationKind::Imports | RelationKind::DependsOn => {
                RelationKind::Uses
            }
            RelationKind::HandledBy
            | RelationKind::Renders
            | RelationKind::Reads
            | RelationKind::Writes
            | RelationKind::Creates
            | RelationKind::Emits
            | RelationKind::Listens
            | RelationKind::RoutesTo
            | RelationKind::EntryPoint => edge.relation,
            _ => continue,
        };
        if matches!(
            graph.find_node(&edge.source_id).map(|node| node.kind),
            Some(NodeKind::Route | NodeKind::ApiEndpoint)
        ) && relation == RelationKind::HandledBy
        {
            route_owners.insert(edge.source_id.clone(), target.clone());
        }
        flows.push(CodeEdge::new(source, target, relation));
    }

    // Cross-language first-party import lifting is accepted only for unique targets.
    let declarations_by_name = graph
        .nodes
        .iter()
        .filter(|node| is_architectural_declaration(node))
        .fold(
            HashMap::<&str, Vec<&CodeNode>>::new(),
            |mut values, node| {
                values.entry(&node.name).or_default().push(node);
                values
            },
        );
    let declaration_by_path = graph
        .nodes
        .iter()
        .filter(|node| is_architectural_declaration(node))
        .filter_map(|node| Some((node.path.as_deref()?, node)))
        .fold(
            HashMap::<&str, Vec<&CodeNode>>::new(),
            |mut values, (path, node)| {
                values.entry(path).or_default().push(node);
                values
            },
        );
    for import in &graph.imports {
        let target_name = import
            .full_path
            .rsplit(['\\', '/', ':', '.'])
            .find(|part| !part.is_empty())
            .unwrap_or_default();
        let Some(targets) = declarations_by_name
            .get(target_name)
            .filter(|items| items.len() == 1)
        else {
            continue;
        };
        let Some(sources) = declaration_by_path
            .get(import.source_path.as_str())
            .filter(|items| items.len() == 1)
        else {
            continue;
        };
        if sources[0].id != targets[0].id {
            flows.push(CodeEdge::new(
                sources[0].id.clone(),
                targets[0].id.clone(),
                RelationKind::Uses,
            ));
        }
    }
    dedupe_edges(&mut flows);
    (flows, route_owners)
}

pub fn analyze_architecture(graph: &ProjectGraph) -> ArchitectureAnalysis {
    let roles = classify_architecture_roles(graph);
    let importance = score_architectural_importance(graph, &roles);
    let (flows, route_owners) = select_architecture_flows(graph, &roles);
    ArchitectureAnalysis {
        roles,
        importance,
        flows,
        route_owners,
    }
}

pub fn classify_role(node: &CodeNode) -> ArchitectureRole {
    classify_role_with_evidence(node).0
}

fn classify_role_with_evidence(node: &CodeNode) -> (ArchitectureRole, u8, Vec<&'static str>) {
    let path = node.path.as_deref().unwrap_or_default().replace('\\', "/");
    let lower_path = path.to_lowercase();
    let lower_name = node.name.to_lowercase();
    let segments = lower_path.split('/').collect::<Vec<_>>();
    let has_segment = |names: &[&str]| segments.iter().any(|segment| names.contains(segment));

    if node.kind == NodeKind::Project {
        return (ArchitectureRole::Application, 100, vec!["node kind"]);
    }
    if node.kind == NodeKind::EntryPoint {
        return (
            ArchitectureRole::EntryPoint,
            100,
            vec!["detected entry point"],
        );
    }
    if node.source_scope == SourceScope::ExternalDependency {
        return (
            ArchitectureRole::External,
            98,
            vec!["external source scope"],
        );
    }
    if has_segment(&["test", "tests", "spec", "specs", "__tests__"]) {
        return (ArchitectureRole::Test, 95, vec!["test source scope/path"]);
    }

    let kind_role = match node.kind {
        NodeKind::ApiEndpoint => Some(ArchitectureRole::Api),
        NodeKind::Route => Some(ArchitectureRole::Route),
        NodeKind::Controller => Some(ArchitectureRole::Controller),
        NodeKind::Handler => Some(ArchitectureRole::Handler),
        NodeKind::Command => Some(ArchitectureRole::Command),
        NodeKind::Service => Some(ArchitectureRole::Service),
        NodeKind::Repository => Some(ArchitectureRole::Repository),
        NodeKind::Model => Some(ArchitectureRole::Model),
        NodeKind::Entity => Some(ArchitectureRole::Entity),
        NodeKind::Database | NodeKind::DatabaseTable => Some(ArchitectureRole::Database),
        NodeKind::Provider => Some(ArchitectureRole::Provider),
        NodeKind::Component => Some(ArchitectureRole::Component),
        NodeKind::Page => Some(ArchitectureRole::Page),
        NodeKind::Worker => Some(ArchitectureRole::Worker),
        NodeKind::Job => Some(ArchitectureRole::Job),
        NodeKind::Queue => Some(ArchitectureRole::Queue),
        NodeKind::Event => Some(ArchitectureRole::Event),
        NodeKind::Listener | NodeKind::EventListener | NodeKind::EventSubscriber => {
            Some(ArchitectureRole::Listener)
        }
        NodeKind::MessageHandler => Some(ArchitectureRole::MessageHandler),
        NodeKind::Template => Some(ArchitectureRole::Template),
        NodeKind::Cli => Some(ArchitectureRole::Cli),
        NodeKind::Infrastructure | NodeKind::Config | NodeKind::Environment => {
            Some(ArchitectureRole::Infrastructure)
        }
        NodeKind::ExternalService => Some(ArchitectureRole::External),
        NodeKind::Shared => Some(ArchitectureRole::Shared),
        NodeKind::Workspace | NodeKind::Package | NodeKind::Crate | NodeKind::Module => {
            Some(ArchitectureRole::Application)
        }
        _ => None,
    };
    if let Some(role) = kind_role {
        return (role, 95, vec!["framework/generic node kind"]);
    }

    let path_or_suffix = |folders: &[&str], suffixes: &[&str]| {
        has_segment(folders) || suffixes.iter().any(|suffix| lower_name.ends_with(suffix))
    };
    let inferred = if path_or_suffix(&["messagehandler", "messagehandlers"], &["messagehandler"]) {
        ArchitectureRole::MessageHandler
    } else if path_or_suffix(&["controller", "controllers"], &["controller"]) {
        ArchitectureRole::Controller
    } else if path_or_suffix(&["handler", "handlers"], &["handler"]) {
        ArchitectureRole::Handler
    } else if path_or_suffix(
        &["command", "commands", "console", "cli"],
        &["command", "cli"],
    ) {
        ArchitectureRole::Command
    } else if path_or_suffix(&["worker", "workers"], &["worker"]) {
        ArchitectureRole::Worker
    } else if path_or_suffix(
        &["job", "jobs", "scheduler", "schedulers"],
        &["job", "scheduler"],
    ) {
        ArchitectureRole::Job
    } else if path_or_suffix(
        &[
            "listener",
            "listeners",
            "subscriber",
            "subscribers",
            "eventsubscriber",
        ],
        &["listener", "subscriber"],
    ) {
        ArchitectureRole::Listener
    } else if path_or_suffix(
        &["repository", "repositories", "persistence", "dao"],
        &["repository", "dao"],
    ) {
        ArchitectureRole::Repository
    } else if path_or_suffix(
        &["service", "services", "usecase", "usecases", "application"],
        &["service", "usecase"],
    ) {
        ArchitectureRole::Service
    } else if path_or_suffix(&["entity", "entities"], &["entity"]) {
        ArchitectureRole::Entity
    } else if path_or_suffix(&["model", "models", "domain"], &["model"]) {
        ArchitectureRole::Model
    } else if path_or_suffix(
        &["provider", "providers", "notifier", "notifiers"],
        &["provider", "notifier"],
    ) {
        ArchitectureRole::Provider
    } else if path_or_suffix(
        &["component", "components", "widget", "widgets"],
        &["component", "widget"],
    ) {
        ArchitectureRole::Component
    } else if path_or_suffix(
        &["page", "pages", "screen", "screens", "view", "views"],
        &["page", "screen", "view"],
    ) {
        ArchitectureRole::Page
    } else if path_or_suffix(
        &["route", "routes", "router", "routers", "api"],
        &["router", "route"],
    ) {
        ArchitectureRole::Route
    } else if path_or_suffix(&["event", "events"], &["event"]) {
        ArchitectureRole::Event
    } else if path_or_suffix(&["queue", "queues"], &["queue"]) {
        ArchitectureRole::Queue
    } else if path_or_suffix(
        &["database", "db", "migration", "migrations"],
        &["database"],
    ) {
        ArchitectureRole::Database
    } else if path_or_suffix(&["template", "templates"], &["template"])
        || lower_path.ends_with(".twig")
    {
        ArchitectureRole::Template
    } else if has_segment(&["infra", "infrastructure", "config", "deploy", "deployment"]) {
        ArchitectureRole::Infrastructure
    } else if has_segment(&["shared", "common", "core"]) {
        ArchitectureRole::Shared
    } else {
        ArchitectureRole::Unknown
    };
    if inferred == ArchitectureRole::Unknown {
        (inferred, 0, vec![])
    } else {
        (inferred, 65, vec!["path/name heuristic"])
    }
}

pub fn detect_domain(node: &CodeNode, role: ArchitectureRole) -> String {
    let normalized = node.path.as_deref().unwrap_or_default().replace('\\', "/");
    if matches!(
        role,
        ArchitectureRole::Frontend
            | ArchitectureRole::Component
            | ArchitectureRole::Page
            | ArchitectureRole::Template
    ) {
        let generic_ui = [
            "src",
            "app",
            "apps",
            "ui",
            "frontend",
            "component",
            "components",
            "page",
            "pages",
            "template",
            "templates",
        ];
        if let Some(module) = normalized
            .split('/')
            .rev()
            .skip(1)
            .find(|segment| !generic_ui.contains(&segment.to_lowercase().as_str()))
        {
            return module.to_string();
        }
        return "UI".into();
    }
    let suffixes = [
        "controller",
        "handler",
        "command",
        "service",
        "repository",
        "entity",
        "model",
        "provider",
        "notifier",
        "component",
        "widget",
        "page",
        "screen",
        "view",
        "worker",
        "job",
        "listener",
        "subscriber",
        "event",
        "queue",
    ];
    let raw_name = node.owner.as_deref().unwrap_or(&node.name);
    let mut domain = raw_name
        .trim_end_matches(|character: char| !character.is_alphanumeric())
        .to_string();
    let lower = domain.to_lowercase();
    if let Some(suffix) = suffixes.iter().find(|suffix| lower.ends_with(**suffix)) {
        domain.truncate(domain.len().saturating_sub(suffix.len()));
    }
    domain = domain.trim_matches(['_', '-', '.']).to_string();
    if !domain.is_empty() && !is_generic_label(&domain) {
        return domain;
    }

    let ignored = [
        "src", "app", "apps", "lib", "packages", "pkg", "internal", "backend", "frontend",
    ];
    let role_folders = [
        "controller",
        "controllers",
        "handler",
        "handlers",
        "service",
        "services",
        "repository",
        "repositories",
        "entity",
        "entities",
        "model",
        "models",
        "domain",
        "command",
        "commands",
        "worker",
        "workers",
        "job",
        "jobs",
        "event",
        "events",
        "listener",
        "listeners",
        "provider",
        "providers",
        "page",
        "pages",
        "component",
        "components",
    ];
    for segment in normalized
        .split('/')
        .filter(|segment| !segment.contains('.'))
    {
        let lower = segment.to_lowercase();
        if !ignored.contains(&lower.as_str())
            && !role_folders.contains(&lower.as_str())
            && !segment.is_empty()
        {
            return segment.to_string();
        }
    }
    match role {
        ArchitectureRole::Application => "Application".into(),
        ArchitectureRole::Infrastructure => "Infrastructure".into(),
        ArchitectureRole::External => "External".into(),
        ArchitectureRole::Test => "Tests".into(),
        _ => "Shared".into(),
    }
}

fn is_generic_label(value: &str) -> bool {
    [
        "service",
        "controller",
        "handler",
        "repository",
        "model",
        "entity",
        "provider",
        "worker",
        "job",
        "page",
        "component",
        "application",
    ]
    .contains(&value.to_lowercase().as_str())
}

fn noise_penalty(node: &CodeNode) -> i32 {
    let name = node.name.to_lowercase();
    let generic = [
        "json", "state", "result", "string", "error", "config", "context", "data", "value",
        "utils", "helper",
    ];
    let mut penalty = if generic.contains(&name.as_str()) {
        35
    } else {
        0
    };
    if matches!(
        node.kind,
        NodeKind::TypeAlias | NodeKind::Constant | NodeKind::Enum
    ) {
        penalty += 20;
    }
    if node.source_scope != SourceScope::Project
        && node.source_scope != SourceScope::WorkspacePackage
    {
        penalty += 30;
    }
    penalty
}

fn is_architectural_declaration(node: &CodeNode) -> bool {
    matches!(
        node.kind,
        NodeKind::Class
            | NodeKind::Struct
            | NodeKind::Interface
            | NodeKind::Trait
            | NodeKind::Service
            | NodeKind::Repository
            | NodeKind::Controller
            | NodeKind::Handler
            | NodeKind::Entity
            | NodeKind::Model
            | NodeKind::Provider
            | NodeKind::Worker
            | NodeKind::Job
    )
}

pub fn quality_metrics(nodes: &[CodeNode], edges: &[CodeEdge]) -> (usize, usize) {
    let ids = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let mut adjacency = HashMap::<&str, Vec<&str>>::new();
    for edge in edges {
        if ids.contains(edge.source_id.as_str()) && ids.contains(edge.target_id.as_str()) {
            adjacency
                .entry(&edge.source_id)
                .or_default()
                .push(&edge.target_id);
            adjacency
                .entry(&edge.target_id)
                .or_default()
                .push(&edge.source_id);
        }
    }
    let isolated = nodes
        .iter()
        .filter(|node| !adjacency.contains_key(node.id.as_str()))
        .count();
    let mut visited = HashSet::new();
    let mut components = 0;
    for node in nodes {
        if !visited.insert(node.id.as_str()) {
            continue;
        }
        components += 1;
        let mut queue = VecDeque::from([node.id.as_str()]);
        while let Some(current) = queue.pop_front() {
            for neighbor in adjacency.get(current).into_iter().flatten() {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
    }
    (components, isolated)
}

fn owner_anchors(
    graph: &ProjectGraph,
    roles: &HashMap<String, RoleAssignment>,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for node in &graph.nodes {
        let Some(owner) = node.owner.as_deref() else {
            continue;
        };
        let owners = graph
            .nodes
            .iter()
            .filter(|candidate| {
                candidate.name == owner
                    && candidate.path == node.path
                    && roles
                        .get(&candidate.id)
                        .is_some_and(|assignment| assignment.role != ArchitectureRole::Unknown)
            })
            .collect::<Vec<_>>();
        if owners.len() == 1 {
            result.insert(node.id.clone(), owners[0].id.clone());
        }
    }
    result
}

fn architectural_anchor(id: &str, owners: &HashMap<String, String>) -> String {
    owners.get(id).cloned().unwrap_or_else(|| id.to_string())
}

fn dedupe_edges(edges: &mut Vec<CodeEdge>) {
    let mut seen = HashSet::new();
    edges.retain(|edge| seen.insert(edge.id.clone()));
}
