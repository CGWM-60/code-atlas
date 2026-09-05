PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS project_metadata (
    project_id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    last_analyzed_at INTEGER NOT NULL,
    languages_json TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    unresolved_count INTEGER NOT NULL,
    git_remote TEXT,
    git_branch TEXT,
    analysis_status TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS analysis_runs_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    files_scanned INTEGER NOT NULL,
    files_changed INTEGER NOT NULL,
    nodes INTEGER NOT NULL,
    edges INTEGER NOT NULL,
    unresolved INTEGER NOT NULL,
    status TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS file_analysis_cache (
    project_id TEXT NOT NULL,
    path TEXT NOT NULL,
    hash TEXT NOT NULL,
    analysis_json TEXT NOT NULL,
    last_analyzed_at INTEGER NOT NULL,
    PRIMARY KEY(project_id,path),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS project_snapshots (
    project_id TEXT PRIMARY KEY,
    scan_json TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS file_scopes (
    project_id TEXT NOT NULL,
    path TEXT NOT NULL,
    scope TEXT NOT NULL,
    last_analyzed_at INTEGER NOT NULL,
    PRIMARY KEY(project_id,path),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS library_entries (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    language TEXT NOT NULL,
    category TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    description TEXT NOT NULL,
    source_project_id TEXT NOT NULL,
    source_node_id TEXT NOT NULL,
    source_path TEXT NOT NULL,
    start_line INTEGER,
    end_line INTEGER,
    source_hash TEXT NOT NULL,
    source_scope TEXT NOT NULL,
    reuse_score INTEGER NOT NULL,
    real_usages_json TEXT NOT NULL,
    documentation_json TEXT,
    entry_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(source_project_id,source_node_id),
    FOREIGN KEY(source_project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_library_name ON library_entries(display_name);
CREATE INDEX IF NOT EXISTS idx_library_project ON library_entries(source_project_id);
