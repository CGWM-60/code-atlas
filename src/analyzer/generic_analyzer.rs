#![allow(clippy::collapsible_if)]

//! Deterministic Tree-sitter analyzers shared by TypeScript, JavaScript, PHP,
//! Python and Go. Dart currently uses a conservative line parser because the
//! maintained grammar ecosystem does not share the project's Tree-sitter ABI.

use anyhow::{Context, Result};
use regex::Regex;
use std::{fs, path::Path};
use tree_sitter::{Language, Node, Parser, Tree};

use crate::{
    language::ProgrammingLanguage,
    model::{
        analysis::FileAnalysis,
        call::{CallKind, CallReference},
        import::ImportReference,
        node::{CodeNode, NodeKind},
        project::ProjectFile,
    },
};

pub fn analyze_tree_sitter_file(project_root: &Path, file: &ProjectFile) -> Result<FileAnalysis> {
    let absolute = project_root.join(&file.path);
    let source = fs::read_to_string(&absolute)
        .with_context(|| format!("cannot read {}", absolute.display()))?;
    if file.language == ProgrammingLanguage::Dart {
        return analyze_dart(&source, file);
    }
    let language = language_for(file.language).context("no Tree-sitter grammar for language")?;
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .context("cannot configure Tree-sitter grammar")?;
    let tree = parser
        .parse(&source, None)
        .context("Tree-sitter returned no syntax tree")?;
    Ok(analyze_tree(&tree, &source, file))
}

fn language_for(language: ProgrammingLanguage) -> Option<Language> {
    match language {
        ProgrammingLanguage::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        ProgrammingLanguage::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        ProgrammingLanguage::Php => Some(tree_sitter_php::LANGUAGE_PHP.into()),
        ProgrammingLanguage::Python => Some(tree_sitter_python::LANGUAGE.into()),
        ProgrammingLanguage::Go => Some(tree_sitter_go::LANGUAGE.into()),
        _ => None,
    }
}

fn analyze_tree(tree: &Tree, source: &str, file: &ProjectFile) -> FileAnalysis {
    let mut analysis = FileAnalysis::empty();
    collect_nodes(tree.root_node(), source, file, &mut analysis.nodes);
    collect_calls(
        tree.root_node(),
        source,
        file,
        &analysis.nodes,
        &mut analysis.calls,
    );
    analysis.imports = collect_imports(source, file);
    analysis
}

fn collect_nodes(node: Node<'_>, source: &str, file: &ProjectFile, output: &mut Vec<CodeNode>) {
    if let Some((kind, name_node)) = classify(node, file.language) {
        if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
            let name = name
                .trim_matches(|ch: char| ch == '\'' || ch == '"')
                .to_string();
            if !name.is_empty() {
                let start = node.start_position().row + 1;
                let owner = owner_name(node, source);
                let mut final_kind =
                    if name == "constructor" || name == "__construct" || name == "__init__" {
                        NodeKind::Constructor
                    } else {
                        kind
                    };
                if matches!(
                    file.language,
                    ProgrammingLanguage::TypeScript | ProgrammingLanguage::JavaScript
                ) && kind == NodeKind::Function
                    && name.chars().next().is_some_and(char::is_uppercase)
                    && matches!(
                        Path::new(&file.path).extension().and_then(|v| v.to_str()),
                        Some("tsx" | "jsx")
                    )
                {
                    final_kind = NodeKind::Component;
                }
                output.push(CodeNode {
                    id: format!(
                        "{}:{}:{}:{}",
                        final_kind.as_str().to_lowercase(),
                        file.path,
                        start,
                        name
                    ),
                    kind: final_kind,
                    name,
                    path: Some(file.path.clone()),
                    language: Some(file.language),
                    start_line: Some(start),
                    end_line: Some(node.end_position().row + 1),
                    owner,
                    source_scope: file.source_scope,
                });
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes(child, source, file, output);
    }
}

fn classify<'a>(node: Node<'a>, language: ProgrammingLanguage) -> Option<(NodeKind, Node<'a>)> {
    let name = || {
        node.child_by_field_name("name")
            .or_else(|| node.named_child(0))
    };
    let kind = match (language, node.kind()) {
        (
            ProgrammingLanguage::TypeScript | ProgrammingLanguage::JavaScript,
            "function_declaration" | "generator_function_declaration",
        ) => NodeKind::Function,
        (
            ProgrammingLanguage::TypeScript | ProgrammingLanguage::JavaScript,
            "class_declaration",
        ) => NodeKind::Class,
        (
            ProgrammingLanguage::TypeScript | ProgrammingLanguage::JavaScript,
            "method_definition",
        ) => NodeKind::Method,
        (ProgrammingLanguage::TypeScript, "interface_declaration") => NodeKind::Interface,
        (ProgrammingLanguage::TypeScript, "type_alias_declaration") => NodeKind::TypeAlias,
        (ProgrammingLanguage::Python, "function_definition") => {
            if has_ancestor(node, "class_definition") {
                NodeKind::Method
            } else {
                NodeKind::Function
            }
        }
        (ProgrammingLanguage::Python, "class_definition") => NodeKind::Class,
        (ProgrammingLanguage::Php, "function_definition") => NodeKind::Function,
        (ProgrammingLanguage::Php, "method_declaration") => NodeKind::Method,
        (ProgrammingLanguage::Php, "class_declaration") => NodeKind::Class,
        (ProgrammingLanguage::Php, "interface_declaration") => NodeKind::Interface,
        (ProgrammingLanguage::Php, "trait_declaration") => NodeKind::Trait,
        (ProgrammingLanguage::Go, "function_declaration") => NodeKind::Function,
        (ProgrammingLanguage::Go, "method_declaration") => NodeKind::Method,
        (ProgrammingLanguage::Go, "type_spec") => {
            let type_node = node.child_by_field_name("type")?;
            match type_node.kind() {
                "struct_type" => NodeKind::Struct,
                "interface_type" => NodeKind::Interface,
                _ => NodeKind::TypeAlias,
            }
        }
        _ => return None,
    };
    name().map(|value| (kind, value))
}

