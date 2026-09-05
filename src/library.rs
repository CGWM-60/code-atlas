use crate::{
    graph::project_graph::ProjectGraph,
    model::{edge::RelationKind, node::CodeNode},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, sync::LazyLock};

static TRIVIAL_GETTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:return)?&?(?:\$?this->|this\.|self\.|self->)([a-z_][a-z0-9_]*);?$")
        .expect("valid getter regex")
});
static TRIVIAL_SETTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:\$?this->|this\.|self\.|self->)?([a-z_][a-z0-9_]*)=\$?[a-z_][a-z0-9_]*;?(?:return(?:\$?this|self);?)?$")
        .expect("valid setter regex")
});
static SIMPLE_PROPERTY_RETURN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:return)?&?(?:self\.)?[a-z_][a-z0-9_]*;?$")
        .expect("valid property return regex")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealUsage {
    pub caller: String,
    pub path: String,
    pub line: Option<usize>,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AiFunctionParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub parameter_type: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AiFunctionReturn {
    #[serde(rename = "type")]
    pub return_type: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AiFunctionDocumentation {
    pub summary: String,
    pub purpose: String,
    pub parameters: Vec<AiFunctionParameter>,
    pub returns: AiFunctionReturn,
    pub errors: Vec<String>,
    pub side_effects: Vec<String>,
    pub use_cases: Vec<String>,
    pub limitations: Vec<String>,
    pub security_notes: Vec<String>,
    pub tags: Vec<String>,
    pub category: Option<String>,
}

impl AiFunctionDocumentation {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.summary.trim().is_empty() {
            return Err("summary must not be empty");
        }
        if self.purpose.trim().is_empty() {
            return Err("purpose must not be empty");
        }
        if self
            .parameters
            .iter()
            .any(|parameter| parameter.name.trim().is_empty())
        {
            return Err("parameter names must not be empty");
        }
        Ok(())
    }
}

pub type FunctionDocumentation = AiFunctionDocumentation;

#[derive(Deserialize)]
struct LegacyFunctionDocumentation {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    purpose: String,
    #[serde(default)]
    parameters: Vec<String>,
    #[serde(default)]
    return_value: String,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    side_effects: Vec<String>,
    #[serde(default)]
    use_cases: Vec<String>,
    #[serde(default)]
    limitations: Vec<String>,
    #[serde(default)]
    security_notes: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

fn deserialize_documentation<'de, D>(
    deserializer: D,
) -> Result<Option<AiFunctionDocumentation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(value) = Option::<serde_json::Value>::deserialize(deserializer)? else {
        return Ok(None);
    };
    if let Ok(documentation) = serde_json::from_value::<AiFunctionDocumentation>(value.clone()) {
        return Ok(Some(documentation));
    }
    let legacy = serde_json::from_value::<LegacyFunctionDocumentation>(value)
        .map_err(serde::de::Error::custom)?;
    Ok(Some(AiFunctionDocumentation {
        summary: legacy.summary,
        purpose: legacy.purpose,
        parameters: legacy
            .parameters
            .into_iter()
            .map(|name| AiFunctionParameter {
                name,
                parameter_type: None,
                description: String::new(),
            })
            .collect(),
        returns: AiFunctionReturn {
            return_type: None,
            description: legacy.return_value,
        },
        errors: legacy.errors,
        side_effects: legacy.side_effects,
        use_cases: legacy.use_cases,
        limitations: legacy.limitations,
        security_notes: legacy.security_notes,
        tags: legacy.tags,
        category: None,
    }))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationStatus {
    #[default]
    NotGenerated,
    Generating,
    Generated,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: String,
    pub display_name: String,
    pub language: String,
    pub category: String,
    pub tags: Vec<String>,
    pub description: String,
    pub source_project_id: String,
    #[serde(default)]
    pub source_project_name: String,
    pub source_node_id: String,
    pub source_path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub source_hash: String,
    pub source_scope: String,
    #[serde(default)]
    pub source_repository: Option<String>,
    #[serde(default)]
    pub source_version: Option<String>,
    #[serde(default)]
    pub source_branch: Option<String>,
    #[serde(default)]
    pub source_license: Option<String>,
    /// Reserved for an explicit, user-confirmed cross-language Recipe association.
    #[serde(default)]
    pub recipe_id: Option<String>,
    pub reuse_score: u8,
    #[serde(default)]
    pub knowledge_value: u8,
    pub real_usages: Vec<RealUsage>,
    #[serde(default)]
    pub calls: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub tests: Vec<RealUsage>,
    #[serde(default)]
    pub source_code: String,
    #[serde(default)]
    pub documentation_status: DocumentationStatus,
    #[serde(default)]
    pub documentation_provider: Option<String>,
    #[serde(default)]
    pub documentation_model: Option<String>,
    #[serde(default)]
    pub documentation_generated_at: Option<i64>,
    #[serde(default)]
    pub documentation_error: Option<String>,
    #[serde(default, deserialize_with = "deserialize_documentation")]
    pub documentation: Option<FunctionDocumentation>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct SourceProvenance {
    pub repository: Option<String>,
    pub version: Option<String>,
    pub branch: Option<String>,
    pub license: Option<String>,
}

pub fn detect_source_provenance(root: &Path) -> SourceProvenance {
    fn git_value(root: &Path, arguments: &[&str]) -> Option<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
        (!value.is_empty()).then_some(value)
    }
    let license = ["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"]
        .iter()
        .find_map(|name| {
            let path = root.join(name);
            path.is_file().then(|| (*name).to_string())
        });
    SourceProvenance {
        repository: git_value(root, &["config", "--get", "remote.origin.url"])
            .or_else(|| Some(root.to_string_lossy().into_owned())),
        version: git_value(root, &["rev-parse", "HEAD"]),
        branch: git_value(root, &["branch", "--show-current"]),
        license,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryCandidate {
    pub node: CodeNode,
    pub reuse_score: u8,
    pub knowledge_value: u8,
    pub reasons: Vec<String>,
    pub reuse_breakdown: Vec<ScoreFactor>,
    pub knowledge_breakdown: Vec<ScoreFactor>,
    pub real_usages: Vec<RealUsage>,
    pub source: String,
    pub signature: String,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub calls: Vec<String>,
    pub dependencies: Vec<String>,
    pub tests: Vec<RealUsage>,
    pub side_effects: Vec<String>,
    pub loc: usize,
    pub complexity: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoreFactor {
    pub label: String,
    pub points: i32,
}

pub fn candidate_for(graph: &ProjectGraph, node: &CodeNode) -> Option<LibraryCandidate> {
    if !node.source_scope.library_eligible()
        || !matches!(
            node.kind,
            crate::model::node::NodeKind::Function
                | crate::model::node::NodeKind::Method
                | crate::model::node::NodeKind::Constructor
        )
    {
        return None;
    }
    let source = source_for_node(graph, node)?;
    let body = inspect_body(node, &source);
    if body.trivial {
        return None;
    }
    let incoming = graph.incoming_edges(&node.id);
    let outgoing = graph.outgoing_edges(&node.id);
    let real_usages = incoming
        .iter()
        .filter(|edge| edge.relation == RelationKind::Calls)
        .filter_map(|edge| graph.find_node(&edge.source_id))
        .filter_map(|caller| {
            let path = caller.path.clone()?;
            let snippet = caller.start_line.and_then(|line| {
                std::fs::read_to_string(std::path::Path::new(&graph.root).join(&path))
                    .ok()?
                    .lines()
                    .nth(line.saturating_sub(1))
                    .map(str::trim)
                    .map(str::to_string)
            });
            Some(RealUsage {
                caller: caller.name.clone(),
                path,
                line: caller.start_line,
                snippet,
            })
        })
        .collect::<Vec<_>>();
    let tests = real_usages
        .iter()
        .filter(|usage| {
            let path = usage.path.to_lowercase();
            path.contains("test") || path.contains("spec")
        })
        .cloned()
        .collect::<Vec<_>>();
    let side_effects = outgoing
        .iter()
        .filter(|edge| {
            matches!(
                edge.relation,
                RelationKind::Writes
                    | RelationKind::Creates
                    | RelationKind::Emits
                    | RelationKind::Reads
            )
        })
        .map(|edge| edge.relation.as_str().to_string())
        .collect::<Vec<_>>();
    let dependency_edges = outgoing
        .iter()
        .filter(|edge| {
            matches!(
                edge.relation,
                RelationKind::Calls
                    | RelationKind::Uses
                    | RelationKind::Imports
                    | RelationKind::DependsOn
            )
        })
        .collect::<Vec<_>>();
    let dependencies = dependency_edges
        .iter()
        .filter_map(|edge| graph.find_node(&edge.target_id))
        .map(|target| target.name.clone())
        .collect::<Vec<_>>();
    let calls = outgoing
        .iter()
        .filter(|edge| edge.relation == RelationKind::Calls)
        .filter_map(|edge| graph.find_node(&edge.target_id))
        .map(|target| target.name.clone())
        .collect::<Vec<_>>();

    let mut reuse_breakdown = vec![ScoreFactor {
        label: "baseline".into(),
        points: 38,
    }];
    reuse_breakdown.push(ScoreFactor {
        label: format!("{} dependency edge(s)", dependency_edges.len()),
        points: match dependency_edges.len() {
            0 => 24,
            1..=2 => 16,
            3..=4 => 7,
            _ => -10,
        },
    });
    reuse_breakdown.push(ScoreFactor {
        label: format!("{} real incoming usage(s)", real_usages.len()),
        points: (real_usages.len().min(4) * 5) as i32,
    });
    if !tests.is_empty() {
        reuse_breakdown.push(ScoreFactor {
            label: "covered by real tests".into(),
            points: 10,
        });
    }
    if body.generic_signature {
        reuse_breakdown.push(ScoreFactor {
            label: "generic signature".into(),
            points: 10,
        });
    }
    if !side_effects.is_empty() {
        reuse_breakdown.push(ScoreFactor {
            label: "known side effects".into(),
            points: -15,
        });
    }
    if is_business_coupled(node) {
        reuse_breakdown.push(ScoreFactor {
            label: "project/domain coupling".into(),
            points: -8,
        });
    }
    let reuse_score = total_score(&reuse_breakdown);

    let mut knowledge_breakdown = vec![ScoreFactor {
        label: "baseline".into(),
        points: 8,
    }];
    knowledge_breakdown.push(ScoreFactor {
        label: format!("{} lines of executable source", body.loc),
        points: if body.loc >= 12 {
            18
        } else if body.loc >= 6 {
            12
        } else if body.loc >= 4 {
            10
        } else {
            3
        },
    });
    knowledge_breakdown.push(ScoreFactor {
        label: format!("complexity {}", body.complexity),
        points: (body.complexity.min(6) * 6) as i32,
    });
    knowledge_breakdown.push(ScoreFactor {
        label: format!("{} outgoing call(s)", calls.len()),
        points: (calls.len().min(5) * 4) as i32,
    });
    if body.has_error_handling {
        knowledge_breakdown.push(ScoreFactor {
            label: "error handling".into(),
            points: 15,
        });
    }
    if body.has_validation {
        knowledge_breakdown.push(ScoreFactor {
            label: "validation logic".into(),
            points: 18,
        });
    }
    if body.has_transformation {
        knowledge_breakdown.push(ScoreFactor {
            label: "transformation logic".into(),
            points: 18,
        });
    }
    if !tests.is_empty() {
        knowledge_breakdown.push(ScoreFactor {
            label: "tested behavior".into(),
            points: 12,
        });
    }
    if body.loc <= 3 && body.complexity == 0 && calls.is_empty() {
        knowledge_breakdown.push(ScoreFactor {
            label: "very small body".into(),
            points: -30,
        });
    }
    let knowledge_value = total_score(&knowledge_breakdown);
    let reasons = vec![
        format!("reuse {reuse_score}/100"),
        format!("knowledge {knowledge_value}/100"),
        format!("{} LOC · complexity {}", body.loc, body.complexity),
    ];
    Some(LibraryCandidate {
        node: node.clone(),
        reuse_score,
        knowledge_value,
        reasons,
        reuse_breakdown,
        knowledge_breakdown,
        real_usages,
        source,
        signature: body.signature,
        parameters: body.parameters,
        return_type: body.return_type,
        calls,
        dependencies,
        tests,
        side_effects,
        loc: body.loc,
        complexity: body.complexity,
    })
}

pub fn source_for_node(graph: &ProjectGraph, node: &CodeNode) -> Option<String> {
    let path = node.path.as_deref()?;
    if crate::ai::context_builder::sensitive_path(path) {
        return None;
    }
    let root = Path::new(&graph.root).canonicalize().ok()?;
    let absolute = root.join(path).canonicalize().ok()?;
    if !absolute.starts_with(&root) {
        return None;
    }
    let source = std::fs::read_to_string(absolute).ok()?;
    let start = node.start_line.unwrap_or(1).saturating_sub(1);
    let end = node.end_line.unwrap_or_else(|| source.lines().count());
    let extracted = source
        .lines()
        .skip(start)
        .take(end.saturating_sub(start) + 1)
        .collect::<Vec<_>>()
        .join("\n");
    Some(bound_declaration_source(&extracted).to_string())
}

/// Tree-sitter end rows may point at the first column of the following line.
/// Keep the exact declaration by stopping at the matching outer brace.
fn bound_declaration_source(source: &str) -> &str {
    let Some(open) = source.find('{') else {
        return source;
    };
    let mut depth = 0usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return &source[..open + offset + character.len_utf8()];
                }
            }
            _ => {}
        }
    }
    source
}

