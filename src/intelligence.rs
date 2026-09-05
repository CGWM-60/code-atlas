//! Read-only change intelligence. Estimates and test links are explicitly heuristic.
use crate::{
    features::Feature,
    graph::{impact::analyze_impact, project_graph::ProjectGraph},
    model::node::CodeNode,
    retrieval::{HybridRetriever, hash, source},
    storage::Repository,
};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub kind: String,
    pub status: String,
    pub evidence: Vec<String>,
    pub priority: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlan {
    pub feature: Option<String>,
    pub existing_tests: Vec<CodeNode>,
    pub scenarios: Vec<Scenario>,
    pub risks: Vec<String>,
    pub generated_files: Vec<GeneratedFile>,
    pub coverage: Option<f32>,
    pub provenance: String,
}
pub fn test_plan(
    graph: &ProjectGraph,
    features: &[Feature],
    feature_id: Option<&str>,
) -> Result<TestPlan> {
    let feature = feature_id
        .map(|id| {
            features
                .iter()
                .find(|f| f.id == id)
                .ok_or_else(|| anyhow!("feature not found"))
        })
        .transpose()?;
    let ids = feature
        .map(|f| f.node_ids.clone())
        .unwrap_or_else(|| graph.nodes.iter().map(|n| n.id.clone()).collect());
    let existing = graph
        .nodes
        .iter()
        .filter(|n| n.source_scope.library_eligible())
        .filter(|n| {
            let path = n.path.as_deref().unwrap_or("").to_lowercase();
            let is_test = path.contains("test")
                || path.contains("spec.")
                || n.name.starts_with("test_")
                || source(graph, n).is_some_and(|(s, _, _)| s.starts_with("#[test]"));
            is_test
                && (feature.is_none()
                    || ids.contains(&n.id)
                    || graph
                        .outgoing_edges(&n.id)
                        .iter()
                        .any(|e| ids.contains(&e.target_id)))
        })
        .cloned()
        .collect::<Vec<_>>();
    let entry_ids = feature
        .map(|f| f.entry_point_node_ids.clone())
        .unwrap_or_else(|| {
            graph
                .nodes
                .iter()
                .filter(|n| {
                    matches!(
                        n.kind,
                        crate::model::node::NodeKind::Route
                            | crate::model::node::NodeKind::ApiEndpoint
                            | crate::model::node::NodeKind::Page
                    )
                })
                .take(40)
                .map(|n| n.id.clone())
                .collect()
        });
    let mut scenarios = Vec::new();
    let mut generated_files = Vec::new();
    for id in &entry_ids {
        let Some(node) = graph.find_node(id) else {
            continue;
        };
        let linked = existing
            .iter()
            .filter(|t| {
                graph
                    .outgoing_edges(&t.id)
                    .iter()
                    .any(|e| e.target_id == *id)
            })
            .map(|n| n.id.clone())
            .collect::<Vec<_>>();
        for (name, kind) in [
            ("Nominal", "integration"),
            ("Entrée invalide", "integration"),
            ("Erreur de dépendance", "unit"),
        ] {
            scenarios.push(Scenario {
                name: format!("{} · {}", node.name, name),
                kind: kind.into(),
                status: if linked.is_empty() {
                    "missing_evidence"
                } else {
                    "test_linked_unverified"
                }
                .into(),
                evidence: linked.clone(),
                priority: "high".into(),
            });
        }
        // A navigation smoke test requires an observed route and GET/page semantics;
        // no fabricated selectors, credentials or application assertions.
        let route = node.name.split_whitespace().find(|p| p.starts_with('/'));
        if let Some(route) =
            route.filter(|r| !r.contains(['{', ':', '[', '*']) && !r.starts_with("//"))
            && matches!(
                node.kind,
                crate::model::node::NodeKind::Page | crate::model::node::NodeKind::Route
            )
            && !node.name.contains("POST")
        {
            let quoted = serde_json::to_string(route)?;
            let title = serde_json::to_string(&format!("Route observée : {route}"))?;
            generated_files.push(GeneratedFile{path:format!("tests/atlas-{}.spec.ts",&hash(id)[..12]),content:format!("import {{ test, expect }} from '@playwright/test';\n\n// Smoke test uniquement. Configurer baseURL et l’état d’authentification si nécessaire.\n// Preuve : {}:{}\ntest({title}, async ({{ page }}) => {{\n  const response = await page.goto({quoted});\n  expect(response, 'La navigation doit produire une réponse HTTP').not.toBeNull();\n  expect(response!.status()).toBeLessThan(400);\n}});\n",node.path.as_deref().unwrap_or(""),node.start_line.unwrap_or(1))});
        }
    }
    Ok(TestPlan{feature:feature.map(|f|f.id.clone()),existing_tests:existing,scenarios,risks:vec!["Une relation vers un test ne prouve ni son exécution ni ses assertions. Couverture fonctionnelle inconnue sans résultats de tests.".into(),"Les fichiers générés sont des smoke tests de navigation à relire ; ils ne valident pas les règles métier.".into()],generated_files,coverage:None,provenance:"ProjectGraph, chemins de tests et relations déterministes ; scénarios proposés par heuristique.".into()})
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub category: String,
    pub minimum: f32,
    pub likely: f32,
    pub maximum: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEstimate {
    pub summary: String,
    pub affected_features: Vec<String>,
    pub affected_nodes: Vec<CodeNode>,
    pub affected_files: Vec<String>,
    pub breakdown: Vec<TimeRange>,
    pub unknowns: Vec<String>,
    pub risk_level: String,
    pub confidence: f32,
    pub provenance: String,
}
fn estimate_uncached(
    repo: &Repository,
    project: &str,
    graph: &ProjectGraph,
    features: &[Feature],
    task: &str,
) -> Result<ChangeEstimate> {
    if task.trim().is_empty() || task.chars().count() > 8000 {
        bail!("Décrivez une mission entre 1 et 8000 caractères");
    }
    let hits = HybridRetriever::search(repo, project, graph, features, task, None, 12)?;
    let mut ids = hits
        .iter()
        .map(|c| c.node.id.clone())
        .collect::<BTreeSet<_>>();
    for hit in &hits {
        if let Some(impact) = analyze_impact(graph, &hit.node.id, 2) {
            for node in impact
                .direct
                .iter()
                .chain(impact.transitive.iter().flat_map(|level| &level.nodes))
            {
                ids.insert(node.id.clone());
            }
        }
    }
    let nodes = ids
        .iter()
        .filter_map(|id| graph.find_node(id))
        .take(100)
        .cloned()
        .collect::<Vec<_>>();
    let files = nodes
        .iter()
        .filter_map(|n| n.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let scale = 1.0 + (files.len() as f32).sqrt();
    let breakdown = if hits.is_empty() {
        vec![]
    } else {
        [
            ("analysis", 1.0),
            ("backend", 2.0),
            ("frontend", 1.5),
            ("database", 0.5),
            ("tests", 1.5),
            ("documentation", 0.5),
            ("review", 0.5),
            ("qa", 1.0),
        ]
        .into_iter()
        .map(|(category, weight)| TimeRange {
            category: category.into(),
            minimum: weight,
            likely: weight * scale,
            maximum: weight * scale * 2.5,
        })
        .collect()
    };
    Ok(ChangeEstimate{summary:task.into(),affected_features:features.iter().filter(|f|f.node_ids.iter().any(|id|ids.contains(id))).map(|f|f.id.clone()).collect(),affected_nodes:nodes,affected_files:files,breakdown,unknowns:vec!["Chiffrage heuristique en heures, non calibré sur la vélocité de votre équipe.".into(),"Valider le périmètre, les critères d’acceptation, les migrations et les intégrations externes avant engagement.".into(),"La recherche fournit des candidats ; les fichiers listés ne sont pas tous nécessairement à modifier.".into()],risk_level:if ids.len()>30{"high"}else{"unknown"}.into(),confidence:if hits.is_empty(){0.0}else{0.3},provenance:"HybridRetriever + impact entrant (profondeur 2). Coefficients de temps explicites et heuristiques.".into()})
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: String,
    pub text: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFile {
    pub path: String,
    pub lines: Vec<DiffLine>,
    pub nodes: Vec<String>,
    pub features: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffAnalysis {
    pub branch: String,
    pub branches: Vec<String>,
    pub base: Option<String>,
    pub head: Option<String>,
    pub files: Vec<DiffFile>,
    pub warnings: Vec<String>,
}
fn git(root: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .context("Git indisponible")?;
    if !output.status.success() {
        bail!("Git : {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
fn revision(root: &str, value: &str) -> Result<String> {
    if value.is_empty() || value.starts_with('-') || value.len() > 250 {
        bail!("Révision Git invalide");
    }
    git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{value}^{{commit}}"),
        ],
    )
    .map(|s| s.trim().into())
}
pub fn git_diff(
    graph: &ProjectGraph,
    features: &[Feature],
    base: Option<&str>,
    head: Option<&str>,
) -> Result<DiffAnalysis> {
    if head.is_some() && base.is_none() {
        bail!("Une révision de base est nécessaire");
    }
    let branch = git(&graph.root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branches = git(&graph.root, &["branch", "--format=%(refname:short)"])?;
    let mut args = vec![
        "diff".to_owned(),
        "--no-ext-diff".into(),
        "--no-textconv".into(),
        "--no-color".into(),
        "--unified=3".into(),
    ];
    if let Some(base) = base {
        args.push(revision(&graph.root, base)?);
    } else {
        args.push("HEAD".into());
    }
    if let Some(head) = head {
        args.push(revision(&graph.root, head)?);
    }
    args.push("--".into());
    let raw = git(
        &graph.root,
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    let mut files = Vec::<DiffFile>::new();
    let (mut old, mut new) = (0, 0);
    let mut truncated = false;
    for (i, line) in raw.lines().enumerate() {
        if i >= 20000 {
            truncated = true;
            break;
        }
        if let Some(path) = line.strip_prefix("+++ b/") {
            files.push(DiffFile {
                path: path.into(),
                lines: vec![],
                nodes: vec![],
                features: vec![],
            });
            continue;
        }
        if let Some(path) = line.strip_prefix("--- a/") {
            if raw.contains(&format!("--- a/{path}\n+++ /dev/null")) {
                files.push(DiffFile {
                    path: path.into(),
                    lines: vec![],
                    nodes: vec![],
                    features: vec![],
                });
            }
            continue;
        }
        if line.starts_with("diff --git") || line.starts_with("index ") || line.starts_with("+++") {
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if line.starts_with("@@") {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            old = parts
                .get(1)
                .and_then(|s| s.trim_start_matches('-').split(',').next()?.parse().ok())
                .unwrap_or(0);
            new = parts
                .get(2)
                .and_then(|s| s.trim_start_matches('+').split(',').next()?.parse().ok())
                .unwrap_or(0);
            file.lines.push(DiffLine {
                kind: "hunk".into(),
                text: line.into(),
                old_line: None,
                new_line: None,
            });
            continue;
        }
        let (kind, old_line, new_line) = if line.starts_with('+') {
            let n = new;
            new += 1;
            ("added", None, Some(n))
        } else if line.starts_with('-') {
            let n = old;
            old += 1;
            ("removed", Some(n), None)
        } else if line.starts_with(' ') {
            let (o, n) = (old, new);
            old += 1;
            new += 1;
            ("context", Some(o), Some(n))
        } else {
            continue;
        };
        file.lines.push(DiffLine {
            kind: kind.into(),
            text: crate::ai::context_builder::redact_sensitive_line(line),
            old_line,
            new_line,
        });
    }
    files.retain(|f| !crate::ai::context_builder::sensitive_path(&f.path));
    for file in &mut files {
        file.nodes = graph
            .nodes
            .iter()
            .filter(|n| n.path.as_deref() == Some(&file.path))
            .filter(|n| {
                file.lines
                    .iter()
                    .filter(|l| l.kind == "added")
                    .filter_map(|l| l.new_line)
                    .any(|line| {
                        line >= n.start_line.unwrap_or(1)
                            && line <= n.end_line.unwrap_or(usize::MAX)
                    })
            })
            .map(|n| n.id.clone())
            .collect();
        file.features = features
            .iter()
            .filter(|f| f.node_ids.iter().any(|id| file.nodes.contains(id)))
            .map(|f| f.id.clone())
            .collect();
    }
    let mut warnings=vec!["Relations évaluées sur le dernier graphe analysé. Les suppressions et les anciennes révisions nécessitent un graphe historique pour une correspondance fiable.".into(),"Les fichiers non suivis et les fichiers binaires ne sont pas inclus. Les findings introduits/résolus ne sont pas déduits sans analyses des deux révisions.".into()];
    if truncated {
        warnings.push("Diff tronqué à 20 000 lignes.".into());
    }
    Ok(DiffAnalysis {
        branch: branch.trim().into(),
        branches: branches.lines().map(str::to_owned).collect(),
        base: base.map(str::to_owned),
        head: head.map(str::to_owned),
        files,
        warnings,
    })
}

fn cached<T: Serialize + serde::de::DeserializeOwned>(repo: &Repository, project: &str, kind: &str, content_hash: &str, compute: impl FnOnce() -> Result<T>) -> Result<T> {
    let key = crate::context_engine::ai_cache_key(content_hash, kind, "intelligence-v1", "deterministic", "local");
    if let Some(value) = repo.get_ai_cache(&key)? { return Ok(serde_json::from_value(value)?); }
    let value = compute()?;
    repo.save_ai_cache(&crate::context_engine::AiCacheWrite { cache_key: &key, project_id: project, analysis_type: kind, content_hash, prompt_version: "intelligence-v1", provider: "deterministic", model: "local", result: &serde_json::to_value(&value)?, input_tokens: None, output_tokens: None })?;
    Ok(value)
}
pub fn cached_test_plan(repo: &Repository, project: &str, graph: &ProjectGraph, features: &[Feature], feature_id: Option<&str>) -> Result<TestPlan> {
    let key = hash(&format!("{}:{}:{}",project,crate::documentation::project_knowledge_hash(graph,features,&[]),feature_id.unwrap_or("")));
    cached(repo,project,"test_plan",&key,||test_plan(graph,features,feature_id))
}
pub fn estimate(repo: &Repository, project: &str, graph: &ProjectGraph, features: &[Feature], task: &str) -> Result<ChangeEstimate> {
    let key = hash(&format!("{}:{}:{}",project,crate::documentation::project_knowledge_hash(graph,features,&[]),task));
    cached(repo,project,"change_estimate",&key,||estimate_uncached(repo,project,graph,features,task))
}