fn has_ancestor(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }
    false
}
fn owner_name(mut node: Node<'_>, source: &str) -> Option<String> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "class_declaration" | "trait_declaration" | "interface_declaration"
        ) {
            return parent
                .child_by_field_name("name")?
                .utf8_text(source.as_bytes())
                .ok()
                .map(str::to_string);
        }
        node = parent;
    }
    None
}

fn collect_calls(
    node: Node<'_>,
    source: &str,
    file: &ProjectFile,
    nodes: &[CodeNode],
    calls: &mut Vec<CallReference>,
) {
    if matches!(
        node.kind(),
        "call_expression" | "function_call_expression" | "member_call_expression"
    ) {
        let callable = node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("name"))
            .or_else(|| node.named_child(0));
        if let Some(callable) = callable {
            if let Ok(raw) = callable.utf8_text(source.as_bytes()) {
                let raw = raw.trim();
                let normalized = raw.replace("->", ".");
                let target_name = normalized
                    .rsplit(['.', ':'])
                    .find(|part| !part.is_empty())
                    .unwrap_or(raw)
                    .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .to_string();
                let line = node.start_position().row + 1;
                if !target_name.is_empty() {
                    if let Some(caller) = nodes
                        .iter()
                        .filter(|item| {
                            matches!(
                                item.kind,
                                NodeKind::Function
                                    | NodeKind::Method
                                    | NodeKind::Constructor
                                    | NodeKind::Component
                            )
                        })
                        .filter(|item| {
                            item.start_line.unwrap_or(0) <= line
                                && item.end_line.unwrap_or(0) >= line
                        })
                        .min_by_key(|item| {
                            item.end_line.unwrap_or(usize::MAX) - item.start_line.unwrap_or(0)
                        })
                    {
                        let qualifier = normalized
                            .rsplit_once('.')
                            .map(|(left, _)| left.to_string())
                            .or_else(|| {
                                normalized
                                    .rsplit_once("::")
                                    .map(|(left, _)| left.to_string())
                            });
                        let kind = if normalized.contains('.') {
                            CallKind::Method
                        } else if normalized.contains("::") {
                            CallKind::Scoped
                        } else {
                            CallKind::Function
                        };
                        calls.push(CallReference {
                            caller_id: caller.id.clone(),
                            path: file.path.clone(),
                            line,
                            raw_target: raw.to_string(),
                            target_name,
                            qualifier,
                            kind,
                        });
                    }
                }
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, source, file, nodes, calls);
    }
}

