import type { AtlasNode, AtlasEdge, Graph } from "../graphView";
import type { FeatureSourceSnippet } from "../FeatureSourceExplorer";
import type { SourceSpan } from "../EvidenceCodeInspector";
export type Details = {
  node: AtlasNode;
  incoming: AtlasEdge[];
  outgoing: AtlasEdge[];
  source?: string;
};
export type DirectoryListing = {
  current: string;
  parent?: string;
  directories: { name: string; path: string }[];
};
export type AiProvider = "openai" | "mistral" | "openrouter";
export type AiSettings = { provider: AiProvider; apiKey: string; model: string };
export type Page =
  | "tests" | "estimate" | "git"
  | "projects"
  | "map"
  | "flow"
  | "search"
  | "features"
  | "my-code"
  | "library"
  | "api"
  | "docs"
  | "quality"
  | "security"
  | "mcp";
export type ProjectSummary = {
  id: string;
  root: string;
  name: string;
  updated_at: number;
  last_analyzed_at: number;
  languages: string[];
  file_count: number;
  unresolved_count: number;
  analysis_status: string;
  node_count: number;
  edge_count: number;
  git_remote?: string;
  git_branch?: string;
};
export type ProgressState = {
  project_id?: string;
  stage: string;
  processed: number;
  total: number;
  nodes: number;
  edges: number;
  current_file?: string;
  elapsed_ms: number;
  unresolved?: number;
};
export type ScoreFactor = { label: string; points: number };
export type RealUsage = {
  caller: string;
  path: string;
  line?: number;
  snippet?: string;
};
export type AiFunctionParameter = {
  name: string;
  type: string | null;
  description: string;
};
export type AiFunctionDocumentation = {
  summary: string;
  purpose: string;
  parameters: AiFunctionParameter[];
  returns: { type: string | null; description: string };
  errors: string[];
  side_effects: string[];
  use_cases: string[];
  limitations: string[];
  security_notes: string[];
  tags: string[];
  category: string | null;
};
export type LibraryEntry = {
  id: string;
  display_name: string;
  language: string;
  category: string;
  tags: string[];
  description: string;
  source_project_id: string;
  source_node_id: string;
  source_path: string;
  start_line?: number;
  end_line?: number;
  source_hash: string;
  source_scope: string;
  source_repository?: string;
  source_version?: string;
  source_branch?: string;
  source_license?: string;
  recipe_id?: string;
  reuse_score: number;
  knowledge_value: number;
  real_usages: RealUsage[];
  calls: string[];
  dependencies: string[];
  tests: RealUsage[];
  source_code: string;
  documentation_status: "not_generated" | "generating" | "generated" | "failed";
  documentation_provider?: string;
  documentation_model?: string;
  documentation_generated_at?: number;
  documentation_error?: string;
  documentation?: AiFunctionDocumentation;
  created_at: number;
  updated_at: number;
};
export type LibraryCandidate = {
  node: AtlasNode;
  reuse_score: number;
  knowledge_value: number;
  reasons: string[];
  reuse_breakdown: ScoreFactor[];
  knowledge_breakdown: ScoreFactor[];
  real_usages: RealUsage[];
  source: string;
  signature: string;
  parameters: string[];
  return_type?: string;
  calls: string[];
  dependencies: string[];
  tests: RealUsage[];
  side_effects: string[];
  loc: number;
  complexity: number;
};
export type MyCodeItem = {
  node: AtlasNode;
  incoming_usages: number;
  outgoing_dependencies: number;
  tests: number;
  complexity?: number;
  coupling: number;
  reuse_score?: number;
  knowledge_value?: number;
  library_status: "saved" | "candidate" | "not_eligible";
};
export type Finding = {
  id: string;
  category: "quality" | "architecture" | "performance" | "security";
  severity: "info" | "low" | "medium" | "high" | "critical";
  title: string;
  description: string;
  evidence: string[];
  node_ids: string[];
  path?: string;
  start_line?: number;
  end_line?: number;
  primary_span?: SourceSpan;
  evidence_spans?: SourceSpan[];
  detector: string;
  confidence: number;
  status: "open" | "accepted" | "ignored" | "fixed" | "false_positive";
  ai_analysis?: string;
};
export type ProjectDocumentation = {
  project_name: string;
  overview: string;
  purpose: string;
  architecture: string[];
  applications: string[];
  entry_points: string[];
  features: string[];
  domains: string[];
  main_flows: string[];
  modules: string[];
  apis: string[];
  data_model: string[];
  external_services: string[];
  workers: string[];
  events: string[];
  configuration: string[];
  security_notes: string[];
  development: string[];
  deployment: string[];
  known_limitations: string[];
  content_hash: string;
  cache_hit: boolean;
  updated_at: number;
};
export type FeatureMembership = {
  node_id: string;
  confidence: number;
  reason: string;
  source: "deterministic" | "ai" | "user";
  optional: boolean;
  external_dependency: boolean;
};
export type Feature = {
  id: string;
  project_id: string;
  name: string;
  description: string;
  confidence: number;
  completeness?: number;
  missing_signals?: string[];
  status: "detected" | "accepted" | "edited" | "ignored";
  entry_point_node_ids: string[];
  node_ids: string[];
  edge_ids: string[];
  memberships?: FeatureMembership[];
  routes: string[];
  pages: string[];
  services: string[];
  repositories: string[];
  models: string[];
  entities: string[];
  templates: string[];
  tests: string[];
  external_services: string[];
  source_hash: string;
};
export type FeatureSpec = {
  name: string;
  description: string;
  actors: string[];
  entry_points: string[];
  inputs: string[];
  outputs: string[];
  business_rules: string[];
  operations: string[];
  data: string[];
  side_effects: string[];
  security: string[];
  error_cases: string[];
  acceptance_criteria: string[];
  dependencies: string[];
  provenance: string;
};
export type FeatureSnippet = FeatureSourceSnippet;
export type FeatureLibraryEntry = {
  id: string;
  source_feature_id: string;
  source_project_id: string;
  name: string;
  languages: string[];
  frameworks: string[];
  source_hash: string;
  spec: FeatureSpec;
  documentation: string;
  acceptance_criteria: string[];
  security_notes: string[];
  architecture_summary: string;
  created_at: number;
  updated_at: number;
};
export type ProjectDashboard = {
  architecture: { zones: number; entry_points: number };
  features: { detected: number; accepted: number };
  my_code: { functions: number; classes: number; services: number };
  quality: { critical: number; high: number; medium: number };
  security: { high: number; medium: number };
  documentation: { current: boolean };
  library: { functions: number; features: number };
};
export type GeneratedFile = { path: string; content: string; is_test: boolean };
export type McpStatus = {
  status: string;
  transport: string;
  protocol_version: string;
  server_version: string;
  database: string;
  registered_projects: number;
  tool_count: number;
  tools: string[];
  command: string;
  checked_at: number;
};