struct BodyInspection {
    trivial: bool,
    signature: String,
    parameters: Vec<String>,
    return_type: Option<String>,
    loc: usize,
    complexity: usize,
    generic_signature: bool,
    has_error_handling: bool,
    has_validation: bool,
    has_transformation: bool,
}

fn inspect_body(node: &CodeNode, source: &str) -> BodyInspection {
    let signature = source
        .split_once('{')
        .or_else(|| source.split_once("=>"))
        .map_or_else(
            || source.lines().next().unwrap_or_default(),
            |(value, _)| value,
        )
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let parameters: Vec<String> = signature
        .split_once('(')
        .and_then(|(_, rest)| rest.rsplit_once(')'))
        .map(|(parameters, _)| {
            parameters
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let return_type = signature
        .rsplit_once(')')
        .and_then(|(_, value)| value.trim().strip_prefix(':'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let bounded = bound_declaration_source(source);
    let body_storage;
    let body = if let Some((_, value)) = bounded.split_once('{') {
        value.rsplit_once('}').map_or(value, |(body, _)| body)
    } else if let Some((_, expression)) = bounded.split_once("=>") {
        expression
    } else {
        body_storage = bounded.lines().skip(1).collect::<Vec<_>>().join("\n");
        body_storage.as_str()
    };
    let compact = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('*'))
        .collect::<String>()
        .replace(' ', "")
        .to_lowercase();
    let loc = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(['/', '*']))
        .count();
    let assignments_only = compact
        .split(';')
        .filter(|statement| !statement.is_empty() && *statement != "return$this")
        .all(|statement| TRIVIAL_SETTER.is_match(&format!("{statement};")));
    let constructor_trivial = node.kind == crate::model::node::NodeKind::Constructor
        && (compact.is_empty() || assignments_only);
    let accessor_name = {
        let name = node.name.to_lowercase();
        name.starts_with("get")
            || name.starts_with("set")
            || name.starts_with("is_")
            || name.starts_with("is")
            || name.starts_with("has_")
            || name.starts_with("has")
            || (node.kind == crate::model::node::NodeKind::Method && parameters.len() <= 1)
    };
    let trivial = compact.is_empty()
        || TRIVIAL_GETTER.is_match(&compact)
        || (accessor_name && SIMPLE_PROPERTY_RETURN.is_match(&compact))
        || (accessor_name && TRIVIAL_SETTER.is_match(&compact))
        || constructor_trivial;
    let lower = body.to_lowercase();
    let complexity = [
        "if ", "if(", "else", "match", "switch", "for ", "foreach", "while", "catch", "&&", "||",
        "? ",
    ]
    .iter()
    .map(|token| lower.matches(token).count())
    .sum();
    let has_error_handling = ["throw ", "catch", "result<", "raise ", "except"]
        .iter()
        .any(|token| lower.contains(token));
    let has_validation = [
        "valid",
        "filter_var",
        "assert",
        "preg_match",
        "regex",
        "constraint",
    ]
    .iter()
    .any(|token| lower.contains(token));
    let has_transformation = [
        "map(",
        "filter(",
        "reduce(",
        "array_map",
        "array_filter",
        "sort(",
        "strtolower",
        "trim(",
        "json_",
    ]
    .iter()
    .any(|token| lower.contains(token));
    let generic_signature = !is_business_coupled(node)
        && !parameters.iter().any(|parameter| {
            let lower = parameter.to_lowercase();
            lower.contains("entity") || lower.contains("repository") || lower.contains("controller")
        });
    BodyInspection {
        trivial,
        signature,
        parameters,
        return_type,
        loc,
        complexity,
        generic_signature,
        has_error_handling,
        has_validation,
        has_transformation,
    }
}

fn is_business_coupled(node: &CodeNode) -> bool {
    let value = format!(
        "{} {}",
        node.owner.as_deref().unwrap_or_default(),
        node.path.as_deref().unwrap_or_default()
    )
    .to_lowercase();
    ["controller", "entity", "repository", "domain", "handler"]
        .iter()
        .any(|token| value.contains(token))
}

fn total_score(factors: &[ScoreFactor]) -> u8 {
    factors
        .iter()
        .map(|factor| factor.points)
        .sum::<i32>()
        .clamp(0, 100) as u8
}

#[cfg(test)]
mod documentation_compatibility_tests {
    use super::LibraryEntry;

    #[test]
    fn reads_legacy_documentation_without_losing_saved_entries() {
        let value = serde_json::json!({
            "id":"library:legacy","display_name":"legacy","language":"Rust","category":"utility","tags":[],"description":"",
            "source_project_id":"project","source_node_id":"node","source_path":"src/lib.rs","start_line":1,"end_line":2,
            "source_hash":"abc","source_scope":"project","reuse_score":70,"real_usages":[],
            "documentation":{"summary":"Legacy summary","purpose":"Legacy purpose","parameters":["input"],"return_value":"A value","errors":[],"side_effects":[],"use_cases":[],"limitations":[],"security_notes":[],"tags":[]},
            "created_at":1,"updated_at":1
        });
        let entry: LibraryEntry = serde_json::from_value(value).expect("legacy entry");
        let documentation = entry.documentation.expect("documentation");
        assert_eq!(documentation.parameters[0].name, "input");
        assert_eq!(documentation.returns.description, "A value");
    }
}