fn collect_imports(source: &str, file: &ProjectFile) -> Vec<ImportReference> {
    let patterns: &[&str] = match file.language {
        ProgrammingLanguage::TypeScript | ProgrammingLanguage::JavaScript => &[
            r#"(?m)^\s*import\s+.*?\s+from\s+['\"]([^'\"]+)['\"]"#,
            r#"(?m)^\s*import\s*['\"]([^'\"]+)['\"]"#,
        ],
        ProgrammingLanguage::Python => &[
            r"(?m)^\s*from\s+([\w.]+)\s+import\s+([\w*]+)",
            r"(?m)^\s*import\s+([\w.]+)",
        ],
        ProgrammingLanguage::Php => &[r"(?m)^\s*use\s+([^;]+);"],
        ProgrammingLanguage::Go => &[r#"(?m)^\s*import\s+(?:\w+\s+)?\"([^\"]+)\""#],
        _ => &[],
    };
    let mut imports = Vec::new();
    for pattern in patterns {
        if let Ok(regex) = Regex::new(pattern) {
            for captures in regex.captures_iter(source) {
                let Some(full) = captures.get(1) else {
                    continue;
                };
                let full_path = full.as_str().trim().to_string();
                let imported_name = captures
                    .get(2)
                    .map(|v| v.as_str())
                    .unwrap_or_else(|| {
                        full_path
                            .rsplit(['/', '.', '\\'])
                            .next()
                            .unwrap_or(&full_path)
                    })
                    .to_string();
                let line = source[..full.start()]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    + 1;
                imports.push(ImportReference {
                    source_path: file.path.clone(),
                    line,
                    full_path: full_path.clone(),
                    imported_name,
                    alias: None,
                    is_wildcard: full_path.ends_with('*'),
                    raw_use: captures.get(0).map_or("", |v| v.as_str()).to_string(),
                });
            }
        }
    }
    imports
}

fn analyze_dart(source: &str, file: &ProjectFile) -> Result<FileAnalysis> {
    let mut analysis = FileAnalysis::empty();
    let declarations = Regex::new(
        r"(?m)^\s*(?:abstract\s+)?(class|mixin|enum)\s+(\w+)|^\s*(?:[\w<>?]+\s+)+(\w+)\s*\([^;]*\)\s*(?:async\s*)?\{",
    )?;
    for captures in declarations.captures_iter(source) {
        let whole = captures.get(0).expect("regex has full capture");
        let line = source[..whole.start()]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1;
        let (name, mut kind) = if let Some(name) = captures.get(2) {
            (
                name.as_str(),
                if captures.get(1).is_some_and(|v| v.as_str() == "enum") {
                    NodeKind::Enum
                } else {
                    NodeKind::Class
                },
            )
        } else {
            (
                captures.get(3).map_or("unknown", |v| v.as_str()),
                NodeKind::Function,
            )
        };
        if whole.as_str().contains("Widget")
            || source
                .lines()
                .nth(line.saturating_sub(1))
                .is_some_and(|v| v.contains("Widget"))
        {
            kind = NodeKind::Component;
        }
        analysis.nodes.push(CodeNode {
            id: format!(
                "{}:{}:{}:{}",
                kind.as_str().to_lowercase(),
                file.path,
                line,
                name
            ),
            kind,
            name: name.to_string(),
            path: Some(file.path.clone()),
            language: Some(file.language),
            start_line: Some(line),
            end_line: Some(line),
            owner: None,
            source_scope: file.source_scope,
        });
    }
    analysis.imports = collect_dart_imports(source, file)?;
    Ok(analysis)
}
fn collect_dart_imports(source: &str, file: &ProjectFile) -> Result<Vec<ImportReference>> {
    let regex = Regex::new(r#"(?m)^\s*import\s+['\"]([^'\"]+)['\"]"#)?;
    Ok(regex
        .captures_iter(source)
        .filter_map(|c| {
            let full = c.get(1)?;
            Some(ImportReference {
                source_path: file.path.clone(),
                line: source[..full.start()]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    + 1,
                full_path: full.as_str().to_string(),
                imported_name: full
                    .as_str()
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(".dart")
                    .to_string(),
                alias: None,
                is_wildcard: false,
                raw_use: c.get(0)?.as_str().to_string(),
            })
        })
        .collect())
}
