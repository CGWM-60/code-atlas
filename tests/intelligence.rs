use code_atlas::{
    ProjectAnalyzer,
    assistant::{AgentOrchestrator, EvidenceVerifier, UiAction, UiContext},
    features::detect_feature_candidates,
    intelligence::{estimate, git_diff, test_plan},
    retrieval::{
        EMBEDDING_VERSION, EmbeddingProvider, HybridRetriever, LocalEmbedding, VectorStore,
        rebuild_project,
    },
    storage::Repository,
};
use std::{fs, process::Command};
fn setup() -> (tempfile::TempDir, Repository, code_atlas::AnalysisResult) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("auth.ts"),"export function login(user: string) { return validate(user); }\nexport function validate(user: string) { return user.length > 0; }\nexport function sendEmail(address: string) { return address; }\n").unwrap();
    let analysis = ProjectAnalyzer.analyze(dir.path()).unwrap();
    let repo = Repository::in_memory().unwrap();
    repo.save("p", &analysis).unwrap();
    (dir, repo, analysis)
}
#[test]
fn vector_index_is_incremental_versioned_and_project_scoped() {
    let (_dir, repo, analysis) = setup();
    let first = rebuild_project(&repo, "p", &analysis.graph, &[], false).unwrap();
    assert!(first.updated > 0);
    let second = rebuild_project(&repo, "p", &analysis.graph, &[], false).unwrap();
    assert_eq!(second.updated, 0);
    assert_eq!(second.reused, first.total);
    let mut units = repo.semantic_units("p").unwrap();
    units[0].embedding_version = "obsolete".into();
    repo.upsert(&units[..1]).unwrap();
    assert_eq!(
        rebuild_project(&repo, "p", &analysis.graph, &[], false)
            .unwrap()
            .updated,
        1
    );
    assert!(
        repo.search_by_project("other", &LocalEmbedding.embed("login"), None, 10)
            .unwrap()
            .is_empty()
    );
    assert!(
        repo.semantic_units("p")
            .unwrap()
            .iter()
            .all(|u| u.embedding_version == EMBEDDING_VERSION)
    );
    let mut empty = analysis.graph.clone();
    empty.nodes.clear();
    empty.rebuild_indexes();
    assert_eq!(
        rebuild_project(&repo, "p", &empty, &[], false)
            .unwrap()
            .deleted,
        first.total
    );
}
#[test]
fn changed_source_reembeds_only_affected_units() {
    let (dir, repo, analysis) = setup();
    rebuild_project(&repo, "p", &analysis.graph, &[], false).unwrap();
    fs::write(dir.path().join("auth.ts"),"export function login(user: string) { return validate(user); }\nexport function validate(user: string) { return user.length > 3; }\nexport function sendEmail(address: string) { return address; }\n").unwrap();
    let next = ProjectAnalyzer.analyze(dir.path()).unwrap();
    repo.save("p", &next).unwrap();
    let stats = rebuild_project(&repo, "p", &next.graph, &[], false).unwrap();
    assert!(stats.updated > 0);
    assert!(stats.reused > 0);
}
#[test]
fn hybrid_search_understands_french_login_and_retains_symbol_ranking() {
    let (_dir, repo, analysis) = setup();
    let hits = HybridRetriever::search(
        &repo,
        "p",
        &analysis.graph,
        &[],
        "Montre-moi la connexion utilisateur",
        None,
        5,
    )
    .unwrap();
    assert!(hits.iter().any(|h| h.node.name == "login"));
    let exact =
        HybridRetriever::search(&repo, "p", &analysis.graph, &[], "sendEmail", None, 5).unwrap();
    assert_eq!(exact[0].node.name, "sendEmail");
    assert!(exact[0].score_lexical > 0.0);
    assert!(exact.len() <= 5);
}
#[tokio::test]
async fn assistant_is_grounded_multiturn_and_persistent() {
    let (dir, repo, analysis) = setup();
    let c = repo.new_conversation("p", "Authentication").unwrap();
    let answer = AgentOrchestrator::respond(
        &repo,
        "p",
        "Montre-moi la connexion",
        &UiContext::default(),
        &[],
        None,
    )
    .await
    .unwrap();
    assert!(!answer.citations.is_empty());
    assert!(
        answer
            .citations
            .iter()
            .all(|c| EvidenceVerifier::verify(&analysis.graph, c))
    );
    assert!(answer.context_tokens <= 12000);
    assert!(answer.entities.len() <= 24);
    assert!(answer.tool_calls.len() <= 10);
    repo.append_turn("p", &c.id, "connexion", &answer).unwrap();
    let history = repo.conversation_messages("p", &c.id).unwrap();
    assert_eq!(history.len(), 2);
    assert!(repo.conversation_messages("other", &c.id).is_err());
    let follow = AgentOrchestrator::respond(
        &repo,
        "p",
        "Quels tests manquent ?",
        &UiContext::default(),
        &history,
        None,
    )
    .await
    .unwrap();
    assert!(
        follow
            .actions
            .iter()
            .any(|a| matches!(a, UiAction::ShowTestPlan { .. }))
    );
    let citation = &answer.citations[0];
    fs::write(dir.path().join(&citation.path), "// changed").unwrap();
    assert!(!EvidenceVerifier::verify(&analysis.graph, citation));
    assert!(repo.rename_conversation("p", &c.id, "Renamed").unwrap());
    assert!(repo.delete_conversation("p", &c.id).unwrap());
    assert!(repo.conversation_messages("p", &c.id).is_err());
}
#[test]
fn estimates_and_test_plans_never_claim_measured_coverage() {
    let (_dir, repo, analysis) = setup();
    let features = detect_feature_candidates("p", &analysis.graph);
    let plan = test_plan(&analysis.graph, &features, None).unwrap();
    assert!(plan.coverage.is_none());
    assert!(test_plan(&analysis.graph, &features, Some("unknown")).is_err());
    let value = estimate(&repo, "p", &analysis.graph, &features, "Modifier login").unwrap();
    assert!(!value.affected_nodes.is_empty());
    assert!(
        value
            .breakdown
            .iter()
            .all(|r| r.minimum <= r.likely && r.likely <= r.maximum)
    );
    assert!(value.confidence < 0.5);
}
#[test]
fn git_diff_is_read_only_and_rejects_option_injection() {
    let (dir, _repo, analysis) = setup();
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["init"]);
    run(&["add", "."]);
    run(&[
        "-c",
        "user.name=Atlas test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "-m",
        "fixture",
    ]);
    fs::write(
        dir.path().join("auth.ts"),
        "export function login() { return false; }\n",
    )
    .unwrap();
    let before = fs::read(dir.path().join(".git/index")).unwrap();
    let diff = git_diff(&analysis.graph, &[], None, None).unwrap();
    assert_eq!(diff.files.len(), 1);
    assert_eq!(diff.files[0].path, "auth.ts");
    assert!(
        diff.files[0]
            .lines
            .iter()
            .any(|l| l.kind == "added" && l.new_line == Some(1))
    );
    assert_eq!(fs::read(dir.path().join(".git/index")).unwrap(), before);
    assert!(git_diff(&analysis.graph, &[], Some("--output=/tmp/no"), None).is_err());
}
#[test]
fn sqlite_upgrade_preserves_projects_and_conversations_after_restart() {
    let (dir, _, analysis) = setup();
    let db = dir.path().join("atlas.sqlite");
    {
        let repo = Repository::open(&db).unwrap();
        repo.save("p", &analysis).unwrap();
        repo.new_conversation("p", "Durable").unwrap();
        rebuild_project(&repo, "p", &analysis.graph, &[], false).unwrap();
    }
    let repo = Repository::open(&db).unwrap();
    assert!(repo.load("p").unwrap().is_some());
    assert_eq!(repo.conversations("p").unwrap().len(), 1);
    assert!(repo.stats("p").unwrap().total > 0);
    repo.delete_project("p").unwrap();
    assert!(repo.conversations("p").unwrap().is_empty());
    assert_eq!(repo.stats("p").unwrap().total, 0);
}
#[tokio::test]
async fn intelligence_http_routes_integrate_real_backend() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;
    let (_dir, repo, _analysis) = setup();
    let app = code_atlas::api::router(code_atlas::api::AppState::new(repo, None));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/projects/p/intelligence/search?q=connexion")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success());
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert!(!json["items"].as_array().unwrap().is_empty());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects/p/conversations")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success());
    let c: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/p/conversations/{}/messages",
                    c["id"].as_str().unwrap()
                ))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"question":"Montre la connexion"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success());
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(value["actions"][0]["type"], "OPEN_SOURCE_RANGE");
}

