use async_trait::async_trait;
use code_atlas::{
    ProjectAnalyzer,
    ai::{
        AiService,
        context_builder::{MAX_CONTEXT_BYTES, build_context},
        provider::{AiProvider, AiProviderConfig, AiProviderKind},
    },
    graph::{
        architecture::{ArchitectureRole, classify_architecture_roles},
        impact::analyze_impact,
        project_graph::ProjectGraph,
        project_map::build_project_map,
        search::{SearchQuery, search},
    },
    library::{FunctionDocumentation, LibraryEntry, candidate_for},
    model::{
        edge::{CodeEdge, RelationKind},
        node::{CodeNode, NodeKind},
        project::SourceScope,
    },
    storage::Repository,
};
use std::{path::PathBuf, sync::Arc};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mixed")
}

fn architecture_node(id: &str, kind: NodeKind, name: &str, path: Option<&str>) -> CodeNode {
    CodeNode {
        id: id.into(),
        kind,
        name: name.into(),
        path: path.map(str::to_string),
        language: None,
        start_line: None,
        end_line: None,
        owner: None,
        source_scope: SourceScope::Project,
    }
}

#[test]
fn ai_context_has_a_hard_budget_even_for_minified_single_line_sources() {
    let temporary = tempfile::tempdir().expect("temporary minified project");
    let path = temporary.path().join("bundle.js");
    std::fs::write(
        &path,
        format!("function bundled(){{return '{}';}}", "x".repeat(1_000_000)),
    )
    .expect("large source");
    let node = CodeNode {
        id: "bundled".into(),
        kind: NodeKind::Function,
        name: "bundled".into(),
        path: Some("bundle.js".into()),
        language: None,
        start_line: Some(1),
        end_line: Some(1),
        owner: None,
        source_scope: SourceScope::Project,
    };
    let graph = ProjectGraph::new(
        temporary.path().to_string_lossy().into_owned(),
        vec![node],
        vec![],
        vec![],
        vec![],
        vec![],
    );
    let context = build_context(&graph, "bundled", Some("bundled"));
    assert!(context.text.len() <= MAX_CONTEXT_BYTES);
    assert!(context.text.contains("[line truncated]"));
    assert!(context.text.len() < 5_000);
}

#[test]
fn generic_architecture_projects_real_flows_and_groups_low_level_noise() {
    let mut nodes = vec![
        architecture_node("project", NodeKind::Project, "Example", None),
        architecture_node(
            "api-entry",
            NodeKind::EntryPoint,
            "API",
            Some("src/main.rs"),
        ),
        architecture_node(
            "worker-entry",
            NodeKind::EntryPoint,
            "Worker",
            Some("src/bin/worker.rs"),
        ),
        architecture_node(
            "handler",
            NodeKind::Struct,
            "UserHandler",
            Some("src/api/user_handler.rs"),
        ),
        architecture_node(
            "service",
            NodeKind::Class,
            "UserService",
            Some("src/services/user_service.ts"),
        ),
        architecture_node(
            "repository",
            NodeKind::Class,
            "UserRepository",
            Some("app/repositories/user_repository.py"),
        ),
        architecture_node("state", NodeKind::Struct, "State", Some("src/state.rs")),
    ];
    for index in 0..300 {
        nodes.push(architecture_node(
            &format!("function-{index}"),
            NodeKind::Function,
            &format!("primitive_{index}"),
            Some("src/generated_like_helpers.rs"),
        ));
    }
    let edges = vec![
        CodeEdge::new("api-entry".into(), "handler".into(), RelationKind::RoutesTo),
        CodeEdge::new("handler".into(), "service".into(), RelationKind::Calls),
        CodeEdge::new("service".into(), "repository".into(), RelationKind::Calls),
    ];
    let graph = ProjectGraph::new(".".into(), nodes, edges, vec![], vec![], vec![]);
    let roles = classify_architecture_roles(&graph);
    assert_eq!(roles["handler"].role, ArchitectureRole::Handler);
    assert_eq!(roles["service"].role, ArchitectureRole::Service);
    assert_eq!(roles["repository"].role, ArchitectureRole::Repository);
    assert!(roles["handler"].confidence >= 60);

    let map = build_project_map(&graph);
    for zone in [
        "ENTRY POINTS",
        "API / ROUTING",
        "APPLICATION",
        "PERSISTENCE",
    ] {
        assert!(map.zones.iter().any(|value| value == zone), "{zone}");
    }
    assert!(map.entry_points >= 2);
    assert!(
        map.nodes.len() < 40,
        "projection contains {} nodes",
        map.nodes.len()
    );
    assert!(map.nodes.iter().all(|node| node.kind != NodeKind::Function));
    assert!(map.edges.iter().any(|edge| edge.source_id == "handler"
        && edge.target_id == "service"
        && edge.relation == RelationKind::Uses));
    assert!(map.edges.iter().any(|edge| edge.source_id == "service"
        && edge.target_id == "repository"
        && edge.relation == RelationKind::Uses));
}

