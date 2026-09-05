CREATE TABLE IF NOT EXISTS finding_scans (
    project_id TEXT PRIMARY KEY,
    graph_hash TEXT NOT NULL,
    scanned_at INTEGER NOT NULL
);

