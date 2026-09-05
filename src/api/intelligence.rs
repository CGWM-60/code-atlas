use super::routes::{ApiError, ApiResult};
use crate::{
    ai::AiService,
    api::state::AppState,
    assistant::{AgentOrchestrator, UiContext},
    retrieval::{HybridRetriever, VectorStore},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{id}/evidence/verify", post(verify_evidence))
        .route("/api/projects/{id}/dependencies", get(dependencies))
        .route("/api/projects/{id}/intelligence/embeddings", post(neural_index))
        .route("/api/projects/{id}/intelligence/semantic-search", post(neural_search))
        .route("/api/assistant/tools", get(|| async { Json(json!({"tools": crate::assistant_tools::TOOLS.iter().map(|(name,_)|name).collect::<Vec<_>>()})) }))
        .route("/api/projects/{id}/source", get(source_file))
        .route("/api/projects/{id}/intelligence/search", get(search))
        .route(
            "/api/projects/{id}/intelligence/index",
            get(index_stats).post(reindex),
        )
        .route(
            "/api/projects/{id}/conversations",
            get(conversations).post(new_conversation),
        )
        .route(
            "/api/projects/{id}/conversations/{conversation}",
            get(messages).patch(rename).delete(delete),
        )
        .route(
            "/api/projects/{id}/conversations/{conversation}/messages",
            post(ask),
        )
        .route("/api/projects/{id}/tests/plan", get(test_plan))
        .route("/api/projects/{id}/estimates", post(estimate))
        .route("/api/projects/{id}/git/diff", get(diff))
}
#[derive(Deserialize)]
struct SearchInput {
    q: String,
    limit: Option<usize>,
}
async fn search(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(input): Query<SearchInput>,
) -> ApiResult<Json<Value>> {
    let repo = state.repository.clone();
    blocking(move||{let graph=repo.load(&id)?.ok_or_else(||anyhow::anyhow!("project not found"))?;let features=repo.list_features(&id)?;Ok(json!({"items":HybridRetriever::search(&repo,&id,&graph,&features,&input.q,None,input.limit.unwrap_or(20))?}))}).await
}
async fn blocking<F>(work: F) -> ApiResult<Json<Value>>
where
    F: FnOnce() -> anyhow::Result<Value> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(|e| ApiError(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))
}
async fn index_stats(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(state.repository.stats(&id)?)))
}
async fn reindex(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let repo = state.repository.clone();
    blocking(move || {
        let graph = repo
            .load(&id)?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        Ok(json!(crate::retrieval::rebuild_project(
            &repo,
            &id,
            &graph,
            &repo.list_features(&id)?,
            true
        )?))
    })
    .await
}
async fn conversations(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(state.repository.conversations(&id)?)))
}
#[derive(Deserialize)]
struct Title {
    title: String,
}
async fn new_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Title>,
) -> ApiResult<Json<Value>> {
    if state.repository.load(&id)?.is_none() {
        return Err(ApiError(StatusCode::NOT_FOUND, "Projet introuvable".into()));
    }
    Ok(Json(json!(
        state.repository.new_conversation(&id, &input.title)?
    )))
}
async fn messages(
    State(state): State<AppState>,
    Path((id, conversation)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(json!(
        state.repository.conversation_messages(&id, &conversation)?
    )))
}
async fn rename(
    State(state): State<AppState>,
    Path((id, conversation)): Path<(String, String)>,
    Json(input): Json<Title>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({"updated":state.repository.rename_conversation(&id,&conversation,&input.title)?}),
    ))
}
async fn delete(
    State(state): State<AppState>,
    Path((id, conversation)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({"deleted":state.repository.delete_conversation(&id,&conversation)?}),
    ))
}
#[derive(Deserialize)]
struct AskInput {
    question: String,
    #[serde(default)]
    context: UiContext,
    configuration: Option<crate::ai::provider::AiProviderConfig>,
    embedding_configuration: Option<crate::ai::provider::AiProviderConfig>,
}
async fn ask(
    State(state): State<AppState>,
    Path((id, conversation)): Path<(String, String)>,
    Json(mut input): Json<AskInput>,
) -> ApiResult<Json<Value>> {
    let history = state.repository.conversation_messages(&id, &conversation)?;
    let ai = if let Some(config) = input.configuration {
        Some(AiService::new(config.build().map_err(|e| {
            ApiError(StatusCode::BAD_REQUEST, e.to_string())
        })?))
    } else {
        state.ai.clone()
    };
    let report = |stage: &str| {
        let _ = state
            .events
            .send(crate::api::state::AnalysisEvent::AssistantActivity {
                project_id: id.clone(),
                conversation_id: conversation.clone(),
                stage: stage.into(),
            });
    };
    let neural = input.embedding_configuration.is_some();
    if let Some(config) = input.embedding_configuration {
        report("Recherche hybride avec les embeddings neuronaux");
        let provider = crate::embeddings::RemoteEmbeddings::new(config)?;
        input.context.retrieval_nodes =
            crate::embeddings::hybrid_search(&state.repository, &id, &input.question, &provider)
                .await?
                .into_iter()
                .take(16)
                .map(|c| c.node.id)
                .collect();
    }
    let preliminary_tools = if neural {
        vec![crate::assistant::ToolCall {
            tool: "neural_hybrid_search".into(),
            status: "completed".into(),
            summary: "Recherche neuronale combinée aux scores lexicaux et symboliques.".into(),
        }]
    } else {
        vec![]
    };
    let response = AgentOrchestrator::run(crate::assistant::AssistantRequest {
        repo: &state.repository,
        project: &id,
        question: &input.question,
        context: &input.context,
        history: &history,
        ai: ai.as_ref(),
        observer: Some(&report),
        preliminary_tools,
    })
    .await
    .map_err(|e| ApiError(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    state
        .repository
        .append_turn(&id, &conversation, &input.question, &response)?;
    Ok(Json(json!(response)))
}
#[derive(Deserialize)]
struct PlanQuery {
    feature_id: Option<String>,
}
async fn test_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(input): Query<PlanQuery>,
) -> ApiResult<Json<Value>> {
    let repo = state.repository.clone();
    blocking(move || {
        let graph = repo
            .load(&id)?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        Ok(json!(crate::intelligence::cached_test_plan(
            &repo,
            &id,
            &graph,
            &repo.list_features(&id)?,
            input.feature_id.as_deref()
        )?))
    })
    .await
}
#[derive(Deserialize)]
struct EstimateInput {
    task: String,
}
async fn estimate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<EstimateInput>,
) -> ApiResult<Json<Value>> {
    let repo = state.repository.clone();
    blocking(move || {
        let graph = repo
            .load(&id)?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        Ok(json!(crate::intelligence::estimate(
            &repo,
            &id,
            &graph,
            &repo.list_features(&id)?,
            &input.task
        )?))
    })
    .await
}
#[derive(Deserialize)]
struct DiffQuery {
    base: Option<String>,
    head: Option<String>,
}
async fn diff(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(input): Query<DiffQuery>,
) -> ApiResult<Json<Value>> {
    let repo = state.repository.clone();
    blocking(move || {
        let graph = repo
            .load(&id)?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        Ok(json!(crate::intelligence::git_diff(
            &id,
            &graph,
            &repo.list_features(&id)?,
            input.base.as_deref().filter(|s| !s.is_empty()),
            input.head.as_deref().filter(|s| !s.is_empty())
        )?))
    })
    .await
}