#[test]
fn analyzes_all_reference_languages_and_frameworks() {
    let result = ProjectAnalyzer
        .analyze(fixture())
        .expect("fixture analysis");
    for language in ["Rust", "TypeScript", "PHP", "Dart", "Python", "Go"] {
        assert!(
            result.scan.language_counts.contains_key(language),
            "missing {language}"
        );
    }
    for kind in [
        NodeKind::Struct,
        NodeKind::Trait,
        NodeKind::Class,
        NodeKind::Interface,
        NodeKind::Component,
        NodeKind::Page,
        NodeKind::ApiEndpoint,
        NodeKind::Provider,
        NodeKind::DatabaseTable,
    ] {
        assert!(
            result.graph.nodes.iter().any(|node| node.kind == kind),
            "missing {kind:?}"
        );
    }
    assert!(
        result
            .graph
            .edges
            .iter()
            .any(|edge| edge.relation == RelationKind::Implements)
    );
    assert!(
        result
            .graph
            .edges
            .iter()
            .any(|edge| edge.relation == RelationKind::HandledBy)
    );
    assert!(
        result
            .graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::EntryPoint)
    );
}

#[test]
fn mixed_stack_adapters_enrich_the_same_generic_projection() {
    let result = ProjectAnalyzer.analyze(fixture()).expect("mixed analysis");
    let map = build_project_map(&result.graph);
    for zone in [
        "ENTRY POINTS",
        "UI / FRONTEND",
        "API / ROUTING",
        "APPLICATION",
    ] {
        assert!(map.zones.iter().any(|value| value == zone), "{zone}");
    }
    assert!(map.nodes.iter().any(|node| {
        node.kind == NodeKind::Page && node.path.as_deref() == Some("web/app/users/page.tsx")
    }));
    assert!(map.nodes.iter().any(|node| {
        node.kind == NodeKind::ApiEndpoint
            && node.path.as_deref() == Some("web/app/api/users/route.ts")
    }));
    assert!(map.nodes.iter().any(|node| {
        node.kind == NodeKind::Route && node.path.as_deref() == Some("flutter/main.dart")
    }));
    assert!(map.nodes.iter().any(|node| {
        node.kind == NodeKind::EntryPoint
            && node
                .path
                .as_deref()
                .is_some_and(|path| path.ends_with("src/main.rs"))
    }));
}

#[test]
fn graph_search_impact_storage_and_incremental_cache_work() {
    let analyzer = ProjectAnalyzer;
    let first = analyzer.analyze(fixture()).expect("first analysis");
    let second = analyzer
        .analyze_incremental(fixture(), Some(&first))
        .expect("incremental analysis");
    assert_eq!(second.analyzed_files, 0);
    assert_eq!(second.reused_files, first.analyzed_files);
    let hits = search(
        &first.graph,
        &SearchQuery {
            q: Some("login".into()),
            ..Default::default()
        },
    );
    assert!(!hits.is_empty());
    let verify = first
        .graph
        .nodes
        .iter()
        .find(|node| node.name == "verify_password")
        .expect("verify_password");
    let impact = analyze_impact(&first.graph, &verify.id, 3).expect("impact");
    assert!(!impact.direct.is_empty());
    let repository = Repository::in_memory().expect("sqlite");
    repository.save("fixture", &first).expect("save");
    let loaded = repository.load("fixture").expect("load").expect("graph");
    assert_eq!(loaded.nodes.len(), first.graph.nodes.len());
    assert!(loaded.find_node(&verify.id).is_some());
}

