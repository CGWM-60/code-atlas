use crate::{
    graph::{
        architecture::{ArchitectureRole, analyze_architecture, quality_metrics},
        project_graph::ProjectGraph,
    },
    model::{
        edge::{CodeEdge, RelationKind},
        node::{CodeNode, NodeKind},
        project::{ProjectFile, SourceScope},
    },
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectMap {
    pub root: String,
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<CodeEdge>,
    pub zones: Vec<String>,
    pub entry_points: usize,
    pub full_node_count: usize,
    pub full_edge_count: usize,
    pub connected_components: usize,
    pub isolated_nodes: usize,
}

pub fn detect_entry_points(
    root: &Path,
    files: &[ProjectFile],
    nodes: &[CodeNode],
    edges: &[CodeEdge],
) -> (Vec<CodeNode>, Vec<CodeEdge>) {
    let cargo_targets = edges
        .iter()
        .filter(|edge| edge.relation == RelationKind::EntryPoint)
        .map(|edge| edge.target_id.as_str())
        .collect::<HashSet<_>>();
    let project_id = nodes
        .iter()
        .find(|node| node.kind == NodeKind::Project)
        .map(|node| node.id.clone());
    let mut entry_nodes = Vec::new();
    let mut entry_edges = Vec::new();
    for file in files {
        let file_id = format!("file:{}", file.path);
        let source = fs::read_to_string(root.join(&file.path)).unwrap_or_default();
        let normalized = file.path.replace('\\', "/").to_lowercase();
        let package_context = has_javascript_application_context(root, &file.path);
        let detected = cargo_targets.contains(file_id.as_str())
            || (file.language.as_str() == "Go"
                && source.contains("package main")
                && source.contains("func main("))
            || (file.language.as_str() == "Dart" && source.contains("void main("))
            || (file.language.as_str() == "Python"
                && ((source.contains("__name__") && source.contains("__main__"))
                    || source.contains("uvicorn.run(")
                    || source.contains("app = FastAPI(")
                    || source.contains("application = get_wsgi_application(")))
            || (file.language.as_str() == "PHP"
                && (normalized.ends_with("public/index.php")
                    || normalized.ends_with("/artisan")
                    || normalized == "artisan"
                    || normalized.ends_with("bin/console")))
            || (matches!(file.language.as_str(), "TypeScript" | "JavaScript")
                && ((is_standard_javascript_entry(&normalized) && package_context)
                    || source.contains(".listen(")
                    || source.contains("addEventListener('fetch'")
                    || source.contains("addEventListener(\"fetch\"")));
        if !detected {
            continue;
        }
        let id = format!("entrypoint:{}", file.path);
        entry_nodes.push(CodeNode {
            id: id.clone(),
            kind: NodeKind::EntryPoint,
            name: Path::new(&file.path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&file.path)
                .to_string(),
            path: Some(file.path.clone()),
            language: Some(file.language),
            start_line: Some(1),
            end_line: Some(1),
            owner: None,
            source_scope: file.source_scope,
        });
        entry_edges.push(CodeEdge::new(id.clone(), file_id, RelationKind::EntryPoint));
        if let Some(project_id) = &project_id {
            entry_edges.push(CodeEdge::new(
                project_id.clone(),
                id,
                RelationKind::Contains,
            ));
        }
    }
    (entry_nodes, entry_edges)
}

fn is_standard_javascript_entry(path: &str) -> bool {
    [
        "src/main.ts",
        "src/main.tsx",
        "src/main.js",
        "src/main.jsx",
        "server.ts",
        "server.js",
    ]
    .iter()
    .any(|candidate| path == *candidate || path.ends_with(&format!("/{candidate}")))
}

fn has_javascript_application_context(root: &Path, relative_path: &str) -> bool {
    let mut current = root.join(relative_path).parent().map(Path::to_path_buf);
    while let Some(directory) = current {
        if directory.join("package.json").is_file()
            || directory.join("vite.config.ts").is_file()
            || directory.join("vite.config.js").is_file()
            || directory.join("next.config.js").is_file()
            || directory.join("next.config.mjs").is_file()
        {
            return true;
        }
        if directory == root {
            break;
        }
        current = directory.parent().map(Path::to_path_buf);
    }
    false
}

pub fn build_project_map(graph: &ProjectGraph) -> ProjectMap {
    let analysis = analyze_architecture(graph);
    let project = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Project);
    let mut nodes = project.cloned().into_iter().collect::<Vec<_>>();
    let mut edges = Vec::new();
    let project_id = project.map(|node| node.id.clone());

    let mut architectural_nodes = graph
        .nodes
        .iter()
        .filter_map(|node| {
            let assignment = analysis.roles.get(&node.id)?;
            (!matches!(assignment.role, ArchitectureRole::Unknown)
                && !matches!(
                    node.kind,
                    NodeKind::Project | NodeKind::File | NodeKind::Method | NodeKind::Constructor
                )
                && (node.kind != NodeKind::Function
                    || matches!(
                        assignment.role,
                        ArchitectureRole::EntryPoint
                            | ArchitectureRole::Handler
                            | ArchitectureRole::Command
                            | ArchitectureRole::Worker
                            | ArchitectureRole::Job
                    )))
            .then(|| {
                let mut projected = node.clone();
                projected.kind = assignment.role.projected_kind(node.kind);
                projected
            })
        })
        .collect::<Vec<_>>();

    // Routes remain in the projection as collapsed children of their controller.
    // They are not rendered in the initial Architecture view.
    for route in graph
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::Route | NodeKind::ApiEndpoint))
    {
        if !architectural_nodes.iter().any(|node| node.id == route.id) {
            architectural_nodes.push(route.clone());
        }
    }

    architectural_nodes.sort_by(|left, right| {
        analysis
            .importance
            .get(&right.id)
            .cmp(&analysis.importance.get(&left.id))
            .then_with(|| left.name.cmp(&right.name))
    });
    let architectural_ids = architectural_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    nodes.extend(architectural_nodes);

    let mut groups = HashMap::<(String, String), Vec<String>>::new();
    for node_id in &architectural_ids {
        let Some(assignment) = analysis.roles.get(node_id) else {
            continue;
        };
        groups
            .entry((assignment.role.zone().into(), assignment.domain.clone()))
            .or_default()
            .push(node_id.clone());
    }
    // A codebase with hundreds of one-class domains falls back to role groups.
    // This is a projection rule, never a truncation of the full graph.
    if groups.len() > 120 {
        groups.clear();
        for node_id in &architectural_ids {
            let Some(assignment) = analysis.roles.get(node_id) else {
                continue;
            };
            groups
                .entry((
                    assignment.role.zone().into(),
                    format!("{:?}", assignment.role),
                ))
                .or_default()
                .push(node_id.clone());
        }
    }
    let mut grouped_routes = HashSet::new();
    for (route, controller) in &analysis.route_owners {
        if architectural_ids.contains(route) && architectural_ids.contains(controller) {
            grouped_routes.insert(route.clone());
            edges.push(CodeEdge::new(
                controller.clone(),
                route.clone(),
                RelationKind::Contains,
            ));
        }
    }
    // Ungrouped routes are explicit unresolved architecture, never invented handlers.
    let unresolved_routes = architectural_ids
        .iter()
        .filter(|id| {
            graph
                .find_node(id)
                .is_some_and(|node| matches!(node.kind, NodeKind::Route | NodeKind::ApiEndpoint))
                && !grouped_routes.contains(*id)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !unresolved_routes.is_empty() {
        groups.insert(
            ("API / ROUTING".into(), "Unresolved routes".into()),
            unresolved_routes,
        );
    }

    let zone_order = [
        "ENTRY POINTS",
        "UI / FRONTEND",
        "API / ROUTING",
        "APPLICATION",
        "DOMAIN",
        "PERSISTENCE",
        "EVENTS / QUEUES",
        "WORKERS / JOBS",
        "CLI",
        "EXTERNAL SERVICES",
        "INFRASTRUCTURE",
        "TESTS",
        "SHARED",
        "UNKNOWN",
    ];
    let mut zones = Vec::new();
    for zone in zone_order {
        let mut zone_groups = groups
            .iter()
            .filter(|((group_zone, _), _)| group_zone == zone)
            .collect::<Vec<_>>();
        if zone_groups.is_empty() {
            continue;
        }
        zone_groups.sort_by(|left, right| left.0.1.cmp(&right.0.1));
        zones.push(zone.to_string());
        let zone_id = synthetic_id("zone", zone);
        nodes.push(synthetic_node(
            zone_id.clone(),
            NodeKind::ArchitectureZone,
            zone,
        ));
        if let Some(project_id) = &project_id {
            edges.push(CodeEdge::new(
                project_id.clone(),
                zone_id.clone(),
                RelationKind::Contains,
            ));
        }
        for ((_, domain), members) in zone_groups {
            let group_id = synthetic_id("architecture-group", &format!("{zone}:{domain}"));
            nodes.push(synthetic_node(
                group_id.clone(),
                NodeKind::ArchitectureGroup,
                &format!("{domain} · {}", members.len()),
            ));
            edges.push(CodeEdge::new(
                zone_id.clone(),
                group_id.clone(),
                RelationKind::Contains,
            ));
            for member in members {
                // Routes owned by a controller are children of that controller, not duplicated.
                if !grouped_routes.contains(member) {
                    edges.push(CodeEdge::new(
                        group_id.clone(),
                        member.clone(),
                        RelationKind::Contains,
                    ));
                }
            }
        }
    }

    let entry_specs: [(&str, &str, &[ArchitectureRole]); 6] = [
        (
            "Web App",
            "UI / FRONTEND",
            &[
                ArchitectureRole::Frontend,
                ArchitectureRole::Component,
                ArchitectureRole::Page,
            ],
        ),
        (
            "API",
            "API / ROUTING",
            &[
                ArchitectureRole::Api,
                ArchitectureRole::Route,
                ArchitectureRole::Controller,
                ArchitectureRole::Handler,
            ],
        ),
        (
            "Worker",
            "WORKERS / JOBS",
            &[ArchitectureRole::Worker, ArchitectureRole::Job],
        ),
        (
            "CLI",
            "CLI",
            &[ArchitectureRole::Command, ArchitectureRole::Cli],
        ),
        (
            "Events",
            "EVENTS / QUEUES",
            &[
                ArchitectureRole::Queue,
                ArchitectureRole::Event,
                ArchitectureRole::Listener,
                ArchitectureRole::MessageHandler,
            ],
        ),
        ("Tests", "TESTS", &[ArchitectureRole::Test]),
    ];
    for (name, zone, roles) in entry_specs {
        if !analysis
            .roles
            .values()
            .any(|assignment| roles.contains(&assignment.role))
        {
            continue;
        }
        let entry_id = synthetic_id("entrypoint", name);
        nodes.push(synthetic_node(entry_id.clone(), NodeKind::EntryPoint, name));
        if let Some(project_id) = &project_id {
            edges.push(CodeEdge::new(
                project_id.clone(),
                entry_id.clone(),
                RelationKind::Contains,
            ));
        }
        edges.push(CodeEdge::new(
            entry_id,
            synthetic_id("zone", zone),
            RelationKind::RoutesTo,
        ));
    }

    // Preserve every real detected entry point and connect it to the project.
    for entry in nodes
        .iter()
        .filter(|node| node.kind == NodeKind::EntryPoint)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>()
    {
        if let Some(project_id) = &project_id {
            edges.push(CodeEdge::new(
                project_id.clone(),
                entry,
                RelationKind::Contains,
            ));
        }
    }

    edges.extend(analysis.flows.into_iter().filter(|edge| {
        let source = architectural_ids.contains(&edge.source_id)
            || nodes.iter().any(|node| node.id == edge.source_id);
        let target = architectural_ids.contains(&edge.target_id)
            || nodes.iter().any(|node| node.id == edge.target_id);
        source && target
    }));
    let mut seen = HashSet::new();
    nodes.retain(|node| seen.insert(node.id.clone()));
    let node_ids = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    edges.retain(|edge| {
        node_ids.contains(edge.source_id.as_str())
            && node_ids.contains(edge.target_id.as_str())
            && seen.insert(edge.id.clone())
    });
    let (connected_components, isolated_nodes) = quality_metrics(&nodes, &edges);
    ProjectMap {
        root: graph.root.clone(),
        entry_points: nodes
            .iter()
            .filter(|node| node.kind == NodeKind::EntryPoint)
            .count(),
        nodes,
        edges,
        zones,
        full_node_count: graph.nodes.len(),
        full_edge_count: graph.edges.len(),
        connected_components,
        isolated_nodes,
    }
}

fn synthetic_id(prefix: &str, name: &str) -> String {
    format!(
        "{prefix}:{}",
        name.to_lowercase().replace([' ', '/', ':'], "-")
    )
}

fn synthetic_node(id: String, kind: NodeKind, name: &str) -> CodeNode {
    CodeNode {
        id,
        kind,
        name: name.to_string(),
        path: None,
        language: None,
        start_line: None,
        end_line: None,
        owner: None,
        source_scope: SourceScope::Project,
    }
}
