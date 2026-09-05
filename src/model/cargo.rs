use serde::{Deserialize, Serialize};

/// Représente la nature principale
/// d'un target Cargo.
///
/// Un target correspond à quelque chose
/// que Cargo peut compiler.
///
/// Exemple :
///
/// src/main.rs
///
/// correspond généralement à :
///
/// CargoTargetKind::Binary
///
///
/// src/lib.rs
///
/// correspond généralement à :
///
/// CargoTargetKind::Library
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CargoTargetKind {
    /// Exécutable Rust.
    ///
    /// Exemple :
    ///
    /// src/main.rs
    Binary,

    /// Bibliothèque Rust.
    ///
    /// Exemple :
    ///
    /// src/lib.rs
    Library,

    /// Macro procédurale.
    ProcMacro,

    /// Target utilisé par `cargo test`.
    Test,

    /// Exemple Cargo.
    ///
    /// Généralement :
    ///
    /// examples/quelque_chose.rs
    Example,

    /// Benchmark.
    Bench,

    /// Script build.rs.
    BuildScript,

    /// Target que notre version de Code Atlas
    /// ne connaît pas encore.
    Unknown,
}
impl CargoTargetKind {
    /// Retourne une représentation textuelle
    /// stable du type de target.
    ///
    /// Nous allons notamment l'utiliser
    /// pour construire des identifiants.
    ///
    /// Exemple :
    ///
    /// CargoTargetKind::Binary.as_str()
    ///
    /// retourne :
    ///
    /// "binary"
    pub fn as_str(&self) -> &'static str {
        match self {
            CargoTargetKind::Binary => "binary",

            CargoTargetKind::Library => "library",

            CargoTargetKind::ProcMacro => "proc_macro",

            CargoTargetKind::Test => "test",

            CargoTargetKind::Example => "example",

            CargoTargetKind::Bench => "bench",

            CargoTargetKind::BuildScript => "build_script",

            CargoTargetKind::Unknown => "unknown",
        }
    }
}
/// Représente un target Cargo
/// dans notre modèle Code Atlas.
///
///
/// Exemple :
///
/// Cargo peut nous dire :
///
/// nom :
///     code-atlas
///
/// type :
///     bin
///
/// fichier source :
///     /Users/.../src/main.rs
///
///
/// Nous convertissons cela
/// dans cette structure générique.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoTargetInfo {
    /// Nom donné au target.
    ///
    /// Exemple :
    ///
    ///     code-atlas
    pub name: String,

    /// Nature du target.
    pub kind: CargoTargetKind,

    /// Fichier source principal.
    ///
    /// Exemple :
    ///
    ///     src/main.rs
    ///
    /// ou :
    ///
    ///     src/lib.rs
    ///
    /// Nous essayerons de conserver
    /// un chemin relatif au projet.
    pub source_path: String,

    /// Edition Rust utilisée.
    ///
    /// Exemple :
    ///
    ///     "2021"
    ///
    /// ou :
    ///
    ///     "2024"
    ///
    /// Nous stockons une String
    /// afin que Code Atlas ne dépende pas
    /// du type Edition de cargo_metadata.
    pub edition: String,
}
/// Représente un package Cargo.
///
/// Un package correspond normalement
/// à un Cargo.toml.
///
///
/// Exemple :
///
/// workspace/
/// ├── backend/
/// │   └── Cargo.toml
/// └── cli/
///     └── Cargo.toml
///
/// donnera deux CargoPackageInfo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoPackageInfo {
    /// Identifiant Cargo du package.
    ///
    /// Cargo utilise un PackageId opaque.
    ///
    /// Nous le conservons uniquement comme String
    /// pour identifier le package.
    pub id: String,

    /// Nom du package.
    ///
    /// Exemple :
    ///
    ///     "code-atlas"
    pub name: String,

    /// Version.
    ///
    /// Exemple :
    ///
    ///     "0.1.0"
    pub version: String,

    /// Edition Rust par défaut du package.
    pub edition: String,

    /// Chemin vers son Cargo.toml.
    pub manifest_path: String,

    /// Targets fournis par ce package.
    ///
    /// Un package peut avoir plusieurs targets.
    ///
    /// Exemple :
    ///
    /// lib
    /// +
    /// bin
    pub targets: Vec<CargoTargetInfo>,
}
/// Représente la structure Cargo globale
/// découverte dans un projet.
///
///
/// Exemple workspace :
///
/// my-workspace/
/// │
/// ├── Cargo.toml
/// │
/// ├── backend/
/// │   └── Cargo.toml
/// │
/// └── cli/
///     └── Cargo.toml
///
///
/// deviendra :
///
/// CargoWorkspaceInfo
/// │
/// ├── backend
/// └── cli
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoWorkspaceInfo {
    /// Racine du workspace renvoyée par Cargo.
    pub workspace_root: String,

    /// Dossier target utilisé par Cargo.
    ///
    /// Exemple :
    ///
    ///     /projet/target
    pub target_directory: String,

    /// Tous les packages appartenant
    /// réellement au workspace.
    ///
    /// Important :
    ///
    /// `cargo metadata` peut également fournir
    /// les packages des dépendances.
    ///
    /// Nous allons filtrer uniquement
    /// les membres du workspace.
    pub packages: Vec<CargoPackageInfo>,
}