struct MockProvider;
#[async_trait]
impl AiProvider for MockProvider {
    async fn answer(&self, _: &str, input: &str) -> anyhow::Result<String> {
        assert!(input.contains("SOURCE"));
        Ok("Grounded answer".into())
    }
}

#[test]
fn frontend_ai_configuration_rejects_missing_secrets() {
    let result = AiProviderConfig {
        provider: AiProviderKind::Mistral,
        api_key: "  ".into(),
        model: "mistral-large-latest".into(),
    }
    .build();
    assert!(result.is_err());
}

#[test]
fn source_scope_classification_controls_library_eligibility() {
    use code_atlas::model::project::SourceScope;
    assert_eq!(SourceScope::classify("src/auth.rs"), SourceScope::Project);
    assert_eq!(
        SourceScope::classify("packages/auth/src/index.ts"),
        SourceScope::WorkspacePackage
    );
    assert_eq!(
        SourceScope::classify("node_modules/pkg/index.js"),
        SourceScope::ExternalDependency
    );
    assert_eq!(
        SourceScope::classify("lib/models/user.g.dart"),
        SourceScope::Generated
    );
    assert!(SourceScope::WorkspacePackage.library_eligible());
    assert!(!SourceScope::ExternalDependency.library_eligible());
}

#[test]
fn progress_entry_points_and_architecture_map_are_real() {
    let mut updates = Vec::new();
    let result = ProjectAnalyzer
        .analyze_incremental_with_progress(fixture(), None, |update| updates.push(update))
        .expect("analysis with progress");
    for stage in [
        "scanning",
        "parsing",
        "building_nodes",
        "resolving_imports",
        "resolving_calls",
        "framework_analysis",
        "building_graph",
    ] {
        assert!(
            updates.iter().any(|update| update.stage == stage),
            "{stage}"
        );
    }
    let parsing = updates
        .iter()
        .filter(|update| update.stage == "parsing")
        .collect::<Vec<_>>();
    assert_eq!(
        parsing.last().expect("parsing update").processed,
        result.scan.total_files
    );
    assert!(parsing.iter().any(|update| update.nodes > 0));
    let map = build_project_map(&result.graph);
    assert!(map.entry_points >= 2);
    assert!(
        map.nodes
            .iter()
            .all(|node| !matches!(node.kind, NodeKind::Method | NodeKind::Constructor))
    );
    assert!(
        map.nodes
            .iter()
            .any(|node| node.kind == NodeKind::ArchitectureZone)
    );
}

