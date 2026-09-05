//! Local manifest/lockfile inventory. No registry lookups or package execution.
use crate::{graph::project_graph::ProjectGraph, retrieval::hash};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub name: String,
    pub version: String,
    pub ecosystem: String,
    pub manifest: String,
    pub scope: String,
    pub direct: bool,
    pub first_party: bool,
    pub files: Vec<String>,
    pub provenance: String,
    pub security: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyInventory {
    pub items: Vec<Dependency>,
    pub warnings: Vec<String>,
}
fn entry(
    name: &str,
    version: &str,
    ecosystem: &str,
    manifest: &str,
    scope: &str,
    direct: bool,
    first_party: bool,
) -> Dependency {
    Dependency {
        id: hash(&format!("{manifest}:{name}:{version}:{scope}")),
        name: name.into(),
        version: version.into(),
        ecosystem: ecosystem.into(),
        manifest: manifest.into(),
        scope: scope.into(),
        direct,
        first_party,
        files: vec![],
        provenance: if direct { "manifest" } else { "lockfile" }.into(),
        security: "Non évaluée : aucune base de vulnérabilités consultée".into(),
    }
}
pub fn inventory(graph: &ProjectGraph) -> DependencyInventory {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    let root = Path::new(&graph.root);
    let Ok(canonical_root) = root.canonicalize() else {
        return DependencyInventory {
            items,
            warnings: vec!["Racine du projet inaccessible".into()],
        };
    };
    let mut dirs = BTreeSet::from([PathBuf::new()]);
    for node in graph
        .nodes
        .iter()
        .filter(|n| n.source_scope.library_eligible())
    {
        if let Some(path) = &node.path {
            let mut parent = Path::new(path).parent();
            while let Some(p) = parent {
                if p.as_os_str().is_empty() {
                    break;
                }
                dirs.insert(p.to_owned());
                parent = p.parent();
            }
        }
    }
    for dir in dirs {
        for file in [
            "package.json",
            "package-lock.json",
            "composer.json",
            "composer.lock",
            "Cargo.toml",
            "Cargo.lock",
            "pubspec.yaml",
            "go.mod",
            "requirements.txt",
            "pyproject.toml",
        ] {
            let relative = dir.join(file);
            let path = relative.to_string_lossy().replace('\\', "/");
            let absolute = root.join(&relative);
            let Ok(canonical) = absolute.canonicalize() else {
                continue;
            };
            if !canonical.starts_with(&canonical_root) {
                warnings.push(format!("{path} : lien hors projet ignoré"));
                continue;
            }
            let text = match std::fs::read_to_string(&canonical) {
                Ok(v) => v,
                Err(e) => {
                    warnings.push(format!("{path} : {e}"));
                    continue;
                }
            };
            if text.len() > 5_000_000 {
                warnings.push(format!("{path} dépasse 5 Mo"));
                continue;
            }
            match file {
                "package.json" | "composer.json" => match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        let scopes = if file == "package.json" {
                            vec![
                                "dependencies",
                                "devDependencies",
                                "peerDependencies",
                                "optionalDependencies",
                            ]
                        } else {
                            vec!["require", "require-dev"]
                        };
                        for scope in scopes {
                            if let Some(deps) = value.get(scope).and_then(Value::as_object) {
                                for (name, version) in deps {
                                    if let Some(version) = version.as_str() {
                                        items.push(entry(
                                            name,
                                            version,
                                            if file == "package.json" {
                                                "npm"
                                            } else {
                                                "composer"
                                            },
                                            &path,
                                            scope,
                                            true,
                                            version.starts_with("workspace:")
                                                || version.starts_with("file:")
                                                || version.starts_with("link:"),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => warnings.push(format!("{path} : JSON invalide ({e})")),
                },
                "package-lock.json" => match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        if let Some(packages) = value["packages"].as_object() {
                            for (location, package) in packages {
                                if location.is_empty() {
                                    continue;
                                }
                                let name = package["name"]
                                    .as_str()
                                    .or_else(|| location.rsplit("node_modules/").next())
                                    .unwrap_or(location);
                                if let Some(version) = package["version"].as_str() {
                                    items.push(entry(
                                        name,
                                        version,
                                        "npm",
                                        &path,
                                        "locked",
                                        false,
                                        package["link"].as_bool() == Some(true),
                                    ));
                                }
                            }
                        } else {
                            warnings
                                .push(format!("{path} : ancien format de lockfile non interprété"));
                        }
                    }
                    Err(e) => warnings.push(format!("{path} : {e}")),
                },
                "composer.lock" => match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        for scope in ["packages", "packages-dev"] {
                            if let Some(packages) = value[scope].as_array() {
                                for p in packages {
                                    if let (Some(name), Some(version)) =
                                        (p["name"].as_str(), p["version"].as_str())
                                    {
                                        items.push(entry(
                                            name, version, "composer", &path, scope, false, false,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => warnings.push(format!("{path} : {e}")),
                },
                "Cargo.toml" | "pyproject.toml" | "Cargo.lock" => {
                    match toml::from_str::<toml::Value>(&text) {
                        Ok(value) => {
                            if file == "Cargo.lock" {
                                if let Some(packages) =
                                    value.get("package").and_then(toml::Value::as_array)
                                {
                                    for p in packages {
                                        if let (Some(name), Some(version)) = (
                                            p.get("name").and_then(toml::Value::as_str),
                                            p.get("version").and_then(toml::Value::as_str),
                                        ) {
                                            items.push(entry(
                                                name,
                                                version,
                                                "cargo",
                                                &path,
                                                "locked",
                                                false,
                                                p.get("source").is_none(),
                                            ));
                                        }
                                    }
                                }
                            } else if file == "Cargo.toml" {
                                fn cargo_tables(
                                    value: &toml::Value,
                                    path: &str,
                                    prefix: &str,
                                    items: &mut Vec<Dependency>,
                                ) {
                                    if let Some(table) = value.as_table() {
                                        for (key, value) in table {
                                            let scope = format!("{prefix}{key}");
                                            if [
                                                "dependencies",
                                                "dev-dependencies",
                                                "build-dependencies",
                                            ]
                                            .contains(&key.as_str())
                                            {
                                                if let Some(deps) = value.as_table() {
                                                    for (name, spec) in deps {
                                                        let version = spec
                                                            .as_str()
                                                            .or_else(|| {
                                                                spec.get("version")
                                                                    .and_then(toml::Value::as_str)
                                                            })
                                                            .unwrap_or(
                                                                if spec.get("workspace").is_some() {
                                                                    "workspace"
                                                                } else {
                                                                    "local/git"
                                                                },
                                                            );
                                                        items.push(entry(
                                                            name,
                                                            version,
                                                            "cargo",
                                                            path,
                                                            &scope,
                                                            true,
                                                            spec.get("path").is_some(),
                                                        ));
                                                    }
                                                }
                                            } else if key == "target"
                                                || key == "workspace"
                                                || prefix.starts_with("target.")
                                            {
                                                cargo_tables(
                                                    value,
                                                    path,
                                                    &format!("{scope}."),
                                                    items,
                                                );
                                            }
                                        }
                                    }
                                }
                                cargo_tables(&value, &path, "", &mut items);
                            } else {
                                if let Some(deps) = value
                                    .get("project")
                                    .and_then(|v| v.get("dependencies"))
                                    .and_then(toml::Value::as_array)
                                {
                                    for dep in deps.iter().filter_map(toml::Value::as_str) {
                                        python_requirement(dep, &path, "project", &mut items);
                                    }
                                }
                                if let Some(groups) = value
                                    .get("project")
                                    .and_then(|v| v.get("optional-dependencies"))
                                    .and_then(toml::Value::as_table)
                                {
                                    for (group, deps) in groups {
                                        if let Some(deps) = deps.as_array() {
                                            for dep in deps.iter().filter_map(toml::Value::as_str) {
                                                python_requirement(dep, &path, group, &mut items);
                                            }
                                        }
                                    }
                                }
                                if let Some(deps) = value
                                    .get("tool")
                                    .and_then(|v| v.get("poetry"))
                                    .and_then(|v| v.get("dependencies"))
                                    .and_then(toml::Value::as_table)
                                {
                                    for (name, spec) in deps {
                                        items.push(entry(
                                            name,
                                            spec.as_str()
                                                .or_else(|| {
                                                    spec.get("version")
                                                        .and_then(toml::Value::as_str)
                                                })
                                                .unwrap_or("unspecified"),
                                            "python",
                                            &path,
                                            "poetry",
                                            true,
                                            spec.get("path").is_some(),
                                        ));
                                    }
                                }
                            }
                        }
                        Err(e) => warnings.push(format!("{path} : TOML invalide ({e})")),
                    }
                }
                "requirements.txt" => {
                    for line in text.lines() {
                        let line = line.split('#').next().unwrap_or("").trim();
                        if line.is_empty() {
                            continue;
                        }
                        if line.starts_with('-') {
                            warnings.push(format!(
                                "{path} : directive non suivie : {}",
                                line.split_whitespace().next().unwrap_or("")
                            ));
                            continue;
                        }
                        python_requirement(line, &path, "requirements", &mut items);
                    }
                }
                "go.mod" => {
                    let mut block = false;
                    for line in text.lines() {
                        let line = line.trim();
                        if line == "require (" {
                            block = true;
                            continue;
                        }
                        if line == ")" {
                            block = false;
                            continue;
                        }
                        let declaration = if block {
                            Some(line)
                        } else {
                            line.strip_prefix("require ")
                        };
                        if let Some(line) = declaration {
                            let fields = line.split_whitespace().collect::<Vec<_>>();
                            if fields.len() >= 2 {
                                items.push(entry(
                                    fields[0],
                                    fields[1],
                                    "go",
                                    &path,
                                    "require",
                                    !line.contains("// indirect"),
                                    false,
                                ));
                            }
                        }
                        if line.starts_with("replace ") {
                            warnings.push(format!("{path} : vérifier les directives replace pour les résolutions locales"));
                        }
                    }
                }
                "pubspec.yaml" => {
                    let mut scope = "";
                    for line in text.lines() {
                        if !line.starts_with(' ') {
                            scope = match line.trim() {
                                "dependencies:" => "dependencies",
                                "dev_dependencies:" => "dev_dependencies",
                                _ => "",
                            };
                            continue;
                        }
                        if scope.is_empty() || !line.starts_with("  ") || line.starts_with("   ") {
                            continue;
                        }
                        if let Some((name, value)) = line.trim().split_once(':') {
                            let name = name.trim();
                            if name.is_empty() || name.starts_with('#') {
                                continue;
                            }
                            let version = value
                                .split('#')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .trim_matches(['\'', '"']);
                            items.push(entry(
                                name,
                                if version.is_empty() {
                                    "structured declaration (inspect manifest)"
                                } else {
                                    version
                                },
                                "pub",
                                &path,
                                scope,
                                true,
                                false,
                            ));
                        }
                    }
                    warnings.push(format!("{path} : les déclarations YAML complexes (path/git/sdk/anchors) nécessitent une inspection du manifeste"));
                }
                _ => {}
            }
        }
    }
    // Mark lockfile entries corresponding to direct declarations without pretending
    // that a requested version range is an installed/resolved version.
    let direct = items
        .iter()
        .filter(|d| d.direct)
        .map(|d| {
            (
                d.ecosystem.clone(),
                Path::new(&d.manifest)
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_owned(),
                d.name.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    for dep in &mut items {
        if dep.scope == "locked" || dep.scope.starts_with("packages") {
            dep.direct = direct.contains(&(
                dep.ecosystem.clone(),
                Path::new(&dep.manifest)
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_owned(),
                dep.name.clone(),
            ));
        }
        let names = [
            dep.name.clone(),
            dep.name.replace('-', "_"),
            dep.name.rsplit('/').next().unwrap_or(&dep.name).into(),
        ];
        dep.files = graph
            .imports
            .iter()
            .filter(|import| {
                let serialized = import.full_path.to_lowercase();
                names
                    .iter()
                    .any(|name| serialized.contains(&name.to_lowercase()))
            })
            .map(|import| import.source_path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if !dep.files.is_empty() {
            dep.provenance
                .push_str(" + lexical import matches (heuristic)");
        }
    }
    let mut unique = BTreeMap::new();
    for dep in items {
        unique.insert(dep.id.clone(), dep);
    }
    let mut items = unique.into_values().collect::<Vec<_>>();
    items.sort_by(|a, b| {
        a.ecosystem
            .cmp(&b.ecosystem)
            .then(a.name.cmp(&b.name))
            .then(a.manifest.cmp(&b.manifest))
    });
    DependencyInventory { items, warnings }
}
fn python_requirement(text: &str, path: &str, scope: &str, items: &mut Vec<Dependency>) {
    let requirement = text.split(';').next().unwrap_or("").trim();
    let split = requirement
        .find(['<', '>', '=', '!', '~', '[', ' ', '@'])
        .unwrap_or(requirement.len());
    let name = &requirement[..split];
    if name.is_empty() {
        return;
    }
    let version = requirement[split..].trim();
    items.push(entry(
        name,
        if version.is_empty() {
            "unspecified"
        } else {
            version
        },
        "python",
        path,
        scope,
        true,
        false,
    ));
}
