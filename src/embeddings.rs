//! Optional neural embeddings. Keys are request-scoped; SQLite stores vectors only.
use crate::{
    ai::provider::{AiProviderConfig, AiProviderKind},
    retrieval::{SemanticUnit, now, rebuild_project},
    storage::Repository,
};
use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[async_trait]
pub trait NeuralEmbeddingProvider: Send + Sync {
    fn version(&self) -> String;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}
pub struct RemoteEmbeddings {
    config: AiProviderConfig,
    client: reqwest::Client,
}
impl RemoteEmbeddings {
    pub fn new(config: AiProviderConfig) -> Result<Self> {
        if config.api_key.trim().is_empty()
            || config.api_key.len() > 4096
            || config.model.trim().is_empty()
            || config.model.len() > 200
        {
            bail!("Clé et modèle d’embedding valides requis");
        }
        Ok(Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
        })
    }
    fn provider(&self) -> (&str, &str) {
        match self.config.provider {
            AiProviderKind::Openai => ("openai", "https://api.openai.com/v1/embeddings"),
            AiProviderKind::Mistral => ("mistral", "https://api.mistral.ai/v1/embeddings"),
            AiProviderKind::Openrouter => ("openrouter", "https://openrouter.ai/api/v1/embeddings"),
        }
    }
}
#[async_trait]
impl NeuralEmbeddingProvider for RemoteEmbeddings {
    fn version(&self) -> String {
        format!(
            "{}:{}:semantic-unit-v1",
            self.provider().0,
            self.config.model.trim()
        )
    }
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let response = self
            .client
            .post(self.provider().1)
            .bearer_auth(self.config.api_key.trim())
            .json(
                &json!({"input":texts,"model":self.config.model.trim(),"encoding_format":"float"}),
            )
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .await?
                .replace(&self.config.api_key, "[REDACTED]");
            bail!(
                "Embeddings {} : {}",
                status,
                message.chars().take(600).collect::<String>()
            );
        }
        #[derive(Deserialize)]
        struct Item {
            index: usize,
            embedding: Vec<f32>,
        }
        #[derive(Deserialize)]
        struct Response {
            data: Vec<Item>,
        }
        let mut data = response.json::<Response>().await?.data;
        data.sort_by_key(|v| v.index);
        if data.len() != texts.len() || data.iter().enumerate().any(|(i, item)| item.index != i) {
            bail!("Le provider a renvoyé des indices de vecteurs incohérents");
        }
        let dimension = data.first().map_or(0, |i| i.embedding.len());
        if dimension == 0 || data.iter().any(|i| i.embedding.len() != dimension) {
            bail!("Dimensions de vecteurs incohérentes");
        }
        data.into_iter().map(|i| normalize(i.embedding)).collect()
    }
}
pub fn normalize(mut vector: Vec<f32>) -> Result<Vec<f32>> {
    if vector.is_empty() || vector.iter().any(|v| !v.is_finite()) {
        bail!("Vecteur vide ou non fini");
    }
    let norm = vector
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    if norm == 0.0 || !norm.is_finite() {
        bail!("Norme de vecteur invalide");
    }
    for v in &mut vector {
        *v = (f64::from(*v) / norm) as f32;
    }
    Ok(vector)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralRecord {
    pub unit_id: String,
    pub version: String,
    pub source_hash: String,
    pub vector: Vec<f32>,
}
#[derive(Debug, Serialize)]
pub struct NeuralStats {
    pub version: String,
    pub total: usize,
    pub updated: usize,
    pub reused: usize,
    pub skipped: Vec<String>,
    pub remaining: usize,
}
impl Repository {
    pub fn neural_records(&self, project: &str, version: &str) -> Result<Vec<NeuralRecord>> {
        let db = self
            .connection
            .lock()
            .map_err(|_| anyhow!("database mutex poisoned"))?;
        let mut stmt=db.prepare("SELECT unit_id,source_hash,vector_json FROM neural_embeddings WHERE project_id=?1 AND version=?2 ORDER BY unit_id")?;
        let rows = stmt
            .query_map(params![project, version], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(unit_id, source_hash, vector)| {
                Ok(NeuralRecord {
                    unit_id,
                    source_hash,
                    version: version.into(),
                    vector: serde_json::from_str(&vector)?,
                })
            })
            .collect()
    }
    pub fn save_neural_records(&self, project: &str, records: &[NeuralRecord]) -> Result<()> {
        let mut db = self
            .connection
            .lock()
            .map_err(|_| anyhow!("database mutex poisoned"))?;
        let tx = db.transaction()?;
        for r in records {
            let vector = normalize(r.vector.clone())?;
            tx.execute("INSERT INTO neural_embeddings(project_id,unit_id,version,source_hash,vector_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6) ON CONFLICT(project_id,unit_id,version) DO UPDATE SET source_hash=excluded.source_hash,vector_json=excluded.vector_json,updated_at=excluded.updated_at",params![project,r.unit_id,r.version,r.source_hash,serde_json::to_string(&vector)?,now()])?;
        }
        tx.commit()?;
        Ok(())
    }
}
pub async fn index(
    repo: &Repository,
    project: &str,
    provider: &dyn NeuralEmbeddingProvider,
) -> Result<NeuralStats> {
    let graph = repo
        .load(project)?
        .ok_or_else(|| anyhow!("project not found"))?;
    let features = repo.list_features(project)?;
    rebuild_project(repo, project, &graph, &features, false)?;
    let version = provider.version();
    let previous = repo
        .neural_records(project, &version)?
        .into_iter()
        .map(|v| (v.unit_id.clone(), v))
        .collect::<HashMap<_, _>>();
    let units = repo.semantic_units(project)?;
    let mut stats = NeuralStats {
        version: version.clone(),
        total: units.len(),
        updated: 0,
        reused: 0,
        skipped: vec![],
        remaining: 0,
    };
    let mut pending = Vec::new();
    for unit in units {
        if previous
            .get(&unit.id)
            .is_some_and(|v| v.source_hash == unit.source_hash)
        {
            stats.reused += 1;
            continue;
        }
        // Keep semantic boundaries intact. Large units remain searchable lexically;
        // their smaller child symbols can be embedded independently.
        if unit.text.len() > 24_000 {
            stats.skipped.push(format!(
                "{} : unité trop grande, utiliser ses symboles enfants",
                unit.symbol
            ));
            continue;
        }
        pending.push(unit);
    }
    // A request does bounded work. Repeating the explicit indexing action resumes
    // without paying again for successfully persisted batches.
    stats.remaining = pending.len().saturating_sub(128);
    pending.truncate(128);
    for batch in pending.chunks(8) {
        let vectors = provider
            .embed(&batch.iter().map(|u| u.text.clone()).collect::<Vec<_>>())
            .await?;
        if vectors.len() != batch.len() {
            bail!("Nombre d’embeddings incorrect");
        }
        let records = batch
            .iter()
            .zip(vectors)
            .map(|(unit, vector)| NeuralRecord {
                unit_id: unit.id.clone(),
                version: version.clone(),
                source_hash: unit.source_hash.clone(),
                vector,
            })
            .collect::<Vec<_>>();
        repo.save_neural_records(project, &records)?;
        stats.updated += records.len();
    }
    Ok(stats)
}
#[derive(Debug, Serialize)]
pub struct NeuralHit {
    pub unit: SemanticUnit,
    pub score: f32,
}
pub async fn search(
    repo: &Repository,
    project: &str,
    query: &str,
    provider: &dyn NeuralEmbeddingProvider,
) -> Result<Vec<NeuralHit>> {
    if query.trim().is_empty() || query.len() > 8000 {
        bail!("Question vide ou trop longue");
    }
    let graph = repo
        .load(project)?
        .ok_or_else(|| anyhow!("project not found"))?;
    rebuild_project(repo, project, &graph, &repo.list_features(project)?, false)?;
    let units = repo
        .semantic_units(project)?
        .into_iter()
        .map(|u| (u.id.clone(), u))
        .collect::<HashMap<_, _>>();
    let records = repo.neural_records(project, &provider.version())?;
    if records.is_empty() {
        bail!("Aucun embedding pour ce modèle. Vectorisez d’abord le projet.");
    }
    let vectors = provider.embed(&[query.into()]).await?;
    let query = normalize(
        vectors
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Vecteur de question absent"))?,
    )?;
    let mut hits = Vec::new();
    for record in records {
        if let Some(unit) = units
            .get(&record.unit_id)
            .filter(|u| u.source_hash == record.source_hash)
        {
            if record.vector.len() != query.len() {
                continue;
            }
            let score = record
                .vector
                .iter()
                .zip(&query)
                .map(|(a, b)| a * b)
                .sum::<f32>();
            hits.push(NeuralHit {
                unit: unit.clone(),
                score,
            });
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.unit.id.cmp(&b.unit.id))
    });
    hits.truncate(50);
    Ok(hits)
}

