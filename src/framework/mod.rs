use crate::model::{
    edge::{CodeEdge, RelationKind},
    node::{CodeNode, NodeKind},
    project::ProjectFile,
};
use anyhow::Result;
use regex::Regex;
use std::{fs, path::Path, sync::LazyLock};

mod symfony;

static HTTP_ROUTE_PATTERNS: LazyLock<[Regex; 3]> = LazyLock::new(|| {
    [
        Regex::new(r#"\.route\(\s*\"([^\"]+)\"\s*,\s*(get|post|put|patch|delete)\((\w+)\)"#)
            .expect("valid route regex"),
        Regex::new(
            r#"Route::(get|post|put|patch|delete)\(\s*['\"]([^'\"]+)['\"]\s*,\s*\[?([\w:]+)"#,
        )
        .expect("valid route regex"),
        Regex::new(r#"#\[Route\(\s*['\"]([^'\"]+)['\"]"#).expect("valid route regex"),
    ]
});
static FLUTTER_ROUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"GoRoute\s*\([^)]*?path\s*:\s*['\"]([^'\"]+)['\"][^)]*?builder\s*:[^=]*=>\s*(\w+)"#,
    )
    .expect("valid Flutter route regex")
});
static FLUTTER_PROVIDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^\s*(?:(?:late\s+)?final|var)?\s*(\w+Provider)\s*=\s*(?:State|Future|Stream|Notifier)?Provider",
    )
    .expect("valid Flutter provider regex")
});
static INHERITANCE_PATTERNS: LazyLock<[(Regex, bool, bool); 3]> = LazyLock::new(|| {
    [
        (
            Regex::new(r"impl\s+([A-Za-z_]\w*)\s+for\s+([A-Za-z_]\w*)")
                .expect("valid Rust implementation regex"),
            true,
            true,
        ),
        (
            Regex::new(r"class\s+([A-Za-z_]\w*)\s+extends\s+([A-Za-z_]\w*)")
                .expect("valid extends regex"),
            false,
            false,
        ),
        (
            Regex::new(r"class\s+([A-Za-z_]\w*)\s+implements\s+([A-Za-z_]\w*)")
                .expect("valid implements regex"),
            true,
            false,
        ),
    ]
});
static SQL_TABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)create\s+table\s+(?:if\s+not\s+exists\s+)?[\"`]?([A-Za-z_]\w*)"#)
        .expect("valid SQL table regex")
});

#[derive(Default)]
pub struct FrameworkAnalysis {
    pub nodes: Vec<CodeNode>,
    pub edges: Vec<CodeEdge>,
}

