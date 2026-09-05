PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS findings (
    id TEXT PRIMARY KEY,
    fingerprint TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    status TEXT NOT NULL,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_findings_project_category ON findings(project_id, category);
CREATE INDEX IF NOT EXISTS idx_findings_status ON findings(status);

CREATE TABLE IF NOT EXISTS ai_cache (
    cache_key TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    analysis_type TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    result_json TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_ai_cache_project_type ON ai_cache(project_id, analysis_type);

CREATE TABLE IF NOT EXISTS project_documentation (
    project_id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    documentation_json TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    prompt_version TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS features (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_features_project ON features(project_id, status);

CREATE TABLE IF NOT EXISTS feature_nodes (
    feature_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    confidence REAL NOT NULL,
    reason TEXT NOT NULL,
    source TEXT NOT NULL,
    PRIMARY KEY(feature_id,node_id),
    FOREIGN KEY(feature_id) REFERENCES features(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS feature_edges (
    feature_id TEXT NOT NULL,
    edge_id TEXT NOT NULL,
    PRIMARY KEY(feature_id,edge_id),
    FOREIGN KEY(feature_id) REFERENCES features(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS feature_specs (
    feature_id TEXT PRIMARY KEY,
    spec_json TEXT NOT NULL,
    provenance TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(feature_id) REFERENCES features(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS feature_library (
    id TEXT PRIMARY KEY,
    source_feature_id TEXT NOT NULL,
    source_project_id TEXT NOT NULL,
    bundle_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(source_feature_id) REFERENCES features(id) ON DELETE CASCADE,
    FOREIGN KEY(source_project_id) REFERENCES projects(id) ON DELETE CASCADE
);