#[test]
fn incremental_add_change_delete_and_registry_reload_work() {
    let temporary = tempfile::tempdir().expect("temporary project");
    let source = temporary.path().join("src");
    std::fs::create_dir_all(&source).expect("src");
    std::fs::write(
        source.join("main.rs"),
        "fn main() { helper(); }\nfn helper() {}\n",
    )
    .expect("main");
    let analyzer = ProjectAnalyzer;
    let first = analyzer.analyze(temporary.path()).expect("first");
    let second = analyzer
        .analyze_incremental(temporary.path(), Some(&first))
        .expect("unchanged");
    assert_eq!(second.analyzed_files, 0);
    std::fs::write(
        source.join("main.rs"),
        "fn main() { helper(); }\nfn helper() { work(); }\n",
    )
    .expect("change");
    let changed = analyzer
        .analyze_incremental(temporary.path(), Some(&second))
        .expect("changed");
    assert_eq!(changed.analyzed_files, 1);
    std::fs::write(source.join("extra.rs"), "fn extra() {}\n").expect("new");
    let added = analyzer
        .analyze_incremental(temporary.path(), Some(&changed))
        .expect("added");
    assert_eq!(added.analyzed_files, 1);
    std::fs::remove_file(source.join("extra.rs")).expect("delete");
    let deleted = analyzer
        .analyze_incremental(temporary.path(), Some(&added))
        .expect("deleted");
    assert!(
        deleted
            .graph
            .nodes
            .iter()
            .all(|node| node.path.as_deref() != Some("src/extra.rs"))
    );

    let repository = Repository::in_memory().expect("sqlite");
    repository
        .save_with_duration("incremental", &deleted, 42)
        .expect("save");
    let summary = repository.list().expect("list");
    assert_eq!(summary[0].file_count, 1);
    let snapshot = repository
        .load_analysis("incremental")
        .expect("load")
        .expect("snapshot");
    let reopened = analyzer
        .analyze_incremental(temporary.path(), Some(&snapshot))
        .expect("reopen incrementally");
    assert_eq!(reopened.analyzed_files, 0);
    assert!(
        repository
            .delete_project("incremental")
            .expect("delete project")
    );
    assert!(repository.list().expect("empty").is_empty());
}

#[test]
fn function_library_preserves_provenance_and_rejects_external_code() {
    use code_atlas::model::project::SourceScope;
    let result = ProjectAnalyzer.analyze(fixture()).expect("analysis");
    let node = result
        .graph
        .nodes
        .iter()
        .find(|node| node.name == "verify_password")
        .expect("function");
    let candidate = candidate_for(&result.graph, node).expect("candidate");
    assert!(!candidate.real_usages.is_empty());
    let mut external = node.clone();
    external.source_scope = SourceScope::ExternalDependency;
    assert!(candidate_for(&result.graph, &external).is_none());
    let hash = result
        .scan
        .files
        .iter()
        .find(|file| file.path == node.path.as_deref().expect("path"))
        .expect("file")
        .hash
        .clone();
    let entry = LibraryEntry {
        id: "library:test".into(),
        display_name: node.name.clone(),
        language: "Rust".into(),
        category: "security".into(),
        tags: vec!["auth".into()],
        description: "Password verification".into(),
        source_project_id: "fixture".into(),
        source_project_name: "mixed".into(),
        source_node_id: node.id.clone(),
        source_path: node.path.clone().expect("path"),
        start_line: node.start_line,
        end_line: node.end_line,
        source_hash: hash.clone(),
        source_scope: node.source_scope.as_str().into(),
        source_repository: Some(result.scan.root.clone()),
        source_version: None,
        source_branch: None,
        source_license: None,
        recipe_id: None,
        reuse_score: candidate.reuse_score,
        knowledge_value: candidate.knowledge_value,
        real_usages: candidate.real_usages,
        calls: candidate.calls,
        dependencies: candidate.dependencies,
        tests: candidate.tests,
        source_code: candidate.source,
        documentation_status: Default::default(),
        documentation_provider: None,
        documentation_model: None,
        documentation_generated_at: None,
        documentation_error: None,
        documentation: None,
        created_at: 1,
        updated_at: 1,
    };
    let repository = Repository::in_memory().expect("sqlite");
    repository.save("fixture", &result).expect("project");
    repository.save_library_entry(&entry).expect("library save");
    let loaded = repository
        .list_library(Some("password"))
        .expect("search library");
    assert_eq!(loaded[0].source_hash, hash);
    let documentation = FunctionDocumentation {
        summary: "Verifies a supplied password.".into(),
        ..Default::default()
    };
    let documented = repository
        .save_library_documentation("library:test", documentation, "Mock", "mock-model")
        .expect("documentation")
        .expect("entry");
    assert!(documented.documentation.is_some());
    assert!(
        repository
            .delete_library_entry("library:test")
            .expect("delete")
    );
}

