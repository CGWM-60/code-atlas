use crate::{
    api_explorer::{
        ApiCollection, ApiContractRun, ApiContractTest, ApiEnvironment, ApiHistoryEntry,
        ApiSavedRequest,
    },
    context_engine::AiCacheWrite,
    documentation::ProjectDocumentation,
    engine::AnalysisResult,
    features::{Feature, FeatureDocumentation, FeatureLibraryEntry, FeatureSpec, FeatureStatus},
    findings::{Finding, FindingStatus},
    graph::project_graph::ProjectGraph,
    library::{DocumentationStatus, FunctionDocumentation, LibraryEntry},
    model::analysis::FileAnalysis,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::Path,
    process::Command,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub root: String,
    pub name: String,
    pub updated_at: i64,
    pub created_at: i64,
    pub last_analyzed_at: i64,
    pub languages: Vec<String>,
    pub file_count: usize,
    pub unresolved_count: usize,
    pub analysis_status: String,
    pub git_remote: Option<String>,
    pub git_branch: Option<String>,
    pub node_count: usize,
    pub edge_count: usize,
}

/// SQLite repository. A mutex serializes short transactions; AST parsing never runs
/// while this lock is held, so concurrent HTTP reads stay responsive.
pub struct Repository {
    connection: Mutex<Connection>,
}
impl Repository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path).context("cannot open Code Atlas database")?;
        connection.execute_batch(include_str!("../../migrations/001_initial.sql"))?;
        connection.execute_batch(include_str!("../../migrations/002_registry_library.sql"))?;
        connection.execute_batch(include_str!("../../migrations/003_knowledge.sql"))?;
        connection.execute_batch(include_str!("../../migrations/004_product_completion.sql"))?;
        connection.execute_batch(include_str!("../../migrations/005_finding_scan_cache.sql"))?;
        connection.execute_batch(include_str!("../../migrations/006_api_explorer.sql"))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
    pub fn in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(include_str!("../../migrations/001_initial.sql"))?;
        connection.execute_batch(include_str!("../../migrations/002_registry_library.sql"))?;
        connection.execute_batch(include_str!("../../migrations/003_knowledge.sql"))?;
        connection.execute_batch(include_str!("../../migrations/004_product_completion.sql"))?;
        connection.execute_batch(include_str!("../../migrations/005_finding_scan_cache.sql"))?;
        connection.execute_batch(include_str!("../../migrations/006_api_explorer.sql"))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
    pub fn save(&self, id: &str, analysis: &AnalysisResult) -> Result<()> {
        self.save_with_duration(id, analysis, 0)
    }

    pub fn save_with_duration(
        &self,
        id: &str,
        analysis: &AnalysisResult,
        duration_ms: u128,
    ) -> Result<()> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let tx = connection.transaction()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let name = Path::new(&analysis.scan.root)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("project");
        let graph_json = serde_json::to_string(&analysis.graph)?;
        let (git_remote, git_branch) = git_metadata(Path::new(&analysis.scan.root));
        tx.execute("INSERT INTO projects(id,root,name,updated_at,graph_json) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET root=excluded.root,name=excluded.name,updated_at=excluded.updated_at,graph_json=excluded.graph_json",params![id,analysis.scan.root,name,now,graph_json])?;
        let languages = serde_json::to_string(
            &analysis
                .scan
                .language_counts
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
        )?;
        tx.execute("INSERT INTO project_metadata(project_id,created_at,last_analyzed_at,languages_json,file_count,unresolved_count,git_remote,git_branch,analysis_status) VALUES(?1,?2,?2,?3,?4,?5,?6,?7,'completed') ON CONFLICT(project_id) DO UPDATE SET last_analyzed_at=excluded.last_analyzed_at,languages_json=excluded.languages_json,file_count=excluded.file_count,unresolved_count=excluded.unresolved_count,git_remote=excluded.git_remote,git_branch=excluded.git_branch,analysis_status='completed'",params![id,now,languages,analysis.scan.total_files as i64,analysis.graph.unresolved_calls.len() as i64,git_remote,git_branch])?;
        tx.execute("INSERT INTO project_snapshots(project_id,scan_json) VALUES(?1,?2) ON CONFLICT(project_id) DO UPDATE SET scan_json=excluded.scan_json",params![id,serde_json::to_string(&analysis.scan)?])?;
        for table in [
            "files",
            "nodes",
            "edges",
            "imports",
            "modules",
            "file_scopes",
            "file_analysis_cache",
        ] {
            tx.execute(&format!("DELETE FROM {table} WHERE project_id=?1"), [id])?;
        }
        {
            let mut statement = tx.prepare(
                "INSERT INTO files(project_id,path,hash,language,size) VALUES(?1,?2,?3,?4,?5)",
            )?;
            for file in &analysis.scan.files {
                statement.execute(params![
                    id,
                    file.path,
                    file.hash,
                    file.language.as_str(),
                    file.size as i64
                ])?;
            }
        }
        {
            let mut statement = tx.prepare("INSERT INTO file_scopes(project_id,path,scope,last_analyzed_at) VALUES(?1,?2,?3,?4)")?;
            for file in &analysis.scan.files {
                statement.execute(params![id, file.path, file.source_scope.as_str(), now])?;
            }
        }
        {
            let hashes = analysis
                .scan
                .files
                .iter()
                .map(|file| (file.path.as_str(), file.hash.as_str()))
                .collect::<HashMap<_, _>>();
            let mut statement = tx.prepare("INSERT INTO file_analysis_cache(project_id,path,hash,analysis_json,last_analyzed_at) VALUES(?1,?2,?3,?4,?5)")?;
            for (path, cached) in &analysis.file_analyses {
                if let Some(hash) = hashes.get(path.as_str()) {
                    statement.execute(params![
                        id,
                        path,
                        hash,
                        serde_json::to_string(cached)?,
                        now
                    ])?;
                }
            }
        }
        {
            let mut statement = tx.prepare(
                "INSERT INTO nodes(project_id,id,name,kind,path,json) VALUES(?1,?2,?3,?4,?5,?6)",
            )?;
            for node in &analysis.graph.nodes {
                statement.execute(params![
                    id,
                    node.id,
                    node.name,
                    node.kind.as_str(),
                    node.path,
                    serde_json::to_string(node)?
                ])?;
            }
        }
        {
            let mut statement=tx.prepare("INSERT INTO edges(project_id,id,source_id,target_id,relation,json) VALUES(?1,?2,?3,?4,?5,?6)")?;
            for edge in &analysis.graph.edges {
                statement.execute(params![
                    id,
                    edge.id,
                    edge.source_id,
                    edge.target_id,
                    edge.relation.as_str(),
                    serde_json::to_string(edge)?
                ])?;
            }
        }
        {
            let mut statement = tx.prepare(
                "INSERT INTO imports(project_id,source_path,line,json) VALUES(?1,?2,?3,?4)",
            )?;
            for import in &analysis.graph.imports {
                statement.execute(params![
                    id,
                    import.source_path,
                    import.line as i64,
                    serde_json::to_string(import)?
                ])?;
            }
        }
        {
            let mut statement =
                tx.prepare("INSERT INTO modules(project_id,module_path,json) VALUES(?1,?2,?3)")?;
            for module in &analysis.graph.modules {
                statement.execute(params![
                    id,
                    module.module_path,
                    serde_json::to_string(module)?
                ])?;
            }
        }
        tx.execute("INSERT INTO analysis_runs(project_id,started_at,finished_at,analyzed_files,reused_files,status) VALUES(?1,?2,?2,?3,?4,'completed')",params![id,now,analysis.analyzed_files as i64,analysis.reused_files as i64])?;
        let duration = duration_ms.min(i64::MAX as u128) as i64;
        let started_at = now.saturating_sub(duration.saturating_add(999) / 1000);
        tx.execute("INSERT INTO analysis_runs_v2(project_id,started_at,completed_at,duration_ms,files_scanned,files_changed,nodes,edges,unresolved,status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'completed')",params![id,started_at,now,duration,analysis.scan.total_files as i64,analysis.analyzed_files as i64,analysis.graph.nodes.len() as i64,analysis.graph.edges.len() as i64,analysis.graph.unresolved_calls.len() as i64])?;
        tx.commit()?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<ProjectSummary>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let mut statement = connection.prepare(
            "SELECT p.id,p.root,p.name,p.updated_at,p.graph_json,COALESCE(m.created_at,p.updated_at),COALESCE(m.last_analyzed_at,p.updated_at),COALESCE(m.languages_json,'[]'),COALESCE(m.file_count,0),COALESCE(m.unresolved_count,0),COALESCE(m.analysis_status,'completed'),m.git_remote,m.git_branch FROM projects p LEFT JOIN project_metadata m ON m.project_id=p.id ORDER BY p.updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (
                id,
                root,
                name,
                updated_at,
                json,
                created_at,
                last_analyzed_at,
                languages_json,
                file_count,
                unresolved_count,
                analysis_status,
                git_remote,
                git_branch,
            ) = row?;
            let graph: ProjectGraph = serde_json::from_str(&json)?;
            result.push(ProjectSummary {
                id,
                root,
                name,
                updated_at,
                created_at,
                last_analyzed_at,
                languages: serde_json::from_str(&languages_json).unwrap_or_default(),
                file_count: file_count.max(0) as usize,
                unresolved_count: unresolved_count.max(0) as usize,
                analysis_status,
                git_remote,
                git_branch,
                node_count: graph.nodes.len(),
                edge_count: graph.edges.len(),
            });
        }
        Ok(result)
    }
    pub fn load(&self, id: &str) -> Result<Option<ProjectGraph>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let mut statement = connection.prepare("SELECT graph_json FROM projects WHERE id=?1")?;
        let mut rows = statement.query([id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let mut graph: ProjectGraph = serde_json::from_str(&row.get::<_, String>(0)?)?;
        graph.rebuild_indexes();
        Ok(Some(graph))
    }

    pub fn load_analysis(&self, id: &str) -> Result<Option<AnalysisResult>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let row = connection.query_row(
            "SELECT p.graph_json,s.scan_json FROM projects p JOIN project_snapshots s ON s.project_id=p.id WHERE p.id=?1",
            [id],
            |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)),
        );
        let (graph_json, scan_json) = match row {
            Ok(value) => value,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut graph: ProjectGraph = serde_json::from_str(&graph_json)?;
        graph.rebuild_indexes();
        let scan = serde_json::from_str(&scan_json)?;
        let mut statement = connection
            .prepare("SELECT path,analysis_json FROM file_analysis_cache WHERE project_id=?1")?;
        let rows = statement.query_map([id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut file_analyses = HashMap::new();
        for row in rows {
            let (path, json) = row?;
            file_analyses.insert(path, serde_json::from_str::<FileAnalysis>(&json)?);
        }
        Ok(Some(AnalysisResult {
            graph,
            scan,
            analyzed_files: 0,
            reused_files: file_analyses.len(),
            file_analyses,
        }))
    }

    pub fn delete_project(&self, id: &str) -> Result<bool> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        Ok(connection.execute("DELETE FROM projects WHERE id=?1", [id])? > 0)
    }

    pub fn update_project_status(&self, id: &str, status: &str) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "UPDATE project_metadata SET analysis_status=?2 WHERE project_id=?1",
            params![id, status],
        )?;
        Ok(())
    }

    pub fn file_hash(&self, project_id: &str, path: &str) -> Result<Option<String>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row(
            "SELECT hash FROM files WHERE project_id=?1 AND path=?2",
            params![project_id, path],
            |row| row.get(0),
        ) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save_library_entry(&self, entry: &LibraryEntry) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute("INSERT INTO library_entries(id,display_name,language,category,tags_json,description,source_project_id,source_node_id,source_path,start_line,end_line,source_hash,source_scope,reuse_score,real_usages_json,documentation_json,entry_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19) ON CONFLICT(id) DO UPDATE SET display_name=excluded.display_name,category=excluded.category,tags_json=excluded.tags_json,description=excluded.description,reuse_score=excluded.reuse_score,real_usages_json=excluded.real_usages_json,documentation_json=excluded.documentation_json,entry_json=excluded.entry_json,updated_at=excluded.updated_at",params![entry.id,entry.display_name,entry.language,entry.category,serde_json::to_string(&entry.tags)?,entry.description,entry.source_project_id,entry.source_node_id,entry.source_path,entry.start_line.map(|value|value as i64),entry.end_line.map(|value|value as i64),entry.source_hash,entry.source_scope,entry.reuse_score as i64,serde_json::to_string(&entry.real_usages)?,entry.documentation.as_ref().map(serde_json::to_string).transpose()?,serde_json::to_string(entry)?,entry.created_at,entry.updated_at])?;
        Ok(())
    }

    pub fn list_library(&self, query: Option<&str>) -> Result<Vec<LibraryEntry>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let mut statement = connection
            .prepare("SELECT entry_json FROM library_entries ORDER BY updated_at DESC")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let needle = query.unwrap_or_default().trim().to_lowercase();
        let mut entries = Vec::new();
        for row in rows {
            let entry: LibraryEntry = serde_json::from_str(&row?)?;
            let searchable = format!(
                "{} {} {} {} {} {} {} {}",
                entry.display_name,
                entry.description,
                entry.category,
                entry.tags.join(" "),
                entry.language,
                entry.source_path,
                entry.source_project_id,
                entry
                    .documentation
                    .as_ref()
                    .and_then(|value| serde_json::to_string(value).ok())
                    .unwrap_or_default()
            )
            .to_lowercase();
            let terms = needle
                .split(|character: char| !character.is_alphanumeric())
                .filter(|term| term.chars().count() >= 3)
                .collect::<Vec<_>>();
            if needle.is_empty()
                || searchable.contains(&needle)
                || terms.iter().any(|term| searchable.contains(term))
            {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    pub fn get_library_entry(&self, id: &str) -> Result<Option<LibraryEntry>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row(
            "SELECT entry_json FROM library_entries WHERE id=?1",
            [id],
            |row| row.get::<_, String>(0),
        ) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn delete_library_entry(&self, id: &str) -> Result<bool> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        Ok(connection.execute("DELETE FROM library_entries WHERE id=?1", [id])? > 0)
    }

    pub fn save_library_documentation(
        &self,
        id: &str,
        documentation: FunctionDocumentation,
        provider: &str,
        model: &str,
    ) -> Result<Option<LibraryEntry>> {
        let Some(mut entry) = self.get_library_entry(id)? else {
            return Ok(None);
        };
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        if let Some(category) = documentation
            .category
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            entry.category = category.to_owned();
        }
        if !documentation.tags.is_empty() {
            entry.tags = documentation.tags.clone();
        }
        entry.documentation = Some(documentation);
        entry.documentation_status = DocumentationStatus::Generated;
        entry.documentation_provider = Some(provider.to_owned());
        entry.documentation_model = Some(model.to_owned());
        entry.documentation_generated_at = Some(now);
        entry.documentation_error = None;
        entry.updated_at = now;
        self.save_library_entry(&entry)?;
        Ok(Some(entry))
    }

    pub fn mark_library_documentation_failed(
        &self,
        id: &str,
        message: &str,
    ) -> Result<Option<LibraryEntry>> {
        let Some(mut entry) = self.get_library_entry(id)? else {
            return Ok(None);
        };
        if entry.documentation.is_none() {
            entry.documentation_status = DocumentationStatus::Failed;
        }
        entry.documentation_error = Some(message.chars().take(1_000).collect());
        entry.updated_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.save_library_entry(&entry)?;
        Ok(Some(entry))
    }

    pub fn replace_findings(&self, project_id: &str, findings: &[Finding]) -> Result<Vec<Finding>> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let transaction = connection.transaction()?;
        let existing = {
            let mut statement = transaction
                .prepare("SELECT fingerprint,entry_json FROM findings WHERE project_id=?1")?;
            let rows = statement.query_map([project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.filter_map(|row| row.ok())
                .filter_map(|(fingerprint, json)| {
                    serde_json::from_str::<Finding>(&json)
                        .ok()
                        .map(|finding| (fingerprint, finding))
                })
                .collect::<HashMap<_, _>>()
        };
        let current = findings
            .iter()
            .map(|finding| finding.fingerprint.as_str())
            .collect::<std::collections::HashSet<_>>();
        transaction.execute("DELETE FROM findings WHERE project_id=?1 AND fingerprint NOT IN (SELECT value FROM json_each(?2))", params![project_id, serde_json::to_string(&current)?])?;
        let mut saved = Vec::with_capacity(findings.len());
        for finding in findings {
            let mut value = finding.clone();
            if let Some(previous) = existing.get(&finding.fingerprint) {
                value.status = previous.status;
                value.created_at = previous.created_at;
                value.ai_analysis = previous.ai_analysis.clone();
            }
            transaction.execute("INSERT INTO findings(id,fingerprint,project_id,category,severity,status,entry_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(fingerprint) DO UPDATE SET category=excluded.category,severity=excluded.severity,status=excluded.status,entry_json=excluded.entry_json,updated_at=excluded.updated_at",params![value.id,value.fingerprint,value.project_id,format!("{:?}",value.category).to_lowercase(),format!("{:?}",value.severity).to_lowercase(),format!("{:?}",value.status).to_lowercase(),serde_json::to_string(&value)?,value.created_at,value.updated_at])?;
            saved.push(value);
        }
        transaction.commit()?;
        Ok(saved)
    }

    pub fn finding_scan_is_current(&self, project_id: &str, graph_hash: &str) -> Result<bool> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let current = connection.query_row(
            "SELECT graph_hash FROM finding_scans WHERE project_id=?1",
            [project_id],
            |row| row.get::<_, String>(0),
        );
        match current {
            Ok(current) => Ok(current == graph_hash),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn mark_finding_scan_current(&self, project_id: &str, graph_hash: &str) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let scanned_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        connection.execute(
            "INSERT INTO finding_scans(project_id,graph_hash,scanned_at) VALUES(?1,?2,?3) ON CONFLICT(project_id) DO UPDATE SET graph_hash=excluded.graph_hash,scanned_at=excluded.scanned_at",
            params![project_id, graph_hash, scanned_at],
        )?;
        Ok(())
    }

    pub fn list_findings(&self, project_id: &str, category: Option<&str>) -> Result<Vec<Finding>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let mut statement=connection.prepare("SELECT entry_json FROM findings WHERE project_id=?1 ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END, updated_at DESC")?;
        let rows = statement.query_map([project_id], |row| row.get::<_, String>(0))?;
        let mut values = Vec::new();
        for row in rows {
            let finding: Finding = serde_json::from_str(&row?)?;
            if category.is_none_or(|category| {
                format!("{:?}", finding.category).eq_ignore_ascii_case(category)
            }) {
                values.push(finding)
            }
        }
        Ok(values)
    }

    pub fn get_finding(&self, id: &str) -> Result<Option<Finding>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row("SELECT entry_json FROM findings WHERE id=?1", [id], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn update_finding_status(
        &self,
        id: &str,
        status: FindingStatus,
    ) -> Result<Option<Finding>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let json =
            match connection.query_row("SELECT entry_json FROM findings WHERE id=?1", [id], |row| {
                row.get::<_, String>(0)
            }) {
                Ok(value) => value,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(error) => return Err(error.into()),
            };
        let mut finding: Finding = serde_json::from_str(&json)?;
        finding.status = status;
        finding.updated_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        connection.execute(
            "UPDATE findings SET status=?2,entry_json=?3,updated_at=?4 WHERE id=?1",
            params![
                id,
                format!("{:?}", status).to_lowercase(),
                serde_json::to_string(&finding)?,
                finding.updated_at
            ],
        )?;
        Ok(Some(finding))
    }

    pub fn save_finding_ai_analysis(&self, id: &str, analysis: &str) -> Result<Option<Finding>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let json =
            match connection.query_row("SELECT entry_json FROM findings WHERE id=?1", [id], |row| {
                row.get::<_, String>(0)
            }) {
                Ok(value) => value,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(error) => return Err(error.into()),
            };
        let mut finding: Finding = serde_json::from_str(&json)?;
        finding.ai_analysis = Some(analysis.chars().take(20_000).collect());
        finding.updated_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        connection.execute(
            "UPDATE findings SET entry_json=?2,updated_at=?3 WHERE id=?1",
            params![id, serde_json::to_string(&finding)?, finding.updated_at],
        )?;
        Ok(Some(finding))
    }

    pub fn get_project_documentation(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectDocumentation>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row(
            "SELECT documentation_json FROM project_documentation WHERE project_id=?1",
            [project_id],
            |row| row.get::<_, String>(0),
        ) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save_project_documentation(&self, documentation: &ProjectDocumentation) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute("INSERT INTO project_documentation(project_id,content_hash,documentation_json,provider,model,prompt_version,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(project_id) DO UPDATE SET content_hash=excluded.content_hash,documentation_json=excluded.documentation_json,provider=excluded.provider,model=excluded.model,prompt_version=excluded.prompt_version,updated_at=excluded.updated_at",params![documentation.project_id,documentation.content_hash,serde_json::to_string(documentation)?,documentation.provider,documentation.model,documentation.prompt_version,documentation.created_at,documentation.updated_at])?;
        Ok(())
    }

    pub fn save_detected_features(
        &self,
        project_id: &str,
        features: &[Feature],
    ) -> Result<Vec<Feature>> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let transaction = connection.transaction()?;
        let existing = {
            let mut statement =
                transaction.prepare("SELECT id,entry_json FROM features WHERE project_id=?1")?;
            let rows = statement.query_map([project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.filter_map(|row| row.ok())
                .filter_map(|(id, json)| {
                    serde_json::from_str::<Feature>(&json)
                        .ok()
                        .map(|feature| (id, feature))
                })
                .collect::<HashMap<_, _>>()
        };
        let current = features
            .iter()
            .map(|feature| feature.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        for previous in existing.values().filter(|feature| {
            feature.status == FeatureStatus::Detected && !current.contains(feature.id.as_str())
        }) {
            transaction.execute("DELETE FROM features WHERE id=?1", [&previous.id])?;
        }
        let mut saved = Vec::new();
        for feature in features {
            let mut value = feature.clone();
            if let Some(previous) = existing.get(&feature.id)
                && matches!(
                    previous.status,
                    FeatureStatus::Accepted | FeatureStatus::Edited | FeatureStatus::Ignored
                )
            {
                value.status = previous.status;
                value.name = previous.name.clone();
                value.description = previous.description.clone();
                value.created_at = previous.created_at;
            }
            transaction.execute("INSERT INTO features(id,project_id,name,status,source_hash,entry_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET name=excluded.name,status=excluded.status,source_hash=excluded.source_hash,entry_json=excluded.entry_json,updated_at=excluded.updated_at",params![value.id,value.project_id,value.name,format!("{:?}",value.status).to_lowercase(),value.source_hash,serde_json::to_string(&value)?,value.created_at,value.updated_at])?;
            transaction.execute("DELETE FROM feature_nodes WHERE feature_id=?1", [&value.id])?;
            transaction.execute("DELETE FROM feature_edges WHERE feature_id=?1", [&value.id])?;
            for membership in &value.memberships {
                transaction.execute("INSERT INTO feature_nodes(feature_id,node_id,confidence,reason,source) VALUES(?1,?2,?3,?4,?5)",params![value.id,membership.node_id,membership.confidence,membership.reason,format!("{:?}",membership.source).to_lowercase()])?;
            }
            for edge_id in &value.edge_ids {
                transaction.execute(
                    "INSERT INTO feature_edges(feature_id,edge_id) VALUES(?1,?2)",
                    params![value.id, edge_id],
                )?;
            }
            saved.push(value);
        }
        transaction.commit()?;
        Ok(saved)
    }
    pub fn list_features(&self, project_id: &str) -> Result<Vec<Feature>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let mut statement = connection.prepare(
            "SELECT entry_json FROM features WHERE project_id=?1 ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([project_id], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn get_feature(&self, id: &str) -> Result<Option<Feature>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row("SELECT entry_json FROM features WHERE id=?1", [id], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    pub fn save_feature(&self, feature: &Feature) -> Result<()> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO features(id,project_id,name,status,source_hash,entry_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,status=excluded.status,source_hash=excluded.source_hash,entry_json=excluded.entry_json,updated_at=excluded.updated_at",
            params![
                feature.id,
                feature.project_id,
                feature.name,
                format!("{:?}", feature.status).to_lowercase(),
                feature.source_hash,
                serde_json::to_string(feature)?,
                feature.created_at,
                feature.updated_at
            ],
        )?;
        transaction.execute(
            "DELETE FROM feature_nodes WHERE feature_id=?1",
            [&feature.id],
        )?;
        transaction.execute(
            "DELETE FROM feature_edges WHERE feature_id=?1",
            [&feature.id],
        )?;
        for membership in &feature.memberships {
            transaction.execute(
                "INSERT INTO feature_nodes(feature_id,node_id,confidence,reason,source) VALUES(?1,?2,?3,?4,?5)",
                params![
                    feature.id,
                    membership.node_id,
                    membership.confidence,
                    membership.reason,
                    format!("{:?}", membership.source).to_lowercase()
                ],
            )?;
        }
        for edge_id in &feature.edge_ids {
            transaction.execute(
                "INSERT INTO feature_edges(feature_id,edge_id) VALUES(?1,?2)",
                params![feature.id, edge_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn update_feature(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        status: Option<FeatureStatus>,
    ) -> Result<Option<Feature>> {
        let Some(mut feature) = self.get_feature(id)? else {
            return Ok(None);
        };
        if let Some(name) = name.map(str::trim).filter(|value| !value.is_empty()) {
            feature.name = name.into();
            feature.status = FeatureStatus::Edited
        }
        if let Some(description) = description {
            feature.description = description.into();
            feature.status = FeatureStatus::Edited
        }
        if let Some(status) = status {
            feature.status = status
        }
        feature.updated_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "UPDATE features SET name=?2,status=?3,entry_json=?4,updated_at=?5 WHERE id=?1",
            params![
                id,
                feature.name,
                format!("{:?}", feature.status).to_lowercase(),
                serde_json::to_string(&feature)?,
                feature.updated_at
            ],
        )?;
        Ok(Some(feature))
    }
    pub fn save_feature_spec(&self, feature_id: &str, spec: &FeatureSpec) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        connection.execute("INSERT INTO feature_specs(feature_id,spec_json,provenance,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(feature_id) DO UPDATE SET spec_json=excluded.spec_json,provenance=excluded.provenance,updated_at=excluded.updated_at",params![feature_id,serde_json::to_string(spec)?,spec.provenance,now])?;
        Ok(())
    }
    pub fn get_feature_spec(&self, feature_id: &str) -> Result<Option<FeatureSpec>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row(
            "SELECT spec_json FROM feature_specs WHERE feature_id=?1",
            [feature_id],
            |row| row.get::<_, String>(0),
        ) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    pub fn save_feature_documentation(&self, documentation: &FeatureDocumentation) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        connection.execute("INSERT INTO feature_documentation(feature_id,content_hash,documentation_json,prompt_version,updated_at) VALUES(?1,?2,?3,'feature-docs-v1',?4) ON CONFLICT(feature_id) DO UPDATE SET content_hash=excluded.content_hash,documentation_json=excluded.documentation_json,prompt_version=excluded.prompt_version,updated_at=excluded.updated_at",params![documentation.feature_id,documentation.content_hash,serde_json::to_string(documentation)?,now])?;
        Ok(())
    }
    pub fn get_feature_documentation(
        &self,
        feature_id: &str,
    ) -> Result<Option<FeatureDocumentation>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row(
            "SELECT documentation_json FROM feature_documentation WHERE feature_id=?1",
            [feature_id],
            |row| row.get::<_, String>(0),
        ) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    pub fn get_ai_cache(&self, cache_key: &str) -> Result<Option<serde_json::Value>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row(
            "SELECT result_json FROM ai_cache WHERE cache_key=?1",
            [cache_key],
            |row| row.get::<_, String>(0),
        ) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    pub fn save_ai_cache(&self, entry: &AiCacheWrite<'_>) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        connection.execute("INSERT INTO ai_cache(cache_key,project_id,analysis_type,content_hash,prompt_version,provider,model,result_json,input_tokens,output_tokens,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11) ON CONFLICT(cache_key) DO UPDATE SET result_json=excluded.result_json,input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,updated_at=excluded.updated_at",params![entry.cache_key,entry.project_id,entry.analysis_type,entry.content_hash,entry.prompt_version,entry.provider,entry.model,serde_json::to_string(entry.result)?,entry.input_tokens.map(|value|value as i64),entry.output_tokens.map(|value|value as i64),now])?;
        Ok(())
    }
    pub fn save_feature_library_entry(&self, entry: &FeatureLibraryEntry) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute("INSERT INTO feature_library(id,source_feature_id,source_project_id,bundle_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET bundle_json=excluded.bundle_json,updated_at=excluded.updated_at",params![entry.id,entry.source_feature_id,entry.source_project_id,serde_json::to_string(entry)?,entry.created_at,entry.updated_at])?;
        Ok(())
    }
    pub fn list_feature_library(&self) -> Result<Vec<FeatureLibraryEntry>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let mut statement = connection
            .prepare("SELECT bundle_json FROM feature_library ORDER BY updated_at DESC")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn delete_feature_library_entry(&self, id: &str) -> Result<bool> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        Ok(connection.execute("DELETE FROM feature_library WHERE id=?1", [id])? > 0)
    }

    pub fn save_api_environment(&self, environment: &ApiEnvironment) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "INSERT INTO api_environments(id,project_id,name,kind,entry_json,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,kind=excluded.kind,entry_json=excluded.entry_json,updated_at=excluded.updated_at",
            params![environment.id,environment.project_id,environment.name,format!("{:?}",environment.kind).to_lowercase(),serde_json::to_string(environment)?,environment.created_at,environment.updated_at],
        )?;
        Ok(())
    }

    pub fn list_api_environments(&self, project_id: &str) -> Result<Vec<ApiEnvironment>> {
        self.list_json(
            "SELECT entry_json FROM api_environments WHERE project_id=?1 ORDER BY updated_at DESC",
            project_id,
        )
    }

    pub fn save_api_request(&self, request: &ApiSavedRequest) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "INSERT INTO api_saved_requests(id,project_id,collection_id,name,favorite,entry_json,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET collection_id=excluded.collection_id,name=excluded.name,favorite=excluded.favorite,entry_json=excluded.entry_json,updated_at=excluded.updated_at",
            params![request.id,request.project_id,request.collection_id,request.name,request.favorite,serde_json::to_string(request)?,request.created_at,request.updated_at],
        )?;
        Ok(())
    }

    pub fn list_api_requests(&self, project_id: &str) -> Result<Vec<ApiSavedRequest>> {
        self.list_json(
            "SELECT entry_json FROM api_saved_requests WHERE project_id=?1 ORDER BY favorite DESC,updated_at DESC",
            project_id,
        )
    }

    pub fn get_api_request(&self, id: &str) -> Result<Option<ApiSavedRequest>> {
        self.get_json("SELECT entry_json FROM api_saved_requests WHERE id=?1", id)
    }

    pub fn save_api_history(&self, history: &ApiHistoryEntry) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "INSERT INTO api_request_history(id,project_id,endpoint_id,method,url,status,duration_ms,entry_json,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![history.id,history.project_id,history.endpoint_id,history.method,history.url,history.status,history.duration_ms.map(|value|value as i64),serde_json::to_string(history)?,history.created_at],
        )?;
        Ok(())
    }

    pub fn list_api_history(&self, project_id: &str) -> Result<Vec<ApiHistoryEntry>> {
        self.list_json(
            "SELECT entry_json FROM api_request_history WHERE project_id=?1 ORDER BY created_at DESC LIMIT 500",
            project_id,
        )
    }

    pub fn save_api_collection(&self, collection: &ApiCollection) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "INSERT INTO api_collections(id,project_id,name,entry_json,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,entry_json=excluded.entry_json,updated_at=excluded.updated_at",
            params![collection.id,collection.project_id,collection.name,serde_json::to_string(collection)?,collection.created_at,collection.updated_at],
        )?;
        Ok(())
    }

    pub fn list_api_collections(&self, project_id: &str) -> Result<Vec<ApiCollection>> {
        self.list_json(
            "SELECT entry_json FROM api_collections WHERE project_id=?1 ORDER BY updated_at DESC",
            project_id,
        )
    }

    pub fn get_api_collection(&self, id: &str) -> Result<Option<ApiCollection>> {
        self.get_json("SELECT entry_json FROM api_collections WHERE id=?1", id)
    }

    pub fn save_api_contract_test(&self, test: &ApiContractTest) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "INSERT INTO api_contract_tests(id,project_id,name,entry_json,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,entry_json=excluded.entry_json,updated_at=excluded.updated_at",
            params![test.id,test.project_id,test.name,serde_json::to_string(test)?,test.created_at,test.updated_at],
        )?;
        Ok(())
    }

    pub fn list_api_contract_tests(&self, project_id: &str) -> Result<Vec<ApiContractTest>> {
        self.list_json(
            "SELECT entry_json FROM api_contract_tests WHERE project_id=?1 ORDER BY updated_at DESC",
            project_id,
        )
    }

    pub fn get_api_contract_test(&self, id: &str) -> Result<Option<ApiContractTest>> {
        self.get_json("SELECT entry_json FROM api_contract_tests WHERE id=?1", id)
    }

    pub fn save_api_contract_run(&self, project_id: &str, run: &ApiContractRun) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        connection.execute(
            "INSERT INTO api_test_runs(id,project_id,test_id,passed,entry_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![run.id,project_id,run.test_id,run.passed,serde_json::to_string(run)?,run.created_at],
        )?;
        Ok(())
    }

    pub fn save_openapi_document(
        &self,
        project_id: &str,
        document: &serde_json::Value,
    ) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        connection.execute(
            "INSERT INTO api_openapi_specs(project_id,document_json,updated_at) VALUES(?1,?2,?3)
             ON CONFLICT(project_id) DO UPDATE SET document_json=excluded.document_json,updated_at=excluded.updated_at",
            params![project_id,serde_json::to_string(document)?,now],
        )?;
        Ok(())
    }

    pub fn get_openapi_document(&self, project_id: &str) -> Result<Option<serde_json::Value>> {
        self.get_json(
            "SELECT document_json FROM api_openapi_specs WHERE project_id=?1",
            project_id,
        )
    }

    fn list_json<T: serde::de::DeserializeOwned>(
        &self,
        query: &str,
        value: &str,
    ) -> Result<Vec<T>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        let mut statement = connection.prepare(query)?;
        let rows = statement.query_map([value], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        query: &str,
        value: &str,
    ) -> Result<Option<T>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?;
        match connection.query_row(query, [value], |row| row.get::<_, String>(0)) {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

fn git_metadata(root: &Path) -> (Option<String>, Option<String>) {
    fn value(root: &Path, arguments: &[&str]) -> Option<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?.trim().to_string();
        (!text.is_empty()).then_some(text)
    }
    (
        value(root, &["config", "--get", "remote.origin.url"]),
        value(root, &["rev-parse", "--abbrev-ref", "HEAD"]),
    )
}
