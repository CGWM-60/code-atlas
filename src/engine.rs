use anyhow::Result;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    analyzer::{cargo_analyzer::analyze_cargo_project, dispatcher::analyze_file},
    framework::analyze_frameworks,
    graph::{
        call_resolver::resolve_calls,
        cargo_graph_builder::build_cargo_graph,
        edge_builder::{build_contains_edges, build_has_method_edges},
        module_resolver::resolve_rust_modules,
        project_graph::ProjectGraph,
        project_map::{build_project_map, detect_entry_points},
    },
    model::{
        analysis::FileAnalysis,
        edge::{CodeEdge, RelationKind},
        node::{CodeNode, NodeKind, build_file_nodes},
        project::ProjectScan,
    },
    scanner::project_scanner::scan_project_with_progress,
};

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisResult {
    pub graph: ProjectGraph,
    pub scan: ProjectScan,
    pub analyzed_files: usize,
    pub reused_files: usize,
    #[serde(skip)]
    pub file_analyses: HashMap<String, FileAnalysis>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectAnalyzer;

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisUpdate {
    pub stage: &'static str,
    pub processed: usize,
    pub total: usize,
    pub nodes: usize,
    pub edges: usize,
    pub current_file: Option<String>,
}

impl ProjectAnalyzer {
    pub fn analyze<P: AsRef<Path>>(&self, root: P) -> Result<AnalysisResult> {
        self.analyze_incremental(root, None)
    }

    /// Reuses immutable `FileAnalysis` values when content hashes match. Resolution is
    /// deliberately rerun globally because an edited export can affect other files.
    pub fn analyze_incremental<P: AsRef<Path>>(
        &self,
        root: P,
        previous: Option<&AnalysisResult>,
    ) -> Result<AnalysisResult> {
        self.analyze_incremental_with_progress(root, previous, |_| {})
    }

    pub fn analyze_incremental_with_progress<P, F>(
        &self,
        root: P,
        previous: Option<&AnalysisResult>,
        mut progress: F,
    ) -> Result<AnalysisResult>
    where
        P: AsRef<Path>,
        F: FnMut(AnalysisUpdate),
    {
        let mut last_scan_update = 0;
        let scan = scan_project_with_progress(root, |processed, total, path| {
            let interval = (total / 250).max(1);
            if processed == total || processed.saturating_sub(last_scan_update) >= interval {
                progress(AnalysisUpdate {
                    stage: "scanning",
                    processed,
                    total,
                    nodes: 0,
                    edges: 0,
                    current_file: Some(path.to_string()),
                });
                last_scan_update = processed;
            }
        })?;
        let root_path = Path::new(&scan.root);
        let total_files = scan.files.len();
        if total_files == 0 {
            progress(AnalysisUpdate {
                stage: "scanning",
                processed: 0,
                total: 0,
                nodes: 0,
                edges: 0,
                current_file: None,
            });
        }
        progress(AnalysisUpdate {
            stage: "detecting_languages",
            processed: total_files,
            total: total_files,
            nodes: 0,
            edges: 0,
            current_file: None,
        });
        progress(AnalysisUpdate {
            stage: "parsing",
            processed: 0,
            total: total_files,
            nodes: 0,
            edges: 0,
            current_file: None,
        });
        let previous_hashes: HashMap<_, _> = previous
            .map(|value| {
                value
                    .scan
                    .files
                    .iter()
                    .map(|file| (file.path.as_str(), file.hash.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        let mut file_analyses = HashMap::new();
        let mut analyzed_files = 0;
        let mut reused_files = 0;
        let mut discovered_nodes = 0;
        let mut discovered_edges = 0;
        let progress_interval = (total_files / 250).max(1);
        for (index, file) in scan.files.iter().enumerate() {
            if previous_hashes.get(file.path.as_str()) == Some(&file.hash.as_str())
                && let Some(cached) = previous.and_then(|value| value.file_analyses.get(&file.path))
            {
                discovered_nodes += cached.nodes.len();
                discovered_edges +=
                    cached.calls.len() + cached.imports.len() + cached.modules.len();
                file_analyses.insert(file.path.clone(), cached.clone());
                reused_files += 1;
            } else if let Some(analysis) = analyze_file(root_path, file)? {
                discovered_nodes += analysis.nodes.len();
                discovered_edges +=
                    analysis.calls.len() + analysis.imports.len() + analysis.modules.len();
                file_analyses.insert(file.path.clone(), analysis);
                analyzed_files += 1;
            }
            let processed = index + 1;
            if processed == total_files || processed % progress_interval == 0 {
                progress(AnalysisUpdate {
                    stage: "parsing",
                    processed,
                    total: total_files,
                    nodes: discovered_nodes,
                    edges: discovered_edges,
                    current_file: Some(file.path.clone()),
                });
            }
        }

        progress(AnalysisUpdate {
            stage: "extracting_symbols",
            processed: total_files,
            total: total_files,
            nodes: discovered_nodes,
            edges: discovered_edges,
            current_file: None,
        });
        progress(AnalysisUpdate {
            stage: "building_nodes",
            processed: total_files,
            total: total_files,
            nodes: discovered_nodes,
            edges: discovered_edges,
            current_file: None,
        });
        let file_nodes = build_file_nodes(&scan.files);
        let project_id = format!("project:{}", scan.root);
        let project_name = root_path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("project")
            .to_string();
        let project_node = CodeNode {
            id: project_id.clone(),
            kind: NodeKind::Project,
            name: project_name,
            path: None,
            language: None,
            start_line: None,
            end_line: None,
            owner: None,
            source_scope: crate::model::project::SourceScope::Project,
        };
        let mut nodes = vec![project_node];
        nodes.extend(file_nodes.clone());
        let mut calls = Vec::new();
        let mut imports = Vec::new();
        let mut module_refs = Vec::new();
        for analysis in file_analyses.values() {
            nodes.extend(analysis.nodes.clone());
            calls.extend(analysis.calls.clone());
            imports.extend(analysis.imports.clone());
            module_refs.extend(analysis.modules.clone());
        }
        progress(AnalysisUpdate {
            stage: "resolving_modules",
            processed: total_files,
            total: total_files,
            nodes: nodes.len(),
            edges: imports.len() + module_refs.len(),
            current_file: None,
        });
        let modules = resolve_rust_modules(&module_refs, &scan.files);
        let mut edges: Vec<_> = file_nodes
            .iter()
            .map(|file| CodeEdge::new(project_id.clone(), file.id.clone(), RelationKind::Contains))
            .collect();
        edges.extend(build_contains_edges(&file_nodes, &nodes));
        edges.extend(build_has_method_edges(&nodes));
        for module in &modules {
            let module_node = CodeNode {
                id: format!("module:{}", module.module_path),
                kind: NodeKind::Module,
                name: module.module_path.clone(),
                path: module
                    .target_path
                    .clone()
                    .or_else(|| Some(module.declared_in.clone())),
                language: Some(crate::language::ProgrammingLanguage::Rust),
                start_line: Some(module.line),
                end_line: Some(module.line),
                owner: None,
                source_scope: crate::model::project::SourceScope::Project,
            };
            if let Some(file) = file_nodes
                .iter()
                .find(|node| node.path.as_deref() == Some(&module.declared_in))
            {
                edges.push(CodeEdge::new(
                    file.id.clone(),
                    module_node.id.clone(),
                    RelationKind::Declares,
                ));
            }
            nodes.push(module_node);
        }
        progress(AnalysisUpdate {
            stage: "resolving_imports",
            processed: total_files,
            total: total_files,
            nodes: nodes.len(),
            edges: edges.len(),
            current_file: None,
        });
        edges.extend(resolve_import_edges(&scan.root, &file_nodes, &imports));
        progress(AnalysisUpdate {
            stage: "resolving_calls",
            processed: total_files,
            total: total_files,
            nodes: nodes.len(),
            edges: edges.len(),
            current_file: None,
        });
        let resolved = resolve_calls(&nodes, &calls, &imports, &modules);
        edges.extend(resolved.edges);
        let mut unresolved = resolved.unresolved;
        resolve_generic_calls(root_path, &nodes, &mut edges, &mut unresolved);

        if let Some(workspace) = analyze_cargo_project(root_path)? {
            let cargo = build_cargo_graph(&workspace, &file_nodes);
            edges.extend(cargo.edges);
            nodes.extend(cargo.nodes);
        }
        progress(AnalysisUpdate {
            stage: "framework_analysis",
            processed: total_files,
            total: total_files,
            nodes: nodes.len(),
            edges: edges.len(),
            current_file: None,
        });
        let framework = analyze_frameworks(root_path, &scan.files, &nodes)?;
        nodes.extend(framework.nodes);
        edges.extend(framework.edges);
        let (entry_nodes, entry_edges) =
            detect_entry_points(root_path, &scan.files, &nodes, &edges);
        nodes.extend(entry_nodes);
        edges.extend(entry_edges);
        dedupe(&mut nodes, &mut edges);
        progress(AnalysisUpdate {
            stage: "building_graph",
            processed: total_files,
            total: total_files,
            nodes: nodes.len(),
            edges: edges.len(),
            current_file: None,
        });
        let graph = ProjectGraph::new(
            scan.root.clone(),
            nodes,
            edges,
            imports,
            modules,
            unresolved,
        );
        let project_map = build_project_map(&graph);
        progress(AnalysisUpdate {
            stage: "building_map",
            processed: total_files,
            total: total_files,
            nodes: project_map.nodes.len(),
            edges: project_map.edges.len(),
            current_file: None,
        });
        Ok(AnalysisResult {
            graph,
            scan,
            analyzed_files,
            reused_files,
            file_analyses,
        })
    }
}

fn dedupe(nodes: &mut Vec<CodeNode>, edges: &mut Vec<CodeEdge>) {
    let mut seen = HashSet::new();
    nodes.retain(|node| seen.insert(node.id.clone()));
    let mut seen = HashSet::new();
    edges.retain(|edge| seen.insert(edge.id.clone()));
}

fn resolve_generic_calls(
    root: &Path,
    nodes: &[CodeNode],
    edges: &mut Vec<CodeEdge>,
    unresolved: &mut Vec<crate::model::call::CallReference>,
) {
    let eligible = |node: &CodeNode| {
        matches!(
            node.kind,
            NodeKind::Function | NodeKind::Method | NodeKind::Constructor | NodeKind::Component
        )
    };
    let mut by_path_name: HashMap<(String, String), Vec<usize>> = HashMap::new();
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    let mut by_owner_name: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (index, node) in nodes.iter().enumerate().filter(|(_, node)| eligible(node)) {
        by_name.entry(node.name.clone()).or_default().push(index);
        if let Some(path) = &node.path {
            by_path_name
                .entry((path.clone(), node.name.clone()))
                .or_default()
                .push(index);
        }
        if let Some(owner) = &node.owner {
            by_owner_name
                .entry((owner.clone(), node.name.clone()))
                .or_default()
                .push(index);
        }
    }
    let mut source_cache = HashMap::<String, String>::new();
    unresolved.retain(|call| {
        let same_file = by_path_name.get(&(call.path.clone(), call.target_name.clone()));
        let target_index = if same_file.is_some_and(|items| items.len() == 1) {
            same_file.and_then(|items| items.first()).copied()
        } else {
            let source = source_cache
                .entry(call.path.clone())
                .or_insert_with(|| fs::read_to_string(root.join(&call.path)).unwrap_or_default());
            let inferred = infer_receiver(source, call.line, call.qualifier.as_deref())
                .and_then(|owner| by_owner_name.get(&(owner, call.target_name.clone())))
                .filter(|items| items.len() == 1)
                .and_then(|items| items.first())
                .copied();
            inferred.or_else(|| {
                by_name
                    .get(&call.target_name)
                    .filter(|items| items.len() == 1)
                    .and_then(|items| items.first())
                    .copied()
            })
        };
        if let Some(target) = target_index.and_then(|index| nodes.get(index)) {
            edges.push(CodeEdge::new(
                call.caller_id.clone(),
                target.id.clone(),
                RelationKind::Calls,
            ));
            false
        } else {
            true
        }
    });
}
fn infer_receiver(source: &str, line: usize, qualifier: Option<&str>) -> Option<String> {
    let qualifier = qualifier?;
    if qualifier == "self" {
        return None;
    }
    let mut inferred = None;
    for source_line in source.lines().take(line) {
        let Some(binding) = source_line.trim().strip_prefix("let ") else {
            continue;
        };
        let binding = binding.strip_prefix("mut ").unwrap_or(binding).trim_start();
        let Some(remainder) = binding.strip_prefix(qualifier) else {
            continue;
        };
        if remainder
            .chars()
            .next()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
        {
            continue;
        }
        let remainder = remainder.trim_start();
        let candidate = if let Some(value) = remainder.strip_prefix(':') {
            value
                .trim_start()
                .split(|character: char| !character.is_alphanumeric() && character != '_')
                .next()
        } else if let Some(value) = remainder.strip_prefix('=') {
            value.trim_start().split_once("::").map(|(owner, _)| owner)
        } else {
            None
        };
        if let Some(owner) = candidate.filter(|owner| !owner.is_empty()) {
            inferred = Some(owner.to_string());
        }
    }
    inferred
}

fn resolve_import_edges(
    root: &str,
    file_nodes: &[CodeNode],
    imports: &[crate::model::import::ImportReference],
) -> Vec<CodeEdge> {
    let known: HashMap<_, _> = file_nodes
        .iter()
        .filter_map(|node| Some((node.path.as_deref()?, node.id.as_str())))
        .collect();
    let mut edges = Vec::new();
    for import in imports {
        let Some(source) = known.get(import.source_path.as_str()) else {
            continue;
        };
        if let Some(path) = resolve_import_path(
            Path::new(root),
            &import.source_path,
            &import.full_path,
            &known,
        ) && let Some(target) = known.get(path.as_str())
        {
            edges.push(CodeEdge::new(
                (*source).to_string(),
                (*target).to_string(),
                RelationKind::Imports,
            ));
        }
    }
    edges
}
fn resolve_import_path(
    root: &Path,
    source: &str,
    import: &str,
    known: &HashMap<&str, &str>,
) -> Option<String> {
    let base = Path::new(source).parent().unwrap_or(Path::new(""));
    let raw = if import.starts_with('.') {
        base.join(import)
    } else if import.starts_with("package:") {
        PathBuf::from("lib").join(
            import
                .split_once('/')
                .map(|(_, value)| value)
                .unwrap_or(import),
        )
    } else {
        return None;
    };
    let extensions = [
        "", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".py", ".php", ".dart", ".go",
    ];
    for extension in extensions {
        let candidate = PathBuf::from(format!("{}{}", raw.to_string_lossy(), extension));
        let normalized = normalize(&candidate);
        if known.contains_key(normalized.as_str()) && root.join(&normalized).is_file() {
            return Some(normalized);
        }
    }
    for name in [
        "index.ts",
        "index.tsx",
        "index.js",
        "index.jsx",
        "__init__.py",
    ] {
        let normalized = normalize(&raw.join(name));
        if known.contains_key(normalized.as_str()) {
            return Some(normalized);
        }
    }
    None
}
fn normalize(path: &Path) -> String {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(value) => parts.push(value.to_string_lossy().to_string()),
            _ => {}
        }
    }
    parts.join("/")
}
