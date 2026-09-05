use std::path::Path;

use crate::{
    language::ProgrammingLanguage,
    model::{
        cargo::{CargoTargetInfo, CargoWorkspaceInfo},
        edge::{CodeEdge, RelationKind},
        node::{CodeNode, NodeKind},
    },
};
/// Représente les éléments de graphe
/// construits grâce aux informations Cargo.
///
/// Nous séparons :
///
/// - nodes
/// - edges
///
/// car ils seront ensuite fusionnés
/// dans le ProjectGraph général.
pub struct CargoGraphParts {
    /// Nodes d'architecture créés :
    ///
    /// Workspace
    /// Package
    /// Crate
    pub nodes: Vec<CodeNode>,

    /// Relations créées :
    ///
    /// Workspace CONTAINS Package
    ///
    /// Package CONTAINS Crate
    ///
    /// Crate ENTRY_POINT File
    pub edges: Vec<CodeEdge>,
}
impl CargoGraphParts {
    /// Crée un résultat vide.
    ///
    /// Utile lorsqu'aucune structure
    /// Cargo n'existe dans le projet.
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}
/// Essaie de produire un nom lisible
/// pour le workspace.
///
/// Exemple :
///
/// /Users/julien/projects/code-atlas
///
/// devient :
///
/// code-atlas
///
///
/// Si aucun nom ne peut être déterminé,
/// nous utilisons simplement :
///
/// Cargo Workspace
fn workspace_display_name(workspace_root: &str) -> String {
    Path::new(workspace_root)
        .file_name()
        // file_name() retourne Option<&OsStr>.
        .and_then(|name| name.to_str())
        // Si nous avons un &str,
        // nous créons une String.
        .map(|name| name.to_string())
        // Fallback.
        .unwrap_or_else(|| "Cargo Workspace".to_string())
}
/// Construit le Node représentant
/// le workspace Cargo.
///
/// Exemple :
///
/// CodeNode {
///
///     kind: Workspace,
///
///     name: "code-atlas",
///
///     ...
/// }
fn build_workspace_node(workspace: &CargoWorkspaceInfo) -> CodeNode {
    let name = workspace_display_name(&workspace.workspace_root);

    // L'identifiant doit être stable
    // dans le projet.
    //
    // Exemple :
    //
    // workspace:cargo:/Users/.../code-atlas
    let id = format!("workspace:cargo:{}", workspace.workspace_root);

    CodeNode {
        id,

        kind: NodeKind::Workspace,

        name,

        // Un Workspace n'appartient
        // pas à un type.
        owner: None,

        // Il ne correspond pas exactement
        // à un fichier source.
        path: None,

        // Le workspace lui-même
        // n'est pas un fichier Rust.
        language: None,

        start_line: None,

        end_line: None,
        source_scope: crate::model::project::SourceScope::Project,
    }
}
/// Construit un Node représentant
/// un Package Cargo.
///
///
/// # Paramètres
///
/// `package_name`
///
/// nom du package.
///
///
/// `manifest_path`
///
/// chemin vers son Cargo.toml.
fn build_package_node(package_name: &str, manifest_path: &str) -> CodeNode {
    // Nous utilisons le manifest dans l'ID
    // plutôt que seulement le nom.
    //
    // Deux packages d'un gros workspace
    // pourraient théoriquement avoir
    // des noms proches.
    let id = format!("package:{}", manifest_path);

    CodeNode {
        id,

        kind: NodeKind::Package,

        name: package_name.to_string(),

        owner: None,

        // Ici le path pointe vers Cargo.toml.
        path: Some(manifest_path.to_string()),

        // Cargo package = univers Rust
        // dans notre système actuel.
        language: Some(ProgrammingLanguage::Rust),

        start_line: None,

        end_line: None,
        source_scope: crate::model::project::SourceScope::WorkspacePackage,
    }
}
/// Construit le Node représentant
/// un Target Cargo.
///
/// Dans notre graphe,
/// nous considérons un Target compilé
/// comme une Crate.
///
///
/// Exemple :
///
/// target :
///
///     name = "cli"
///     kind = Binary
///     source = cli/src/main.rs
///
/// devient :
///
///     NodeKind::Crate
fn build_crate_node(package_manifest: &str, target: &CargoTargetInfo) -> CodeNode {
    // ID exemple :
    //
    // crate:cli/Cargo.toml:binary:cli
    let id = format!(
        "crate:{}:{}:{}",
        package_manifest,
        target.kind.as_str(),
        target.name,
    );

    CodeNode {
        id,

        kind: NodeKind::Crate,

        name: target.name.clone(),

        owner: None,

        // Le path correspond au point
        // d'entrée de la crate.
        path: Some(target.source_path.clone()),

        language: Some(ProgrammingLanguage::Rust),

        start_line: None,

        end_line: None,
        source_scope: crate::model::project::SourceScope::WorkspacePackage,
    }
}
/// Recherche le Node File correspondant
/// au fichier d'entrée d'un target Cargo.
///
///
/// Exemple :
///
/// target.source_path :
///
///     src/main.rs
///
/// nous cherchons :
///
///     CodeNode {
///         kind: File,
///         path: Some("src/main.rs")
///     }
///
///
/// Nous retournons uniquement une cible
/// s'il existe exactement UN candidat.
fn find_target_file_node<'a>(
    target: &CargoTargetInfo,
    file_nodes: &'a [CodeNode],
) -> Option<&'a CodeNode> {
    let mut candidates = file_nodes.iter().filter(|node| {
        let is_file = node.kind == NodeKind::File;

        let same_path = node.path.as_deref() == Some(target.source_path.as_str());

        is_file && same_path
    });

    let first = candidates.next()?;

    // Plusieurs fichiers identiques
    // dans la collection seraient anormaux.
    //
    // Mais comme toujours :
    //
    // ambiguïté = aucune fausse relation.
    if candidates.next().is_some() {
        return None;
    }

    Some(first)
}
/// Transforme la structure Cargo
/// en Nodes et Edges Code Atlas.
///
///
/// Le résultat attendu ressemble à :
///
/// Workspace
///     │
///     │ CONTAINS
///     ▼
/// Package
///     │
///     │ CONTAINS
///     ▼
/// Crate
///     │
///     │ ENTRY_POINT
///     ▼
/// File
pub fn build_cargo_graph(
    workspace: &CargoWorkspaceInfo,
    file_nodes: &[CodeNode],
) -> CargoGraphParts {
    let mut nodes = Vec::new();

    let mut edges = Vec::new();

    // ========================================================
    // WORKSPACE
    // ========================================================

    let workspace_node = build_workspace_node(workspace);

    // Nous avons besoin de son ID
    // après avoir déplacé le Node
    // dans `nodes`.
    //
    // Donc nous clonons uniquement
    // cette petite information.
    let workspace_id = workspace_node.id.clone();

    nodes.push(workspace_node);

    // ========================================================
    // PACKAGES
    // ========================================================

    for package in &workspace.packages {
        let package_node = build_package_node(&package.name, &package.manifest_path);

        let package_id = package_node.id.clone();

        // ----------------------------------------
        // Workspace CONTAINS Package
        // ----------------------------------------

        edges.push(CodeEdge::new(
            workspace_id.clone(),
            package_id.clone(),
            RelationKind::Contains,
        ));

        nodes.push(package_node);

        // ====================================================
        // TARGETS / CRATES
        // ====================================================

        for target in &package.targets {
            let crate_node = build_crate_node(&package.manifest_path, target);

            let crate_id = crate_node.id.clone();

            // ------------------------------------
            // Package CONTAINS Crate
            // ------------------------------------

            edges.push(CodeEdge::new(
                package_id.clone(),
                crate_id.clone(),
                RelationKind::Contains,
            ));

            nodes.push(crate_node);

            // ------------------------------------
            // Crate ENTRY_POINT File
            // ------------------------------------

            if let Some(file_node) = find_target_file_node(target, file_nodes) {
                edges.push(CodeEdge::new(
                    crate_id,
                    file_node.id.clone(),
                    RelationKind::EntryPoint,
                ));
            }
        }
    }

    CargoGraphParts { nodes, edges }
}
