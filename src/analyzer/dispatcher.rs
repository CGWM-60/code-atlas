use crate::{
    analyzer::{generic_analyzer::analyze_tree_sitter_file, rust_analyzer::analyze_rust_file},
    language::ProgrammingLanguage,
    model::{analysis::FileAnalysis, project::ProjectFile},
};
use anyhow::Result;
use std::path::Path;

/// Selects a deterministic analyzer while keeping the graph core language-neutral.
pub fn analyze_file(project_root: &Path, file: &ProjectFile) -> Result<Option<FileAnalysis>> {
    match file.language {
        ProgrammingLanguage::Rust => analyze_rust_file(project_root, file).map(Some),
        ProgrammingLanguage::TypeScript
        | ProgrammingLanguage::JavaScript
        | ProgrammingLanguage::Php
        | ProgrammingLanguage::Dart
        | ProgrammingLanguage::Python
        | ProgrammingLanguage::Go => analyze_tree_sitter_file(project_root, file).map(Some),
        _ => Ok(None),
    }
}