#[test]
fn next_routes_expose_every_observed_http_export_in_api_explorer() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("app/api/login")).unwrap();
    fs::write(dir.path().join("app/api/login/route.ts"),"export function GET() { return Response.json({ok:true}); }\nexport function POST() { return Response.json({ok:true}); }\n").unwrap();
    let analysis = ProjectAnalyzer.analyze(dir.path()).unwrap();
    let endpoints =
        code_atlas::api_explorer::discover_api_endpoints("p", &analysis.graph, &[], &[]);
    assert!(
        endpoints
            .iter()
            .any(|e| e.path == "/api/login" && e.method == "GET")
    );
    assert!(
        endpoints
            .iter()
            .any(|e| e.path == "/api/login" && e.method == "POST")
    );
    assert!(
        endpoints
            .iter()
            .all(|e| analysis.graph.find_node(&e.id).is_some())
    );
}

#[test]
fn documentation_cache_invalidates_on_body_only_edits() {
    let (dir, _repo, analysis) = setup();
    let before = code_atlas::documentation::project_content_hash(&analysis.graph);
    fs::write(
        dir.path().join("auth.ts"),
        "export function login(user: string) { return false; }\n",
    )
    .unwrap();
    assert_ne!(
        before,
        code_atlas::documentation::project_content_hash(&analysis.graph)
    );
}

