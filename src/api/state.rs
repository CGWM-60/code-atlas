use crate::{ai::AiService, engine::AnalysisResult, storage::Repository};
use notify::RecommendedWatcher;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{RwLock, broadcast};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnalysisEvent {
    AnalysisStarted {
        project_id: String,
        total_files: usize,
    },
    AnalysisProgress {
        project_id: String,
        stage: String,
        processed: usize,
        total: usize,
        nodes: usize,
        edges: usize,
        current_file: Option<String>,
        elapsed_ms: u128,
    },
    FileChanged {
        project_id: String,
        paths: Vec<String>,
    },
    GraphUpdated {
        project_id: String,
        nodes: usize,
        edges: usize,
    },
    AnalysisCompleted {
        project_id: String,
        files: usize,
        nodes: usize,
        edges: usize,
        unresolved: usize,
        duration_ms: u128,
    },
    AnalysisError {
        project_id: String,
        message: String,
    },
    FeatureDetectionStarted {
        project_id: String,
    },
    FeatureCandidateProgress {
        project_id: String,
        candidates: usize,
    },
    FeatureAiAnalysis {
        project_id: String,
        provider: String,
        model: String,
    },
    FeatureDetectionCompleted {
        project_id: String,
        features: usize,
    },
    DocumentationStarted {
        project_id: String,
    },
    DocumentationProgress {
        project_id: String,
        stage: String,
    },
    DocumentationCompleted {
        project_id: String,
        cache_hit: bool,
    },
    SecurityScanProgress {
        project_id: String,
        findings: usize,
    },
    QualityScanProgress {
        project_id: String,
        findings: usize,
    },
    FeaturePortProgress {
        feature_id: String,
        stage: String,
    },
}
#[derive(Clone)]
pub struct AppState {
    pub repository: Arc<Repository>,
    pub analyses: Arc<RwLock<HashMap<String, AnalysisResult>>>,
    pub events: broadcast::Sender<AnalysisEvent>,
    pub ai: Option<AiService>,
    pub watchers: Arc<Mutex<HashMap<String, RecommendedWatcher>>>,
}
impl AppState {
    pub fn new(repository: Repository, ai: Option<AiService>) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            repository: Arc::new(repository),
            analyses: Arc::new(RwLock::new(HashMap::new())),
            events,
            ai,
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
