use crate::ai::provider::AiProviderConfig;
use crate::context_engine::ContextIntent;
use crate::features::{FeatureSpec, FeatureStatus};
use crate::findings::FindingStatus;
use crate::model::{edge::CodeEdge, node::CodeNode};
use crate::porting::{PortPreview, TargetProfile};
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize)]
pub struct AnalyzeProjectRequest {
    pub path: String,
    #[serde(default)]
    pub watch: bool,
}
#[derive(Debug, Serialize)]
pub struct AnalyzeProjectResponse {
    pub id: String,
    pub root: String,
    pub nodes: usize,
    pub edges: usize,
    pub analyzed_files: usize,
    pub reused_files: usize,
}
#[derive(Debug, Deserialize)]
pub struct Pagination {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
#[derive(Debug, Serialize)]
pub struct NodeDetails {
    pub node: CodeNode,
    pub incoming: Vec<CodeEdge>,
    pub outgoing: Vec<CodeEdge>,
    pub source: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct ImpactQuery {
    pub depth: Option<usize>,
}
#[derive(Deserialize)]
pub struct AiQuery {
    pub question: String,
    pub node_id: Option<String>,
    pub configuration: Option<AiProviderConfig>,
}

#[derive(Debug, Deserialize)]
pub struct DirectoryQuery {
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct DirectoryListing {
    pub current: String,
    pub parent: Option<String>,
    pub directories: Vec<DirectoryEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLibraryRequest {
    pub project_id: String,
    pub node_id: String,
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LibraryQuery {
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MyCodeQuery {
    pub q: Option<String>,
    pub kind: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct VisibleViewQuery {
    pub level: Option<String>,
    pub q: Option<String>,
    pub focus: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFindingRequest {
    pub status: FindingStatus,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeatureRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<FeatureStatus>,
}

#[derive(Debug, Deserialize)]
pub struct FeatureMembershipRequest {
    pub action: String,
    pub node_id: String,
    pub target_feature_id: Option<String>,
    pub optional: Option<bool>,
    pub external_dependency: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FeatureSourceQuery {
    pub q: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FeatureSpecRequest {
    pub spec: Option<FeatureSpec>,
    pub configuration: Option<AiProviderConfig>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FeatureDetectionRequest {
    pub configuration: Option<AiProviderConfig>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AiOperationRequest {
    pub configuration: Option<AiProviderConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ContextRequest {
    pub intent: ContextIntent,
    pub task: String,
    #[serde(default)]
    pub node_ids: Vec<String>,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct PortRequest {
    pub target_project_id: Option<String>,
    pub target_profile: TargetProfile,
    #[serde(default)]
    pub constraints: Vec<String>,
    pub configuration: Option<AiProviderConfig>,
}
#[derive(Debug, Deserialize)]
pub struct PortApplyRequest {
    pub target_project_id: String,
    pub expected_target_hash: String,
    pub preview: PortPreview,
    pub confirm: bool,
}
#[derive(Debug, Deserialize)]
pub struct CompareFeatureQuery {
    pub other_feature_id: String,
}

#[derive(Debug, Deserialize)]
pub struct MergeFeaturesRequest {
    pub feature_ids: Vec<String>,
    pub name: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct SplitFeaturePart {
    pub name: String,
    pub node_ids: Vec<String>,
}
#[derive(Debug, Deserialize)]
pub struct SplitFeatureRequest {
    pub parts: Vec<SplitFeaturePart>,
}

#[derive(Deserialize)]
pub struct GenerateDocumentationRequest {
    pub configuration: Option<AiProviderConfig>,
}