#[tokio::test]
async fn provider_can_request_internal_tools_with_bounded_evidence() {
 use code_atlas::ai::{AiService,provider::{AiProvider,StructuredOutputRequest,AiDocumentationError}};
 use async_trait::async_trait;
 use serde_json::{json,Value};
 use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
 struct Provider { node: String, calls: AtomicUsize }
 #[async_trait] impl AiProvider for Provider {
  async fn answer(&self,_:&str,_:&str)->anyhow::Result<String>{unreachable!()}
  async fn generate_structured(&self,request:StructuredOutputRequest<'_>)->Result<Value,AiDocumentationError>{
   if request.purpose=="assistant_tools" {
    if self.calls.fetch_add(1,Ordering::SeqCst)==0{return Ok(json!({"done":false,"calls":[{"tool":"get_source","arguments_json":json!({"node_id":self.node}).to_string()}]}));}
    return Ok(json!({"done":true,"calls":[]}));
   }
   assert!(request.input.chars().count().div_ceil(4)<=12000);
   Ok(json!({"answer":"La source de login est disponible dans auth.ts.","citation_indices":[0]}))
  }
 }
 let(_dir,repo,analysis)=setup();let node=analysis.graph.nodes.iter().find(|n|n.name=="login").unwrap();
 let service=AiService::new(Arc::new(Provider{node:node.id.clone(),calls:AtomicUsize::new(0)}));
 let answer=AgentOrchestrator::respond(&repo,"p","Explique login",&UiContext::default(),&[],Some(&service)).await.unwrap();
 assert!(answer.tool_calls.iter().any(|t|t.tool=="get_source"&&t.status=="completed"));assert!(!answer.citations.is_empty());assert!(answer.uncertain);
}