#[test]
fn library_filters_ast_bounded_boilerplate_and_scores_meaningful_logic() {
    use code_atlas::model::project::SourceScope;
    let temporary = tempfile::tempdir().expect("temporary PHP project");
    let source = temporary.path().join("src/Service");
    std::fs::create_dir_all(&source).expect("service directory");
    std::fs::write(
        source.join("ValueTools.php"),
        r#"<?php
class ValueTools {
    public function getTitle(): string { return $this->title; }
    public function setTitle(string $title): void { $this->title = $title; }
    public function __construct(string $title) { $this->title = $title; }
    public function validateEmail(string $email): string {
        $normalized = strtolower(trim($email));
        if (!filter_var($normalized, FILTER_VALIDATE_EMAIL)) {
            throw new InvalidArgumentException('invalid email');
        }
        return $normalized;
    }
    public function transformRows(array $rows): array {
        $result = [];
        foreach ($rows as $row) {
            if (isset($row['name'])) { $result[] = trim($row['name']); }
        }
        return array_values(array_unique($result));
    }
}
"#,
    )
    .expect("PHP source");
    let result = ProjectAnalyzer
        .analyze(temporary.path())
        .expect("PHP analysis");
    for name in ["getTitle", "setTitle", "__construct"] {
        let node = result
            .graph
            .nodes
            .iter()
            .find(|node| node.name == name)
            .expect("boilerplate node");
        assert!(candidate_for(&result.graph, node).is_none(), "{name}");
    }
    for name in ["validateEmail", "transformRows"] {
        let node = result
            .graph
            .nodes
            .iter()
            .find(|node| node.name == name)
            .expect("meaningful node");
        let candidate = candidate_for(&result.graph, node).expect("meaningful candidate");
        assert!(candidate.knowledge_value >= 45, "{name}");
        assert!(candidate.source.contains(&format!("function {name}")));
        assert!(!candidate.signature.is_empty());
        assert!(candidate.loc >= 4);
        let mut generated = node.clone();
        generated.source_scope = SourceScope::Generated;
        assert!(candidate_for(&result.graph, &generated).is_none());
    }
}

#[test]
fn library_filters_accessors_across_rust_typescript_and_python() {
    let temporary = tempfile::tempdir().expect("temporary multilingual project");
    std::fs::create_dir_all(temporary.path().join("src")).expect("src");
    std::fs::write(
        temporary.path().join("src/value.rs"),
        r#"pub struct Value { title: String }
impl Value {
    pub fn title(&self) -> &str { &self.title }
    pub fn normalize_tags(&self, tags: Vec<String>) -> Vec<String> {
        tags.into_iter().map(|tag| tag.trim().to_lowercase()).filter(|tag| !tag.is_empty()).collect()
    }
}"#,
    )
    .expect("Rust source");
    std::fs::write(
        temporary.path().join("src/value.ts"),
        r#"export class ValueTools {
  getTitle(): string { return this.title; }
  normalizeTags(tags: string[]): string[] {
    return tags.map(tag => tag.trim().toLowerCase()).filter(Boolean);
  }
}"#,
    )
    .expect("TypeScript source");
    std::fs::write(
        temporary.path().join("src/value.py"),
        r#"class ValueTools:
    def get_title(self):
        return self.title

    def normalize_tags(self, tags):
        return sorted({tag.strip().lower() for tag in tags if tag.strip()})
"#,
    )
    .expect("Python source");

    let result = ProjectAnalyzer
        .analyze(temporary.path())
        .expect("multilingual analysis");
    for name in ["title", "getTitle", "get_title"] {
        let node = result
            .graph
            .nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing accessor {name}"));
        assert!(candidate_for(&result.graph, node).is_none(), "{name}");
    }
    for name in ["normalize_tags", "normalizeTags"] {
        let candidates = result
            .graph
            .nodes
            .iter()
            .filter(|node| node.name == name)
            .filter_map(|node| candidate_for(&result.graph, node))
            .collect::<Vec<_>>();
        assert!(!candidates.is_empty(), "{name}");
        assert!(
            candidates
                .iter()
                .all(|candidate| !candidate.source.is_empty())
        );
    }
}

