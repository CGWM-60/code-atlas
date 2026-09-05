use super::{FrameworkAnalysis, virtual_node};
use crate::model::{
    edge::{CodeEdge, RelationKind},
    node::{CodeNode, NodeKind},
    project::{ProjectFile, SourceScope},
};
use regex::Regex;
use std::{fs, path::Path, sync::LazyLock};

static PHP_ROUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:#\[Route\(|@Route\()[^\]\n]*(?:path\s*:\s*)?['\"]([^'\"]+)['\"][^\]\n]*"#)
        .expect("valid Symfony route regex")
});
static ROUTE_METHOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"methods?\s*:\s*\[?\s*['\"]([A-Za-z]+)['\"]"#)
        .expect("valid Symfony route method regex")
});
static RENDER_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:->)?render\s*\(\s*['\"]([^'\"]+\.twig)['\"]"#)
        .expect("valid Symfony render regex")
});
static MESSAGE_ARGUMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"function\s+__invoke\s*\(\s*([A-Za-z_\\][A-Za-z0-9_\\]*)\s+\$")
        .expect("valid message argument regex")
});
static LISTENED_EVENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:event\s*:\s*)?([A-Za-z_\\][A-Za-z0-9_\\]*)::class")
        .expect("valid Symfony event regex")
});

pub fn analyze_symfony(
    root: &Path,
    files: &[ProjectFile],
    symbols: &[CodeNode],
) -> FrameworkAnalysis {
    if !is_symfony(root) {
        return FrameworkAnalysis::default();
    }
    let mut output = FrameworkAnalysis::default();
    for file in files {
        let source = match fs::read_to_string(root.join(&file.path)) {
            Ok(source) => source,
            Err(_) => continue,
        };
        if file.path.ends_with(".php") {
            analyze_php_routes(file, &source, symbols, &mut output);
            analyze_templates(root, file, &source, symbols, &mut output);
            analyze_command(file, &source, symbols, &mut output);
            analyze_events_and_messages(file, &source, symbols, &mut output);
        } else if is_route_yaml(&file.path) {
            analyze_yaml_routes(file, &source, symbols, &mut output);
        }
    }
    output
}

fn is_symfony(root: &Path) -> bool {
    root.join("symfony.lock").is_file()
        || fs::read_to_string(root.join("composer.json"))
            .is_ok_and(|source| source.contains("symfony/framework-bundle"))
}

