PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS semantic_units (
 project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 id TEXT NOT NULL, node_id TEXT, feature_id TEXT, path TEXT,
 source_hash TEXT NOT NULL, embedding_version TEXT NOT NULL,
 entry_json TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
 PRIMARY KEY(project_id,id)
);
CREATE INDEX IF NOT EXISTS idx_units_feature ON semantic_units(project_id,feature_id);
CREATE TABLE IF NOT EXISTS conversations (
 id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 title TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conversations_project ON conversations(project_id,updated_at);
CREATE TABLE IF NOT EXISTS assistant_messages (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
 role TEXT NOT NULL CHECK(role IN ('user','assistant')), content_json TEXT NOT NULL,
 created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_conversation ON assistant_messages(conversation_id,id);