/// Framework adapters are intentionally separate from AST extraction. Every edge is
/// emitted only when both the framework syntax and an unambiguous handler are found.
pub fn analyze_frameworks(
    root: &Path,
    files: &[ProjectFile],
    symbols: &[CodeNode],
) -> Result<FrameworkAnalysis> {
    let mut out = FrameworkAnalysis::default();
    for file in files {
        let source = match fs::read_to_string(root.join(&file.path)) {
            Ok(value) => value,
            Err(_) => continue,
        };
        analyze_next(file, symbols, &mut out);
        analyze_http_routes(file, &source, symbols, &mut out)?;
        analyze_flutter(file, &source, symbols, &mut out)?;
        analyze_inheritance(file, &source, symbols, &mut out)?;
        analyze_sql(file, &source, &mut out)?;
    }
    let symfony = symfony::analyze_symfony(root, files, symbols);
    out.nodes.extend(symfony.nodes);
    out.edges.extend(symfony.edges);
    Ok(out)
}
pub(super) fn virtual_node(
    file: &ProjectFile,
    kind: NodeKind,
    name: String,
    line: usize,
) -> CodeNode {
    CodeNode {
        id: format!(
            "{}:{}:{}:{}",
            kind.as_str().to_lowercase(),
            file.path,
            line,
            name
        ),
        kind,
        name,
        path: Some(file.path.clone()),
        language: Some(file.language),
        start_line: Some(line),
        end_line: Some(line),
        owner: None,
        source_scope: file.source_scope,
    }
}
fn find_handler<'a>(symbols: &'a [CodeNode], file: &str, name: &str) -> Option<&'a CodeNode> {
    let mut values = symbols
        .iter()
        .filter(|node| node.path.as_deref() == Some(file) && node.name == name);
    let first = values.next()?;
    values.next().is_none().then_some(first)
}
fn analyze_next(file: &ProjectFile, symbols: &[CodeNode], out: &mut FrameworkAnalysis) {
    let path = file.path.replace('\\', "/");
    // Next applications are often nested in `web/`, `frontend/` or `apps/name/`.
    // The segment boundary prevents a directory such as `myapp/` from matching.
    let rest = if let Some(rest) = path.strip_prefix("app/") {
        rest
    } else if let Some(position) = path.find("/app/") {
        &path[position + 5..]
    } else {
        return;
    };
    let (kind, route) = {
        if rest.ends_with("/page.tsx") || rest == "page.tsx" {
            (
                NodeKind::Page,
                format!(
                    "/{}",
                    rest.trim_end_matches("page.tsx")
                        .trim_matches('/')
                        .replace("[", ":")
                        .replace(']', "")
                ),
            )
        } else if rest.ends_with("/route.ts") {
            (
                NodeKind::ApiEndpoint,
                format!(
                    "/api/{}",
                    rest.trim_end_matches("route.ts")
                        .trim_matches('/')
                        .trim_start_matches("api/")
                ),
            )
        } else {
            return;
        }
    };
    let node = virtual_node(
        file,
        kind,
        if route == "/" {
            "/".into()
        } else {
            route.trim_end_matches('/').into()
        },
        1,
    );
    if kind == NodeKind::ApiEndpoint {
        for method in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            if let Some(handler) = find_handler(symbols, &file.path, method) {
                // Keep the original path node for compatibility, and expose each
                // observed HTTP export as an independently navigable endpoint.
                let endpoint = virtual_node(
                    file,
                    NodeKind::ApiEndpoint,
                    format!("{} {}", method, node.name),
                    handler.start_line.unwrap_or(1),
                );
                out.edges.push(CodeEdge::new(
                    endpoint.id.clone(),
                    handler.id.clone(),
                    RelationKind::HandledBy,
                ));
                out.nodes.push(endpoint);
                out.edges.push(CodeEdge::new(
                    node.id.clone(),
                    handler.id.clone(),
                    RelationKind::HandledBy,
                ));
            }
        }
    }
    out.nodes.push(node);
}
fn analyze_http_routes(
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    out: &mut FrameworkAnalysis,
) -> Result<()> {
    for (pattern_index, regex) in HTTP_ROUTE_PATTERNS.iter().enumerate() {
        for captures in regex.captures_iter(source) {
            let whole = captures.get(0).expect("full match");
            let line = source[..whole.start()]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 1;
            let values: Vec<_> = captures
                .iter()
                .skip(1)
                .flatten()
                .map(|v| v.as_str())
                .collect();
            let (method, route, handler) = if pattern_index == 0 {
                (
                    values.get(1).copied().unwrap_or("ANY"),
                    values[0],
                    values.get(2).copied(),
                )
            } else if pattern_index == 1 {
                (values[0], values[1], values.get(2).copied())
            } else {
                ("ANY", values[0], None)
            };
            let node = virtual_node(
                file,
                NodeKind::ApiEndpoint,
                format!("{} {}", method.to_uppercase(), route),
                line,
            );
            if let Some(handler) = handler
                .and_then(|value| value.rsplit([':', '@']).next())
                .and_then(|value| find_handler(symbols, &file.path, value))
            {
                out.edges.push(CodeEdge::new(
                    node.id.clone(),
                    handler.id.clone(),
                    RelationKind::HandledBy,
                ));
            }
            out.nodes.push(node);
        }
    }
    Ok(())
}
fn analyze_flutter(
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    out: &mut FrameworkAnalysis,
) -> Result<()> {
    if !file.path.ends_with(".dart") {
        return Ok(());
    }
    for captures in FLUTTER_ROUTE.captures_iter(source) {
        let whole = captures.get(0).expect("full");
        let line = source[..whole.start()]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1;
        let node = virtual_node(file, NodeKind::Route, captures[1].to_string(), line);
        if let Some(page) = symbols.iter().find(|node| node.name == captures[2]) {
            out.edges.push(CodeEdge::new(
                node.id.clone(),
                page.id.clone(),
                RelationKind::Renders,
            ));
        }
        out.nodes.push(node);
    }
    for captures in FLUTTER_PROVIDER.captures_iter(source) {
        let whole = captures.get(0).expect("full");
        let line = source[..whole.start()]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1;
        out.nodes.push(virtual_node(
            file,
            NodeKind::Provider,
            captures[1].to_string(),
            line,
        ));
    }
    Ok(())
}
fn analyze_inheritance(
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    out: &mut FrameworkAnalysis,
) -> Result<()> {
    for (regex, implements, rust_impl) in INHERITANCE_PATTERNS.iter() {
        for captures in regex.captures_iter(source) {
            let (left, right) = if *rust_impl {
                (&captures[2], &captures[1])
            } else {
                (&captures[1], &captures[2])
            };
            let source_node = symbols
                .iter()
                .filter(|node| node.path.as_deref() == Some(&file.path) && node.name == left)
                .collect::<Vec<_>>();
            let local_target = symbols
                .iter()
                .filter(|node| node.path.as_deref() == Some(&file.path) && node.name == right)
                .collect::<Vec<_>>();
            let target_node = if local_target.len() == 1 {
                local_target
            } else {
                symbols.iter().filter(|node| node.name == right).collect()
            };
            if source_node.len() == 1 && target_node.len() == 1 {
                out.edges.push(CodeEdge::new(
                    source_node[0].id.clone(),
                    target_node[0].id.clone(),
                    if *implements {
                        RelationKind::Implements
                    } else {
                        RelationKind::Extends
                    },
                ));
            }
        }
    }
    Ok(())
}
fn analyze_sql(file: &ProjectFile, source: &str, out: &mut FrameworkAnalysis) -> Result<()> {
    for captures in SQL_TABLE.captures_iter(source) {
        let whole = captures.get(0).expect("full");
        let line = source[..whole.start()]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1;
        out.nodes.push(virtual_node(
            file,
            NodeKind::DatabaseTable,
            captures[1].to_string(),
            line,
        ));
    }
    Ok(())
}
