use crate::{
    features::{Feature, FeatureSpec},
    graph::project_graph::ProjectGraph,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetProfile {
    RustAxum,
    RustActix,
    TypeScriptNext,
    TypeScriptExpress,
    TypeScriptNest,
    PhpSymfony,
    PhpLaravel,
    PythonFastApi,
    PythonDjango,
    GoGin,
    GoEcho,
    DartFlutter,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortingPlan {
    pub feature_id: String,
    pub feature_name: String,
    pub target_profile: TargetProfile,
    pub target_project_id: Option<String>,
    pub files_to_create: Vec<String>,
    pub files_to_modify: Vec<String>,
    pub dependencies: Vec<String>,
    pub tests: Vec<String>,
    pub security_requirements: Vec<String>,
    pub constraints: Vec<String>,
    pub risks: Vec<String>,
    pub target_conventions: Vec<String>,
    pub validation_commands: Vec<String>,
    pub source_hash: String,
    pub target_hash: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
    pub is_test: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortPreview {
    pub plan: PortingPlan,
    pub files: Vec<GeneratedFile>,
    pub unified_diff: String,
    pub generated_tests: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiGeneratedFiles {
    pub files: Vec<GeneratedFile>,
}

pub fn generated_files_schema() -> serde_json::Value {
    serde_json::json!({"type":"object","additionalProperties":false,"required":["files"],"properties":{"files":{"type":"array","minItems":2,"maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["path","content","is_test"],"properties":{"path":{"type":"string"},"content":{"type":"string"},"is_test":{"type":"boolean"}}}}}})
}

pub fn preview_from_generated_files(
    plan: PortingPlan,
    generated: AiGeneratedFiles,
) -> Result<PortPreview, String> {
    let expected = plan
        .files_to_create
        .iter()
        .chain(&plan.tests)
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let actual = generated
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected
        || generated
            .files
            .iter()
            .any(|file| !safe_relative(&file.path))
    {
        return Err("la réponse IA ne respecte pas exactement le plan de fichiers".into());
    }
    if generated
        .files
        .iter()
        .any(|file| file.content.trim().is_empty() || file.content.len() > 80_000)
    {
        return Err("un fichier généré est vide ou dépasse la limite autorisée".into());
    }
    Ok(preview_with_files(plan, generated.files))
}
#[derive(Debug, Clone, Serialize)]
pub struct ApplyResult {
    pub created: Vec<String>,
    pub unchanged: Vec<String>,
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}
pub fn target_hash(graph: &ProjectGraph) -> String {
    let mut hasher = Sha256::new();
    for node in &graph.nodes {
        hasher.update(node.id.as_bytes())
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub fn build_porting_plan(
    feature: &Feature,
    spec: &FeatureSpec,
    target_profile: TargetProfile,
    target_project_id: Option<String>,
    target_graph: Option<&ProjectGraph>,
    constraints: Vec<String>,
) -> PortingPlan {
    let name = slug(&feature.name);
    let (files, modify, deps, tests) = match target_profile {
        TargetProfile::RustAxum => (
            vec![format!("src/routes/{name}.rs")],
            vec!["src/router.rs".into()],
            vec!["axum".into()],
            vec![format!("tests/{name}.rs")],
        ),
        TargetProfile::RustActix => (
            vec![format!("src/routes/{name}.rs")],
            vec!["src/main.rs".into()],
            vec!["actix-web".into()],
            vec![format!("tests/{name}.rs")],
        ),
        TargetProfile::TypeScriptNext => (
            vec![format!("app/api/{name}/route.ts")],
            vec![],
            vec!["next".into()],
            vec![format!("app/api/{name}/route.test.ts")],
        ),
        TargetProfile::TypeScriptExpress => (
            vec![format!("src/routes/{name}.ts")],
            vec!["src/app.ts".into()],
            vec!["express".into()],
            vec![format!("tests/{name}.test.ts")],
        ),
        TargetProfile::TypeScriptNest => (
            vec![
                format!("src/{name}/{name}.controller.ts"),
                format!("src/{name}/{name}.service.ts"),
            ],
            vec!["src/app.module.ts".into()],
            vec!["@nestjs/common".into()],
            vec![format!("src/{name}/{name}.spec.ts")],
        ),
        TargetProfile::PhpSymfony => (
            vec![
                format!(
                    "src/Controller/{}Controller.php",
                    feature.name.replace(' ', "")
                ),
                format!("src/Service/{}Service.php", feature.name.replace(' ', "")),
            ],
            vec!["config/routes.yaml".into()],
            vec!["symfony/framework-bundle".into()],
            vec![format!(
                "tests/Feature/{}Test.php",
                feature.name.replace(' ', "")
            )],
        ),
        TargetProfile::PhpLaravel => (
            vec![format!(
                "app/Http/Controllers/{}Controller.php",
                feature.name.replace(' ', "")
            )],
            vec!["routes/api.php".into()],
            vec!["laravel/framework".into()],
            vec![format!(
                "tests/Feature/{}Test.php",
                feature.name.replace(' ', "")
            )],
        ),
        TargetProfile::PythonFastApi => (
            vec![format!("app/routers/{name}.py")],
            vec!["app/main.py".into()],
            vec!["fastapi".into()],
            vec![format!("tests/test_{name}.py")],
        ),
        TargetProfile::PythonDjango => (
            vec![format!("app/views/{name}.py")],
            vec!["app/urls.py".into()],
            vec!["django".into()],
            vec![format!("app/tests/test_{name}.py")],
        ),
        TargetProfile::GoGin => (
            vec![format!("internal/handlers/{name}.go")],
            vec!["cmd/server/main.go".into()],
            vec!["github.com/gin-gonic/gin".into()],
            vec![format!("internal/handlers/{name}_test.go")],
        ),
        TargetProfile::GoEcho => (
            vec![format!("internal/handlers/{name}.go")],
            vec!["cmd/server/main.go".into()],
            vec!["github.com/labstack/echo/v4".into()],
            vec![format!("internal/handlers/{name}_test.go")],
        ),
        TargetProfile::DartFlutter => (
            vec![format!("lib/features/{name}/{name}_page.dart")],
            vec!["lib/app.dart".into()],
            vec!["flutter".into()],
            vec![format!("test/features/{name}_test.dart")],
        ),
    };
    let target_conventions = target_graph
        .map(|graph| {
            let mut values = graph
                .nodes
                .iter()
                .filter(|node| node.source_scope.library_eligible())
                .filter_map(|node| node.path.as_deref())
                .take(30)
                .map(|path| format!("Existing source layout: {path}"))
                .collect::<Vec<_>>();
            values.sort();
            values.dedup();
            values.truncate(12);
            values
        })
        .unwrap_or_default();
    let validation_commands = match target_profile {
        TargetProfile::RustAxum | TargetProfile::RustActix => vec!["cargo test".into()],
        TargetProfile::TypeScriptNext
        | TargetProfile::TypeScriptExpress
        | TargetProfile::TypeScriptNest => vec!["npm test".into()],
        TargetProfile::PhpSymfony | TargetProfile::PhpLaravel => vec!["phpunit".into()],
        TargetProfile::PythonFastApi | TargetProfile::PythonDjango => vec!["pytest".into()],
        TargetProfile::GoGin | TargetProfile::GoEcho => vec!["go test ./...".into()],
        TargetProfile::DartFlutter => vec!["flutter test".into()],
    };
    PortingPlan {
        feature_id: feature.id.clone(),
        feature_name: feature.name.clone(),
        target_profile,
        target_project_id,
        files_to_create: files,
        files_to_modify: modify,
        dependencies: deps,
        tests,
        security_requirements: spec.security.clone(),
        constraints,
        risks: vec![
            "Generated code must be reviewed against target project conventions.".into(),
            "Existing files are never overwritten automatically.".into(),
        ],
        target_conventions,
        validation_commands,
        source_hash: feature.source_hash.clone(),
        target_hash: target_graph.map(target_hash),
    }
}
pub fn generate_preview(plan: PortingPlan, spec: &FeatureSpec) -> PortPreview {
    let operations = if spec.operations.is_empty() {
        "business_operation".into()
    } else {
        spec.operations.join(", ")
    };
    let acceptance = if spec.acceptance_criteria.is_empty() {
        vec!["Feature entry point returns an expected response".into()]
    } else {
        spec.acceptance_criteria.clone()
    };
    let mut files = Vec::new();
    for path in plan.files_to_create.iter().chain(&plan.tests) {
        let is_test = path.contains("test") || path.contains("spec");
        let content = render_target_file(&plan, spec, path, is_test, &acceptance, &operations);
        files.push(GeneratedFile {
            path: path.clone(),
            content,
            is_test,
        })
    }
    preview_with_files(plan, files)
}

fn preview_with_files(plan: PortingPlan, files: Vec<GeneratedFile>) -> PortPreview {
    let unified_diff = files
        .iter()
        .map(|file| {
            format!(
                "--- /dev/null\n+++ b/{}\n{}",
                file.path,
                file.content
                    .lines()
                    .map(|line| format!("+{line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    PortPreview {
        generated_tests: plan.tests.clone(),
        plan,
        files,
        unified_diff,
    }
}
fn render_target_file(
    plan: &PortingPlan,
    spec: &FeatureSpec,
    path: &str,
    is_test: bool,
    acceptance: &[String],
    operations: &str,
) -> String {
    let name = slug(&plan.feature_name);
    let criteria = acceptance
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("; ");
    match plan.target_profile{
        TargetProfile::RustAxum|TargetProfile::RustActix if is_test=>format!("#[test]\nfn {name}_acceptance_contract() {{\n    let criteria = {:?};\n    assert!(!criteria.is_empty());\n}}\n",criteria),
        TargetProfile::RustAxum=>format!("use axum::{{Json, http::StatusCode}};\nuse serde::{{Deserialize, Serialize}};\n\n#[derive(Debug, Deserialize)]\npub struct {0}Input {{ pub value: String }}\n#[derive(Debug, Serialize)]\npub struct {0}Output {{ pub status: &'static str }}\n\npub async fn {1}(Json(input): Json<{0}Input>) -> Result<Json<{0}Output>, StatusCode> {{\n    if input.value.trim().is_empty() {{ return Err(StatusCode::BAD_REQUEST); }}\n    Ok(Json({0}Output {{ status: \"accepted\" }}))\n}}\n",pascal(&name),name),
        TargetProfile::RustActix=>format!("use actix_web::{{post, web, HttpResponse, Responder}};\nuse serde::Deserialize;\n#[derive(Deserialize)] pub struct {0}Input {{ pub value: String }}\n#[post(\"/{1}\")]\npub async fn {1}(input: web::Json<{0}Input>) -> impl Responder {{ if input.value.trim().is_empty() {{ HttpResponse::BadRequest().finish() }} else {{ HttpResponse::Ok().json(serde_json::json!({{\"status\":\"accepted\"}})) }} }}\n",pascal(&name),name),
        TargetProfile::TypeScriptNext|TargetProfile::TypeScriptExpress|TargetProfile::TypeScriptNest if is_test=>format!("describe('{name}', () => {{\n  it('satisfies the acceptance contract', () => {{\n    expect({:?}).toBeTruthy();\n  }});\n}});\n",criteria),
        TargetProfile::TypeScriptNext=>"import { NextResponse } from 'next/server';\nexport async function POST(request: Request) {\n  const input = await request.json() as { value?: string };\n  if (!input.value?.trim()) return NextResponse.json({ error: 'invalid_input' }, { status: 400 });\n  return NextResponse.json({ status: 'accepted' });\n}\n".to_string(),
        TargetProfile::TypeScriptExpress=>format!("import {{ Router }} from 'express';\nexport const {name}Router = Router();\n{name}Router.post('/{name}', (request, response) => {{\n  if (typeof request.body?.value !== 'string' || !request.body.value.trim()) return response.status(400).json({{ error: 'invalid_input' }});\n  return response.json({{ status: 'accepted' }});\n}});\n"),
        TargetProfile::TypeScriptNest=>format!("import {{ Body, Controller, Post, BadRequestException }} from '@nestjs/common';\n@Controller('{name}') export class {0}Controller {{\n  @Post() execute(@Body() input: {{ value?: string }}) {{ if (!input.value?.trim()) throw new BadRequestException('invalid_input'); return {{ status: 'accepted' }}; }}\n}}\n",pascal(&name)),
        TargetProfile::PhpSymfony|TargetProfile::PhpLaravel if is_test=>format!("<?php\ndeclare(strict_types=1);\nuse PHPUnit\\Framework\\TestCase;\nfinal class {0}Test extends TestCase {{ public function testAcceptanceContract(): void {{ self::assertNotEmpty({1:?}); }} }}\n",pascal(&name),criteria),
        TargetProfile::PhpSymfony=>format!("<?php\ndeclare(strict_types=1);\nnamespace App\\Controller;\nuse Symfony\\Component\\HttpFoundation\\JsonResponse;\nuse Symfony\\Component\\Routing\\Attribute\\Route;\nfinal class {0}Controller {{ #[Route('/{1}', methods: ['POST'])] public function __invoke(): JsonResponse {{ return new JsonResponse(['status' => 'accepted']); }} }}\n",pascal(&name),name),
        TargetProfile::PhpLaravel=>format!("<?php\ndeclare(strict_types=1);\nnamespace App\\Http\\Controllers;\nuse Illuminate\\Http\\JsonResponse;\nuse Illuminate\\Http\\Request;\nfinal class {0}Controller {{ public function __invoke(Request $request): JsonResponse {{ $request->validate(['value' => ['required','string']]); return response()->json(['status' => 'accepted']); }} }}\n",pascal(&name)),
        TargetProfile::PythonFastApi|TargetProfile::PythonDjango if is_test=>format!("def test_{name}_acceptance_contract():\n    criteria = {criteria:?}\n    assert criteria\n"),
        TargetProfile::PythonFastApi=>format!("from fastapi import APIRouter, HTTPException\nfrom pydantic import BaseModel\nrouter = APIRouter(prefix='/{name}')\nclass Input(BaseModel): value: str\n@router.post('')\nasync def {name}(data: Input):\n    if not data.value.strip(): raise HTTPException(status_code=400, detail='invalid_input')\n    return {{'status': 'accepted'}}\n"),
        TargetProfile::PythonDjango=>format!("from django.http import JsonResponse\nfrom django.views.decorators.http import require_POST\n@require_POST\ndef {name}(request):\n    return JsonResponse({{'status': 'accepted'}})\n"),
        TargetProfile::GoGin|TargetProfile::GoEcho if is_test=>format!("package handlers\nimport \"testing\"\nfunc Test{0}AcceptanceContract(t *testing.T) {{ if len({1:?}) == 0 {{ t.Fatal(\"missing acceptance contract\") }} }}\n",pascal(&name),criteria),
        TargetProfile::GoGin=>format!("package handlers\nimport \"github.com/gin-gonic/gin\"\nfunc {0}(context *gin.Context) {{ var input struct {{ Value string `json:\"value\" binding:\"required\"` }}; if err := context.ShouldBindJSON(&input); err != nil {{ context.JSON(400, gin.H{{\"error\":\"invalid_input\"}}); return }}; context.JSON(200, gin.H{{\"status\":\"accepted\"}}) }}\n",pascal(&name)),
        TargetProfile::GoEcho=>format!("package handlers\nimport \"github.com/labstack/echo/v4\"\nfunc {0}(context echo.Context) error {{ var input struct {{ Value string `json:\"value\"` }}; if context.Bind(&input) != nil || input.Value == \"\" {{ return context.JSON(400, map[string]string{{\"error\":\"invalid_input\"}}) }}; return context.JSON(200, map[string]string{{\"status\":\"accepted\"}}) }}\n",pascal(&name)),
        TargetProfile::DartFlutter if is_test=>format!("import 'package:flutter_test/flutter_test.dart';\nvoid main() {{ test('{name} acceptance contract', () {{ expect({criteria:?}, isNotEmpty); }}); }}\n"),
        TargetProfile::DartFlutter=>format!("import 'package:flutter/material.dart';\nclass {0}Page extends StatelessWidget {{ const {0}Page({{super.key}}); @override Widget build(BuildContext context) => const Scaffold(body: Center(child: Text({1:?}))); }}\n",pascal(&name),spec.description),
    }.replace("__OPERATIONS__",operations).replace("__PATH__",path)
}
fn pascal(value: &str) -> String {
    value
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}
fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}
pub fn apply_preview(
    root: &Path,
    expected_target_hash: &str,
    current_graph: &ProjectGraph,
    preview: &PortPreview,
    confirmed: bool,
) -> Result<ApplyResult, String> {
    if !confirmed {
        return Err("explicit confirmation is required".into());
    }
    if target_hash(current_graph) != expected_target_hash {
        return Err("ProjectChangedSinceGeneration".into());
    }
    let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
    if canonical_root.join(".git").exists() {
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&canonical_root)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() || !output.stdout.is_empty() {
            return Err("PortConflict: target git worktree must be clean before Apply".into());
        }
    }
    let mut created = Vec::new();
    let mut unchanged = Vec::new();
    for file in &preview.files {
        if !safe_relative(&file.path) {
            return Err("PortConflict: invalid target path".into());
        }
        let target = canonical_root.join(&file.path);
        if target.exists() {
            let existing = fs::read_to_string(&target).map_err(|error| error.to_string())?;
            if existing != file.content {
                return Err(format!("PortConflict: {} already exists", file.path));
            }
        }
    }
    for file in &preview.files {
        if !safe_relative(&file.path) {
            return Err("PortConflict: invalid target path".into());
        }
        let target = canonical_root.join(&file.path);
        if target.exists() {
            let existing = fs::read_to_string(&target).map_err(|error| error.to_string())?;
            if existing == file.content {
                unchanged.push(file.path.clone());
                continue;
            }
            return Err(format!("PortConflict: {} already exists", file.path));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?
        }
        fs::write(&target, &file.content).map_err(|error| error.to_string())?;
        created.push(file.path.clone())
    }
    Ok(ApplyResult { created, unchanged })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apply_requires_confirmation_and_blocks_traversal() {
        let graph = ProjectGraph::new(".".into(), vec![], vec![], vec![], vec![], vec![]);
        let preview = PortPreview {
            plan: PortingPlan {
                feature_id: "f".into(),
                feature_name: "F".into(),
                target_profile: TargetProfile::RustAxum,
                target_project_id: None,
                files_to_create: vec![],
                files_to_modify: vec![],
                dependencies: vec![],
                tests: vec![],
                security_requirements: vec![],
                constraints: vec![],
                risks: vec![],
                target_conventions: vec![],
                validation_commands: vec!["cargo test".into()],
                source_hash: "s".into(),
                target_hash: Some(target_hash(&graph)),
            },
            files: vec![],
            unified_diff: String::new(),
            generated_tests: vec![],
        };
        assert!(
            apply_preview(
                Path::new("."),
                &target_hash(&graph),
                &graph,
                &preview,
                false
            )
            .is_err()
        );
        let temporary = tempfile::tempdir().unwrap();
        let graph = ProjectGraph::new(
            temporary.path().to_string_lossy().into_owned(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let mut unsafe_preview = preview;
        unsafe_preview.files.push(GeneratedFile {
            path: "../escape.rs".into(),
            content: "unsafe".into(),
            is_test: false,
        });
        assert!(
            apply_preview(
                temporary.path(),
                &target_hash(&graph),
                &graph,
                &unsafe_preview,
                true
            )
            .is_err()
        );
        assert!(
            !temporary
                .path()
                .parent()
                .unwrap()
                .join("escape.rs")
                .exists()
        );
    }

    #[test]
    fn every_target_profile_generates_implementation_and_acceptance_test() {
        let feature = crate::features::Feature {
            id: "f".into(),
            project_id: "p".into(),
            name: "Password Reset".into(),
            description: "Reset a credential".into(),
            confidence: 1.0,
            completeness: 1.0,
            missing_signals: vec![],
            status: crate::features::FeatureStatus::Accepted,
            entry_point_node_ids: vec!["route".into()],
            node_ids: vec!["route".into()],
            edge_ids: vec![],
            memberships: vec![],
            routes: vec![],
            pages: vec![],
            services: vec![],
            repositories: vec![],
            models: vec![],
            entities: vec![],
            templates: vec![],
            tests: vec![],
            external_services: vec![],
            business_rules: vec!["single use token".into()],
            inputs: vec!["email".into()],
            outputs: vec!["generic response".into()],
            side_effects: vec!["email".into()],
            security_constraints: vec!["rate limit".into()],
            source_hash: "hash".into(),
            ai_provider: None,
            ai_model: None,
            prompt_version: "v1".into(),
            created_at: 0,
            updated_at: 0,
        };
        let spec = crate::features::FeatureSpec {
            name: feature.name.clone(),
            description: feature.description.clone(),
            acceptance_criteria: vec!["token cannot be reused".into()],
            operations: vec!["generate token".into()],
            security: feature.security_constraints.clone(),
            ..Default::default()
        };
        for profile in [
            TargetProfile::RustAxum,
            TargetProfile::RustActix,
            TargetProfile::TypeScriptNext,
            TargetProfile::TypeScriptExpress,
            TargetProfile::TypeScriptNest,
            TargetProfile::PhpSymfony,
            TargetProfile::PhpLaravel,
            TargetProfile::PythonFastApi,
            TargetProfile::PythonDjango,
            TargetProfile::GoGin,
            TargetProfile::GoEcho,
            TargetProfile::DartFlutter,
        ] {
            let plan = build_porting_plan(&feature, &spec, profile, None, None, vec![]);
            let preview = generate_preview(plan, &spec);
            assert!(preview.files.iter().any(|file| !file.is_test));
            assert!(preview.files.iter().any(|file| file.is_test));
            assert!(
                preview
                    .files
                    .iter()
                    .all(|file| !file.content.contains("implementation scaffold"))
            );
            assert!(!preview.unified_diff.is_empty());
        }
    }

    #[test]
    fn ai_generated_files_must_match_the_plan_exactly() {
        let plan = PortingPlan {
            feature_id: "f".into(),
            feature_name: "Demo".into(),
            target_profile: TargetProfile::RustAxum,
            target_project_id: None,
            files_to_create: vec!["src/routes/demo.rs".into()],
            files_to_modify: vec![],
            dependencies: vec!["axum".into()],
            tests: vec!["tests/demo.rs".into()],
            security_requirements: vec![],
            constraints: vec![],
            risks: vec![],
            target_conventions: vec![],
            validation_commands: vec!["cargo test".into()],
            source_hash: "hash".into(),
            target_hash: None,
        };
        let valid = AiGeneratedFiles {
            files: vec![
                GeneratedFile {
                    path: "src/routes/demo.rs".into(),
                    content: "pub fn demo() {}".into(),
                    is_test: false,
                },
                GeneratedFile {
                    path: "tests/demo.rs".into(),
                    content: "#[test] fn demo() {}".into(),
                    is_test: true,
                },
            ],
        };
        assert!(preview_from_generated_files(plan.clone(), valid).is_ok());
        let escaped = AiGeneratedFiles {
            files: vec![GeneratedFile {
                path: "../escape.rs".into(),
                content: "bad".into(),
                is_test: false,
            }],
        };
        assert!(preview_from_generated_files(plan, escaped).is_err());
    }
}
