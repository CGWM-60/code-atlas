PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS ai_analysis_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    analysis_type TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    cache_status TEXT NOT NULL,
    estimated_input_tokens INTEGER,
    actual_input_tokens INTEGER,
    actual_output_tokens INTEGER,
    duration_ms INTEGER NOT NULL,
    structured_output_success INTEGER NOT NULL,
    error TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_ai_runs_project_type ON ai_analysis_runs(project_id,analysis_type,created_at);

CREATE TABLE IF NOT EXISTS module_documentation (
    project_id TEXT NOT NULL,
    module_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    documentation_json TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    prompt_version TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(project_id,module_id),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS feature_documentation (
    feature_id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    documentation_json TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    prompt_version TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(feature_id) REFERENCES features(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_feature_nodes_node ON feature_nodes(node_id,feature_id);
CREATE INDEX IF NOT EXISTS idx_feature_edges_edge ON feature_edges(edge_id,feature_id);