#[derive(Deserialize)]
struct SourceQuery {
    path: String,
}
async fn source_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(input): Query<SourceQuery>,
) -> ApiResult<Json<Value>> {
    let repo = state.repository.clone();
    blocking(move || {
        let graph = repo.load(&id)?.ok_or_else(|| anyhow::anyhow!("project not found"))?;
        let mut node = graph.nodes.iter().find(|n| n.path.as_deref() == Some(&input.path) && n.source_scope.library_eligible()).cloned().ok_or_else(|| anyhow::anyhow!("Fichier absent du graphe du projet"))?;
        node.start_line = None; node.end_line = None;
        let (text, start, end) = crate::retrieval::source(&graph, &node).ok_or_else(|| anyhow::anyhow!("Source inaccessible ou hors du projet"))?;
        if text.len() > 2_000_000 { anyhow::bail!("Ce fichier dépasse la limite de lecture de 2 Mo. Ouvrez un symbole précis."); }
        node.start_line = Some(start); node.end_line = Some(end);
        Ok(json!({"node":node,"source":text,"incoming":graph.incoming_edges(&node.id),"outgoing":graph.outgoing_edges(&node.id)}))
    }).await
}

#[derive(Deserialize)]
struct EmbeddingInput {
    configuration: crate::ai::provider::AiProviderConfig,
    query: Option<String>,
}
async fn neural_index(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<EmbeddingInput>,
) -> ApiResult<Json<Value>> {
    let provider = crate::embeddings::RemoteEmbeddings::new(input.configuration)?;
    Ok(Json(json!(
        crate::embeddings::index(&state.repository, &id, &provider).await?
    )))
}
async fn neural_search(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<EmbeddingInput>,
) -> ApiResult<Json<Value>> {
    let provider = crate::embeddings::RemoteEmbeddings::new(input.configuration)?;
    let query = input
        .query
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "Question requise".into()))?;
    Ok(Json(
        json!({"items":crate::embeddings::hybrid_search(&state.repository,&id,&query,&provider).await?}),
    ))
}

async fn dependencies(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let repo = state.repository.clone();
    blocking(move || {
        let graph = repo
            .load(&id)?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        Ok(json!(crate::dependencies::inventory(&graph)))
    })
    .await
}

#[derive(Deserialize)]
struct EvidenceInput {
    node_id: String,
    source_hash: String,
    start_line: usize,
    end_line: usize,
}
async fn verify_evidence(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<EvidenceInput>,
) -> ApiResult<Json<Value>> {
    let graph = state
        .repository
        .load(&id)?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Projet introuvable".into()))?;
    let node = graph
        .find_node(&input.node_id)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Symbole introuvable".into()))?;
    let current = crate::retrieval::source(&graph, node);
    if !current.is_some_and(|(text, start, end)| {
        crate::retrieval::hash(&text) == input.source_hash
            && start == input.start_line
            && end == input.end_line
    }) {
        return Err(ApiError(StatusCode::CONFLICT,"Cette preuve a changé depuis la réponse. Relancez l’analyse et posez de nouveau la question.".into()));
    }
    Ok(Json(json!({"verified":true})))
}
