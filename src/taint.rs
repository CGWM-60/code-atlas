//! Bounded, heuristic data propagation over resolved CALLS edges. This is not a
//! proof of exploitability: aliasing, sanitizers and path conditions are incomplete.
use crate::{
    findings::SourceSpan,
    graph::project_graph::ProjectGraph,
    model::{
        edge::RelationKind,
        node::{CodeNode, NodeKind},
    },
    retrieval::source,
};
use regex::Regex;
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    sync::LazyLock,
};
static INPUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\b(?:request|req)\s*(?:\.|->)\s*(?:body|query|params|input|json|form)|\$_(?:GET|POST|REQUEST)|\b(?:argv|query_params|search_params)\b)").unwrap()
});
static ASSIGN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\blet\s+(?:mut\s+)?|\bconst\s+|\bvar\s+)?(\$?[A-Za-z_]\w*)\s*(?::\s*[^=;]+)?=\s*([^=].*)").unwrap()
});
static IDENTIFIER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$?[A-Za-z_]\w*").unwrap());
static SINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(exec|system|shell_exec|query|execute|redirect|writeFile|write_file)\s*\((.*)\)",
    )
    .unwrap()
});
#[derive(Debug, Clone)]
pub struct TaintCandidate {
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub spans: Vec<SourceSpan>,
    pub sink: String,
}
#[derive(Clone)]
struct State {
    node: String,
    parameters: BTreeMap<String, Vec<SourceSpan>>,
    nodes: Vec<String>,
    edges: Vec<String>,
    depth: usize,
}
fn span(node: &CodeNode, line: usize, text: &str, label: &str, role: &str) -> SourceSpan {
    SourceSpan {
        path: node.path.clone().unwrap_or_default(),
        start_line: line,
        end_line: line,
        start_column: 1,
        end_column: text.chars().count() + 1,
        snippet: Some(text.trim().into()),
        label: Some(label.into()),
        role: Some(role.into()),
    }
}
fn parameter_names(text: &str) -> Vec<String> {
    let Some((_, rest)) = text.split_once('(') else {
        return vec![];
    };
    let Some((params, _)) = rest.split_once(')') else {
        return vec![];
    };
    params
        .split(',')
        .filter_map(|p| {
            let before = p
                .split(':')
                .next()
                .unwrap_or(p)
                .split('=')
                .next()
                .unwrap_or(p)
                .trim();
            let tokens = IDENTIFIER
                .find_iter(before)
                .map(|m| m.as_str())
                .collect::<Vec<_>>();
            tokens
                .iter()
                .find(|t| t.starts_with('$'))
                .or_else(|| {
                    if before.contains(':') || before.starts_with("mut ") {
                        tokens.last()
                    } else {
                        tokens.first()
                    }
                })
                .map(|s| s.to_string())
        })
        .collect()
}
fn inherited(
    expression: &str,
    variables: &BTreeMap<String, Vec<SourceSpan>>,
) -> Option<Vec<SourceSpan>> {
    IDENTIFIER
        .find_iter(expression)
        .find_map(|m| variables.get(m.as_str()).cloned())
}
pub fn analyze(graph: &ProjectGraph) -> Vec<TaintCandidate> {
    let procedures = graph
        .nodes
        .iter()
        .filter(|n| {
            n.source_scope.library_eligible()
                && matches!(
                    n.kind,
                    NodeKind::Function
                        | NodeKind::Method
                        | NodeKind::Handler
                        | NodeKind::Constructor
                )
        })
        .filter_map(|n| source(graph, n).map(|s| (n.id.clone(), s)))
        .collect::<BTreeMap<_, _>>();
    let mut queue = VecDeque::new();
    for (id, (text, _, _)) in &procedures {
        if INPUT.is_match(text) {
            queue.push_back(State {
                node: id.clone(),
                parameters: BTreeMap::new(),
                nodes: vec![id.clone()],
                edges: vec![],
                depth: 0,
            });
        }
    }
    let mut visited = HashSet::new();
    let mut results = Vec::new();
    let mut budget = 256;
    while let Some(mut state) = queue.pop_front() {
        if budget == 0 || results.len() >= 100 {
            break;
        }
        budget -= 1;
        let key = format!(
            "{}:{:?}",
            state.node,
            state.parameters.keys().collect::<Vec<_>>()
        );
        if !visited.insert(key) {
            continue;
        }
        let Some(node) = graph.find_node(&state.node) else {
            continue;
        };
        let Some((text, start, _)) = procedures.get(&state.node) else {
            continue;
        };
        let calls = graph
            .outgoing_edges(&state.node)
            .into_iter()
            .filter(|e| e.relation == RelationKind::Calls)
            .filter_map(|e| graph.find_node(&e.target_id).map(|n| (e, n)))
            .collect::<Vec<_>>();
        for (offset, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if ["//", "#", "/*", "*"]
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
            {
                continue;
            }
            let line_number = start + offset;
            let code = mask_literals(line);
            if let Some(assignment) = ASSIGN.captures(&code) {
                let name = assignment[1].to_string();
                let expression = &assignment[2];
                if INPUT.is_match(expression) {
                    state.parameters.insert(
                        name,
                        vec![span(
                            node,
                            line_number,
                            line,
                            "Entrée externe observée",
                            "source",
                        )],
                    );
                } else if let Some(mut chain) = inherited(expression, &state.parameters) {
                    chain.push(span(
                        node,
                        line_number,
                        line,
                        "Propagation par affectation",
                        "transform",
                    ));
                    state.parameters.insert(name, chain);
                } else {
                    state.parameters.remove(&name);
                }
            }
            if let Some(sink) = SINK.captures(&code) {
                let expression = &sink[2];
                let chain = if INPUT.is_match(expression) {
                    Some(vec![span(
                        node,
                        line_number,
                        line,
                        "Entrée externe observée",
                        "source",
                    )])
                } else {
                    inherited(expression, &state.parameters)
                };
                if let Some(mut chain) = chain {
                    chain.push(span(
                        node,
                        line_number,
                        line,
                        &format!("Appel sensible : {}", &sink[1]),
                        "sink",
                    ));
                    results.push(TaintCandidate {
                        node_ids: state.nodes.clone(),
                        edge_ids: state.edges.clone(),
                        spans: chain,
                        sink: sink[1].into(),
                    });
                }
            }
            if state.depth >= 4 {
                continue;
            }
            for (edge, callee) in &calls {
                let Ok(pattern) =
                    Regex::new(&format!(r"\b{}\s*\(([^)]*)\)", regex::escape(&callee.name)))
                else {
                    continue;
                };
                let Some(capture) = pattern.captures(&code) else {
                    continue;
                };
                let Some((callee_text, _, _)) = procedures.get(&callee.id) else {
                    continue;
                };
                let params = parameter_names(callee_text);
                let mut passed = BTreeMap::new();
                for (arg, param) in capture[1].split(',').zip(params) {
                    let chain = if INPUT.is_match(arg) {
                        Some(vec![span(
                            node,
                            line_number,
                            line,
                            "Entrée externe passée en argument",
                            "source",
                        )])
                    } else {
                        inherited(arg, &state.parameters)
                    };
                    if let Some(mut chain) = chain {
                        chain.push(span(
                            node,
                            line_number,
                            line,
                            &format!("Appel résolu vers {}", callee.name),
                            "call",
                        ));
                        passed.insert(param, chain);
                    }
                }
                if !passed.is_empty() {
                    let mut nodes = state.nodes.clone();
                    nodes.push(callee.id.clone());
                    let mut edges = state.edges.clone();
                    edges.push(edge.id.clone());
                    queue.push_back(State {
                        node: callee.id.clone(),
                        parameters: passed,
                        nodes,
                        edges,
                        depth: state.depth + 1,
                    });
                }
            }
        }
    }
    results
}

fn mask_literals(line: &str) -> String {
    let mut quote = None;
    let mut escape = false;
    let mut result = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(delimiter) = quote {
            result.push(' ');
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == delimiter {
                quote = None;
            }
        } else if ['\'', '"', '`'].contains(&c) {
            quote = Some(c);
            result.push(' ');
        } else if c == '/' && chars.peek() == Some(&'/') {
            break;
        } else {
            result.push(c);
        }
    }
    result
}
