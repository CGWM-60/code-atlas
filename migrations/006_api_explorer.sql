PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS api_environments (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_environments_project ON api_environments(project_id, updated_at);

CREATE TABLE IF NOT EXISTS api_saved_requests (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    collection_id TEXT,
    name TEXT NOT NULL,
    favorite INTEGER NOT NULL DEFAULT 0,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_saved_requests_project ON api_saved_requests(project_id, updated_at);

CREATE TABLE IF NOT EXISTS api_request_history (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    endpoint_id TEXT,
    method TEXT NOT NULL,
    url TEXT NOT NULL,
    status INTEGER,
    duration_ms INTEGER,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_history_project ON api_request_history(project_id, created_at);

CREATE TABLE IF NOT EXISTS api_collections (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_collections_project ON api_collections(project_id, updated_at);

CREATE TABLE IF NOT EXISTS api_contract_tests (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_contract_tests_project ON api_contract_tests(project_id, updated_at);

CREATE TABLE IF NOT EXISTS api_test_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    test_id TEXT NOT NULL,
    passed INTEGER NOT NULL,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(test_id) REFERENCES api_contract_tests(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_test_runs_test ON api_test_runs(test_id, created_at);

CREATE TABLE IF NOT EXISTS api_openapi_specs (
    project_id TEXT PRIMARY KEY,
    document_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
