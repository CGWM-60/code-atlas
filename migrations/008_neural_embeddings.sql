CREATE TABLE IF NOT EXISTS neural_embeddings (
 project_id TEXT NOT NULL,
 unit_id TEXT NOT NULL,
 version TEXT NOT NULL,
 source_hash TEXT NOT NULL,
 vector_json TEXT NOT NULL,
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 PRIMARY KEY(project_id,unit_id,version),
 FOREIGN KEY(project_id,unit_id) REFERENCES semantic_units(project_id,id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_neural_project_version ON neural_embeddings(project_id,version);