#[test]
fn symfony_map_groups_routes_roles_and_multiple_entry_points() {
    let temporary = tempfile::tempdir().expect("temporary Symfony project");
    let root = temporary.path();
    for directory in [
        "src/Controller",
        "src/Service",
        "src/Repository",
        "src/Entity",
        "src/Command",
        "src/Message",
        "src/MessageHandler",
        "src/Event",
        "src/EventSubscriber",
        "templates/user",
        "config/routes",
    ] {
        std::fs::create_dir_all(root.join(directory)).expect("Symfony directory");
    }
    std::fs::write(
        root.join("composer.json"),
        r#"{"require":{"symfony/framework-bundle":"^7.0"}}"#,
    )
    .expect("composer");
    std::fs::write(root.join("symfony.lock"), "{}").expect("lock");
    std::fs::write(root.join("templates/user/show.html.twig"), "<h1>User</h1>").expect("template");
    std::fs::write(
        root.join("src/Controller/UserController.php"),
        r#"<?php
namespace App\Controller;
use App\Service\UserService;
class UserController {
    #[Route('/users', methods: ['GET'])]
    public function list(): Response {
        $this->service->loadUsers();
        return $this->render('user/show.html.twig');
    }
}"#,
    )
    .expect("controller");
    std::fs::write(
        root.join("src/Service/UserService.php"),
        "<?php namespace App\\Service; class UserService { public function loadUsers() { return saveUsers(); } }",
    )
    .expect("service");
    std::fs::write(
        root.join("src/Repository/UserRepository.php"),
        "<?php namespace App\\Repository; use App\\Entity\\User; class UserRepository { public function saveUsers() {} }",
    )
    .expect("repository");
    std::fs::write(
        root.join("src/Entity/User.php"),
        "<?php namespace App\\Entity; class User {}",
    )
    .expect("entity");
    std::fs::write(
        root.join("src/Command/SyncUsersCommand.php"),
        "<?php #[AsCommand(name: 'app:sync')] class SyncUsersCommand { protected function execute() {} }",
    )
    .expect("command");
    std::fs::write(
        root.join("src/Message/RefreshUsers.php"),
        "<?php class RefreshUsers {}",
    )
    .expect("message");
    std::fs::write(
        root.join("src/MessageHandler/RefreshUsersHandler.php"),
        "<?php #[AsMessageHandler] class RefreshUsersHandler { public function __invoke(RefreshUsers $message) {} }",
    )
    .expect("message handler");
    std::fs::write(
        root.join("src/Event/UserChanged.php"),
        "<?php class UserChanged {}",
    )
    .expect("event");
    std::fs::write(
        root.join("src/EventSubscriber/UserSubscriber.php"),
        "<?php class UserSubscriber implements EventSubscriberInterface { public static function getSubscribedEvents() { return [UserChanged::class => 'changed']; } }",
    )
    .expect("subscriber");
    std::fs::write(
        root.join("config/routes/users.yaml"),
        "user_show:\n  path: /users/{id}\n  controller: App\\Controller\\UserController::list\n  methods: [GET]\n",
    )
    .expect("YAML route");

    let result = ProjectAnalyzer.analyze(root).expect("Symfony analysis");
    let map = build_project_map(&result.graph);
    for kind in [
        NodeKind::Controller,
        NodeKind::Service,
        NodeKind::Repository,
        NodeKind::Entity,
        NodeKind::Command,
        NodeKind::MessageHandler,
        NodeKind::Template,
    ] {
        assert!(map.nodes.iter().any(|node| node.kind == kind), "{kind:?}");
    }
    assert!(map.entry_points >= 3);
    assert!(map.nodes.iter().all(|node| node.kind != NodeKind::Method));
    assert!(map.zones.contains(&"API / ROUTING".to_string()));
    assert!(map.zones.contains(&"APPLICATION".to_string()));
    assert!(map.zones.contains(&"PERSISTENCE".to_string()));
    let controller = map
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Controller)
        .expect("controller");
    assert!(map.edges.iter().any(|edge| {
        edge.source_id == controller.id
            && edge.relation == RelationKind::Contains
            && map
                .nodes
                .iter()
                .any(|node| node.id == edge.target_id && node.kind == NodeKind::ApiEndpoint)
    }));
    assert!(map.connected_components <= 2);
}
#[tokio::test]
async fn ai_context_is_grounded_without_a_real_key() {
    let result = ProjectAnalyzer.analyze(fixture()).expect("analysis");
    let node = result
        .graph
        .nodes
        .iter()
        .find(|node| node.name == "login")
        .expect("login");
    let answer = AiService::new(Arc::new(MockProvider))
        .query(&result.graph, "How does login work?", Some(&node.id))
        .await
        .expect("answer");
    assert_eq!(answer.answer, "Grounded answer");
    assert!(
        answer
            .citations
            .iter()
            .any(|citation| citation.contains("auth.rs"))
    );
}