fn analyze_php_routes(
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    output: &mut FrameworkAnalysis,
) {
    for handler in symbols
        .iter()
        .filter(|node| node.kind == NodeKind::Method && node.path.as_deref() == Some(&file.path))
    {
        let method_line = handler.start_line.unwrap_or(1);
        let prefix_start = method_line.saturating_sub(21);
        let prefix = source
            .lines()
            .skip(prefix_start)
            .take(method_line.saturating_sub(prefix_start + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let Some(captures) = PHP_ROUTE.captures_iter(&prefix).last() else {
            continue;
        };
        let whole = captures.get(0).expect("full route");
        let line = prefix_start + line_at(&prefix, whole.start());
        let method = ROUTE_METHOD
            .captures(whole.as_str())
            .and_then(|values| values.get(1))
            .map(|value| value.as_str().to_uppercase())
            .unwrap_or_else(|| "ANY".into());
        let mut route = virtual_node(
            file,
            NodeKind::ApiEndpoint,
            format!("{method} {}", &captures[1]),
            line,
        );
        route.owner = handler.owner.clone();
        output.edges.push(CodeEdge::new(
            route.id.clone(),
            handler.id.clone(),
            RelationKind::HandledBy,
        ));
        output.nodes.push(route);
    }
}

fn analyze_yaml_routes(
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    output: &mut FrameworkAnalysis,
) {
    #[derive(Default)]
    struct RouteBlock {
        name: String,
        path: Option<String>,
        controller: Option<String>,
        methods: Option<String>,
        line: usize,
    }
    fn flush(
        file: &ProjectFile,
        block: &mut RouteBlock,
        symbols: &[CodeNode],
        output: &mut FrameworkAnalysis,
    ) {
        let Some(path) = block.path.take() else {
            *block = RouteBlock::default();
            return;
        };
        let method = block.methods.take().unwrap_or_else(|| "ANY".into());
        let mut node = virtual_node(
            file,
            NodeKind::ApiEndpoint,
            format!("{} {}", method.to_uppercase(), path),
            block.line,
        );
        if let Some(controller) = block.controller.take() {
            let (owner, method) = controller
                .split_once("::")
                .map_or((controller.as_str(), "__invoke"), |values| values);
            let owner = owner.rsplit('\\').next().unwrap_or(owner);
            let handlers = symbols
                .iter()
                .filter(|symbol| {
                    symbol.owner.as_deref() == Some(owner)
                        && symbol.name == method
                        && symbol.kind == NodeKind::Method
                })
                .collect::<Vec<_>>();
            if handlers.len() == 1 {
                node.owner = Some(owner.to_string());
                output.edges.push(CodeEdge::new(
                    node.id.clone(),
                    handlers[0].id.clone(),
                    RelationKind::HandledBy,
                ));
            }
        }
        output.nodes.push(node);
        *block = RouteBlock::default();
    }

    let key = Regex::new(r"^([A-Za-z0-9_.-]+):\s*$").expect("valid YAML route key");
    let value = Regex::new(r"^\s+(path|controller|_controller|methods):\s*(.+?)\s*$")
        .expect("valid YAML route value");
    let mut block = RouteBlock::default();
    for (index, line) in source.lines().enumerate() {
        if let Some(captures) = key.captures(line) {
            if !block.name.is_empty() {
                flush(file, &mut block, symbols, output);
            }
            block.name = captures[1].to_string();
            block.line = index + 1;
        } else if let Some(captures) = value.captures(line) {
            let raw = captures[2]
                .trim()
                .trim_matches(['\'', '"', '[', ']'])
                .to_string();
            match &captures[1] {
                "path" => block.path = Some(raw),
                "controller" | "_controller" => block.controller = Some(raw),
                "methods" => {
                    block.methods = raw.split(',').next().map(str::trim).map(str::to_string)
                }
                _ => {}
            }
        }
    }
    if !block.name.is_empty() {
        flush(file, &mut block, symbols, output);
    }
}

fn analyze_templates(
    root: &Path,
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    output: &mut FrameworkAnalysis,
) {
    for captures in RENDER_CALL.captures_iter(source) {
        let whole = captures.get(0).expect("full render call");
        let line = line_at(source, whole.start());
        let template_path = format!("templates/{}", &captures[1]);
        if !root.join(&template_path).is_file() {
            continue;
        }
        let id = format!("template:{template_path}");
        if !output.nodes.iter().any(|node| node.id == id) {
            output.nodes.push(CodeNode {
                id: id.clone(),
                kind: NodeKind::Template,
                name: captures[1].to_string(),
                path: Some(template_path),
                language: None,
                start_line: Some(1),
                end_line: None,
                owner: None,
                source_scope: SourceScope::Project,
            });
        }
        if let Some(caller) = enclosing_method(symbols, &file.path, line) {
            output
                .edges
                .push(CodeEdge::new(caller.id.clone(), id, RelationKind::Renders));
        }
    }
}

fn analyze_command(
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    output: &mut FrameworkAnalysis,
) {
    if !source.contains("#[AsCommand") && !file.path.replace('\\', "/").contains("/Command/") {
        return;
    }
    let Some(class) = class_in_file(symbols, &file.path) else {
        return;
    };
    if let Some(execute) = symbols.iter().find(|node| {
        node.path.as_deref() == Some(&file.path)
            && node.owner.as_deref() == Some(&class.name)
            && node.name == "execute"
    }) {
        output.edges.push(CodeEdge::new(
            class.id.clone(),
            execute.id.clone(),
            RelationKind::EntryPoint,
        ));
    }
}

fn analyze_events_and_messages(
    file: &ProjectFile,
    source: &str,
    symbols: &[CodeNode],
    output: &mut FrameworkAnalysis,
) {
    let Some(handler) = class_in_file(symbols, &file.path) else {
        return;
    };
    if source.contains("#[AsMessageHandler")
        && let Some(captures) = MESSAGE_ARGUMENT.captures(source)
        && let Some(message) =
            unique_class(symbols, captures.get(1).map_or("", |value| value.as_str()))
    {
        output.edges.push(CodeEdge::new(
            message.id.clone(),
            handler.id.clone(),
            RelationKind::HandledBy,
        ));
    }
    if source.contains("#[AsEventListener")
        || source.contains("EventSubscriberInterface")
        || file.path.contains("EventSubscriber")
    {
        for captures in LISTENED_EVENT.captures_iter(source) {
            if let Some(event) = unique_class(symbols, &captures[1])
                && event.id != handler.id
            {
                output.edges.push(CodeEdge::new(
                    event.id.clone(),
                    handler.id.clone(),
                    RelationKind::Listens,
                ));
            }
        }
    }
}

fn unique_class<'a>(symbols: &'a [CodeNode], raw_name: &str) -> Option<&'a CodeNode> {
    let name = raw_name.rsplit('\\').next().unwrap_or(raw_name);
    let values = symbols
        .iter()
        .filter(|node| node.kind == NodeKind::Class && node.name == name)
        .collect::<Vec<_>>();
    (values.len() == 1).then(|| values[0])
}

fn class_in_file<'a>(symbols: &'a [CodeNode], path: &str) -> Option<&'a CodeNode> {
    let values = symbols
        .iter()
        .filter(|node| node.kind == NodeKind::Class && node.path.as_deref() == Some(path))
        .collect::<Vec<_>>();
    (values.len() == 1).then(|| values[0])
}

fn enclosing_method<'a>(symbols: &'a [CodeNode], path: &str, line: usize) -> Option<&'a CodeNode> {
    symbols
        .iter()
        .filter(|node| node.kind == NodeKind::Method && node.path.as_deref() == Some(path))
        .filter(|node| {
            node.start_line.unwrap_or(usize::MAX) <= line
                && node.end_line.unwrap_or_default() >= line
        })
        .min_by_key(|node| {
            node.end_line.unwrap_or(usize::MAX) - node.start_line.unwrap_or_default()
        })
}

fn is_route_yaml(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_lowercase();
    (normalized.ends_with(".yaml") || normalized.ends_with(".yml"))
        && (normalized.contains("routes") || normalized.contains("routing"))
}

fn line_at(source: &str, byte: usize) -> usize {
    source[..byte]
        .bytes()
        .filter(|value| *value == b'\n')
        .count()
        + 1
}
