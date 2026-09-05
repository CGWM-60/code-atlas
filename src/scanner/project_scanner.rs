use anyhow::{Context, Result};
use ignore::WalkBuilder;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::language::detector::detect_language;
use crate::model::project::{ProjectFile, ProjectScan, SourceScope};
use sha2::{Digest, Sha256};
/*
dossiers que l'on ne veux pas analyser.
*/

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".dart_tool",
    ".idea",
    ".vscode",
    "vendor",
    "coverage",
    ".code-atlas",
    ".playwright-cli",
    "graphify-out",
];

fn is_sensitive(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    name == ".env"
        || name.starts_with(".env.")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.contains("private_key")
        || name.contains("credential")
        || name.contains("secret")
}

pub fn scan_project<P: AsRef<Path>>(project_path: P) -> Result<ProjectScan> {
    scan_project_with_progress(project_path, |_, _, _| {})
}

pub fn scan_project_with_progress<P, F>(project_path: P, mut progress: F) -> Result<ProjectScan>
where
    P: AsRef<Path>,
    F: FnMut(usize, usize, &str),
{
    let root: PathBuf = project_path
        .as_ref()
        .canonicalize()
        .context("Impossible de trouver le dossier du project")?;

    if !root.is_dir() {
        anyhow::bail!("Le chemein fourni n'est pas un dossier")
    }

    let mut files = Vec::new();
    let mut extension_counts = BTreeMap::new();
    let mut total_bytes = 0_u64;
    let mut language_counts = BTreeMap::new();

    let walker = WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();

            !IGNORED_DIRECTORIES.iter().any(|ignored| name == *ignored)
        })
        .build();

    let candidates = walker
        .filter_map(|entry_result| match entry_result {
            Ok(entry) if entry.path().is_file() && !is_sensitive(entry.path()) => {
                Some(entry.into_path())
            }
            Ok(_) => None,
            Err(error) => {
                eprintln!("Erreur pendant le scan :  {error}");
                None
            }
        })
        .collect::<Vec<_>>();
    let candidate_count = candidates.len();

    for (index, path) in candidates.into_iter().enumerate() {
        let metadata = match path.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        let size = metadata.len();

        total_bytes += size;

        let relative_path = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase());

        let language = detect_language(&path);

        let extension_name = extension
            .clone()
            .unwrap_or_else(|| "sans_extension".to_string());

        *extension_counts.entry(extension_name).or_insert(0) += 1;

        let language_name = language.as_str().to_string();
        *language_counts.entry(language_name).or_insert(0) += 1;
        let hash = match std::fs::read(path) {
            Ok(bytes) => Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            Err(_) => continue,
        };

        progress(index + 1, candidate_count, &relative_path);
        files.push(ProjectFile {
            source_scope: SourceScope::classify(&relative_path),
            path: relative_path,
            extension,
            language,
            size,
            hash,
        });
    }
    Ok(ProjectScan {
        root: root.to_string_lossy().to_string(),
        total_files: files.len(),
        total_bytes,
        extension_counts,
        language_counts,
        files,
    })
}