#[tokio::test]
async fn api_health_and_real_analysis_endpoints_work() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use code_atlas::api::{AppState, router};
    use tower::ServiceExt;
    let app = router(AppState::new(
        Repository::in_memory().expect("sqlite"),
        None,
    ));
    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(health.status(), StatusCode::OK);
    let directory = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/fs/directories?path={}",
                    fixture().to_string_lossy()
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(directory.status(), StatusCode::OK);
    let payload = serde_json::json!({"path":fixture(),"watch":false}).to_string();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects/analyze")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("analysis body");
    let analyzed: serde_json::Value = serde_json::from_slice(&body).expect("analysis json");
    let project_id = analyzed["id"].as_str().expect("project id");
    let map = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/map"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(map.status(), StatusCode::OK);
    let view = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/view?level=Symbols&limit=80"
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("view");
    assert_eq!(view.status(), StatusCode::OK);
    let view_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(view.into_body(), usize::MAX)
            .await
            .expect("view body"),
    )
    .expect("view json");
    assert!(
        view_json["nodes"]
            .as_array()
            .is_some_and(|nodes| nodes.len() <= 80)
    );
    assert!(view_json["full_node_count"].as_u64().is_some());
    let my_code = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/my-code?kind=functions&limit=50"
                ))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("my code");
    assert_eq!(my_code.status(), StatusCode::OK);
    let my_code_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(my_code.into_body(), usize::MAX)
            .await
            .expect("body"),
    )
    .expect("json");
    assert!(
        my_code_json["items"]
            .as_array()
            .is_some_and(|items| items.iter().all(|item| matches!(
                item["node"]["source_scope"].as_str(),
                Some("Project" | "WorkspacePackage")
            )))
    );
    for endpoint in ["quality", "security", "docs"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/projects/{project_id}/{endpoint}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("knowledge endpoint");
        assert_eq!(response.status(), StatusCode::OK, "{endpoint}");
    }
    let context=app.clone().oneshot(Request::builder().method("POST").uri(format!("/api/projects/{project_id}/context")).header("content-type","application/json").body(Body::from(serde_json::json!({"intent":"prepare_task","task":"add login audit logging","max_tokens":5000}).to_string())).expect("request")).await.expect("context");
    assert_eq!(context.status(), StatusCode::OK);
    let context_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(context.into_body(), usize::MAX)
            .await
            .expect("body"),
    )
    .expect("json");
    assert!(
        context_json["token_estimate"]
            .as_u64()
            .is_some_and(|tokens| tokens <= 5000)
    );
    let detection = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/features/detect"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request"),
        )
        .await
        .expect("features");
    assert_eq!(detection.status(), StatusCode::OK);
    let detection_json: serde_json::Value = serde_json::from_slice(
        &to_bytes(detection.into_body(), usize::MAX)
            .await
            .expect("body"),
    )
    .expect("json");
    let features = detection_json["items"].as_array().expect("feature items");
    assert!(!features.is_empty());
    let feature_id = features[0]["id"].as_str().expect("feature id");
    for suffix in ["spec", "code", "graph", "docs"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/projects/{project_id}/features/{feature_id}/{suffix}"
                    ))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("feature endpoint");
        assert_eq!(response.status(), StatusCode::OK, "{suffix}");
    }
}