pub async fn hybrid_search(
    repo: &Repository,
    project: &str,
    query: &str,
    provider: &dyn NeuralEmbeddingProvider,
) -> Result<Vec<crate::retrieval::Candidate>> {
    use crate::retrieval::{Candidate, DeterministicReranker, HybridRetriever, Reranker};
    let graph = repo
        .load(project)?
        .ok_or_else(|| anyhow!("project not found"))?;
    let features = repo.list_features(project)?;
    let mut candidates =
        HybridRetriever::search(repo, project, &graph, &features, query, None, 100)?;
    for candidate in &mut candidates {
        candidate.score_semantic = 0.0;
    }
    for hit in search(repo, project, query, provider).await? {
        let ids = hit.unit.node_id.into_iter().chain(
            hit.unit
                .feature_id
                .as_deref()
                .and_then(|id| features.iter().find(|f| f.id == id))
                .into_iter()
                .flat_map(|f| f.entry_point_node_ids.clone()),
        );
        for id in ids {
            if let Some(candidate) = candidates.iter_mut().find(|c| c.node.id == id) {
                candidate.score_semantic = hit.score.max(0.0);
                candidate
                    .reasons
                    .push(format!("neural embedding: {}", provider.version()));
            } else if let Some(node) = graph.find_node(&id) {
                candidates.push(Candidate {
                    node: node.clone(),
                    score_lexical: 0.0,
                    score_symbol: 0.0,
                    score_semantic: hit.score.max(0.0),
                    score_graph: 0.0,
                    score_feature: 0.0,
                    score_final: 0.0,
                    reasons: vec![format!("neural embedding: {}", provider.version())],
                });
            }
        }
    }
    DeterministicReranker.rerank(&mut candidates, 50);
    Ok(candidates)
}
