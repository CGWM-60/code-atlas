use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::language::ProgrammingLanguage;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceScope {
    #[default]
    Project,
    WorkspacePackage,
    ExternalDependency,
    Generated,
}

impl SourceScope {
    pub fn classify(path: &str) -> Self {
        let normalized = path.replace('\\', "/").to_lowercase();
        let segments = normalized.split('/').collect::<Vec<_>>();
        if segments.iter().any(|segment| {
            matches!(
                *segment,
                "node_modules" | "vendor" | ".venv" | "site-packages" | "pods" | "registry"
            )
        }) {
            Self::ExternalDependency
        } else if segments
            .iter()
            .any(|segment| matches!(*segment, "build" | "dist" | "generated" | "codegen"))
            || normalized.ends_with(".g.dart")
            || normalized.contains(".generated.")
        {
            Self::Generated
        } else if segments
            .first()
            .is_some_and(|segment| matches!(*segment, "apps" | "packages" | "crates" | "workspace"))
        {
            Self::WorkspacePackage
        } else {
            Self::Project
        }
    }

    pub fn library_eligible(self) -> bool {
        matches!(self, Self::Project | Self::WorkspacePackage)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::WorkspacePackage => "workspace_package",
            Self::ExternalDependency => "external_dependency",
            Self::Generated => "generated",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    // chemin relatif
    pub path: String,
    // extension des fichier
    pub extension: Option<String>,

    pub language: ProgrammingLanguage,
    // taille du fichier
    pub size: u64,

    /// SHA-256 du contenu. Il permet de réutiliser une analyse AST inchangée.
    pub hash: String,

    #[serde(default)]
    pub source_scope: SourceScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScan {
    // racine du project analyser
    pub root: String,

    // nbr total de fichier
    pub total_files: usize,

    pub total_bytes: u64,

    // repartitions des extentiosn
    pub extension_counts: BTreeMap<String, usize>,

    pub language_counts: BTreeMap<String, usize>,
    /// tous les fichier detecter
    pub files: Vec<ProjectFile>,
}
