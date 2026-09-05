use std::path::Path;

use anyhow::{Context, Result};

use cargo_metadata::{MetadataCommand, Target, TargetKind};

use crate::model::cargo::{CargoPackageInfo, CargoTargetInfo, CargoTargetKind, CargoWorkspaceInfo};

/// Convertit un Target Cargo externe
/// vers le type utilisé par Code Atlas.
///
///
/// Un Target peut posséder plusieurs valeurs
/// dans `target.kind`.
///
/// Pour notre représentation principale,
/// nous cherchons le type le plus significatif.
///
///
/// # Paramètre
///
///     target: &Target
///
/// Nous empruntons le Target reçu
/// de cargo_metadata.
///
///
/// # Retour
///
///     CargoTargetKind
fn detect_target_kind(target: &Target) -> CargoTargetKind {
    // Nous parcourons toutes les catégories
    // associées au target.
    for kind in &target.kind {
        match kind {
            TargetKind::Bin => {
                return CargoTargetKind::Binary;
            }

            TargetKind::Lib
            | TargetKind::RLib
            | TargetKind::DyLib
            | TargetKind::CDyLib
            | TargetKind::StaticLib => {
                return CargoTargetKind::Library;
            }

            TargetKind::ProcMacro => {
                return CargoTargetKind::ProcMacro;
            }

            TargetKind::Test => {
                return CargoTargetKind::Test;
            }

            TargetKind::Example => {
                return CargoTargetKind::Example;
            }

            TargetKind::Bench => {
                return CargoTargetKind::Bench;
            }

            TargetKind::CustomBuild => {
                return CargoTargetKind::BuildScript;
            }

            // TargetKind est non-exhaustif.
            //
            // Cela signifie qu'une future version
            // de Cargo peut ajouter de nouvelles variantes.
            //
            // `_` nous empêche donc de casser
            // si cela arrive.
            _ => {}
        }
    }

    CargoTargetKind::Unknown
}
/// Essaie de transformer un chemin absolu
/// en chemin relatif au projet.
///
///
/// Exemple :
///
/// project_root :
///
///     /Users/julien/code-atlas
///
///
/// source_path :
///
///     /Users/julien/code-atlas/src/main.rs
///
///
/// résultat :
///
///     src/main.rs
fn make_relative_path(project_root: &Path, source_path: &Path) -> String {
    // strip_prefix() tente de retirer :
    //
    // /Users/julien/code-atlas
    //
    // du chemin :
    //
    // /Users/julien/code-atlas/src/main.rs
    //
    // pour obtenir :
    //
    // src/main.rs
    if let Ok(relative) = source_path.strip_prefix(project_root) {
        return relative.to_string_lossy().to_string();
    }

    // Si le chemin n'est pas situé
    // dans notre projet,
    // nous conservons le chemin complet.
    source_path.to_string_lossy().to_string()
}
/// Convertit un Target de cargo_metadata
/// en CargoTargetInfo Code Atlas.
///
///
/// # `target`
///
/// Target fourni par Cargo.
///
///
/// # `project_root`
///
/// Racine de notre projet analysé,
/// utilisée pour rendre le chemin relatif.
fn convert_target(target: &Target, project_root: &Path) -> CargoTargetInfo {
    // target.src_path utilise Utf8PathBuf.
    //
    // `.as_std_path()`
    //
    // permet de le regarder comme
    // un std::path::Path classique.
    let source_path = make_relative_path(project_root, target.src_path.as_std_path());

    CargoTargetInfo {
        name: target.name.clone(),

        kind: detect_target_kind(target),

        source_path,

        // Edition implémente Display.
        //
        // Exemple :
        //
        // Edition::E2021
        //
        // deviendra :
        //
        // "2021"
        edition: target.edition.to_string(),
    }
}
/// Analyse la structure Cargo
/// d'un projet Rust.
///
///
/// Cette fonction ne lit PAS manuellement
/// Cargo.toml.
///
/// Elle demande directement à Cargo
/// de nous fournir sa compréhension du projet.
///
///
/// Cela permet de gérer proprement :
///
/// - package simple
/// - workspace
/// - plusieurs crates
/// - lib
/// - bin
/// - src/bin
/// - examples
/// - tests
/// - build.rs
///
///
/// # Paramètre
///
///     project_root: &Path
///
/// Racine du projet transmis à Code Atlas.
///
///
/// # Retour
///
///     Result<Option<CargoWorkspaceInfo>>
///
/// Pourquoi Option ?
///
/// Parce qu'un projet analysé par Code Atlas
/// peut ne pas être un projet Cargo.
///
/// Exemple :
///
/// projet PHP
///
/// Dans ce cas :
///
///     Ok(None)
///
///
/// Si Cargo existe et comprend le projet :
///
///     Ok(Some(...))
///
///
/// Si un vrai projet Cargo est trouvé
/// mais que Cargo échoue :
///
///     Err(...)
pub fn analyze_cargo_project(project_root: &Path) -> Result<Option<CargoWorkspaceInfo>> {
    // ================================================
    // ÉTAPE 1
    //
    // CHERCHER Cargo.toml
    // ================================================

    let manifest_path = project_root.join("Cargo.toml");

    // Aucun Cargo.toml à cette racine.
    //
    // Cela ne représente PAS une erreur.
    //
    // Le projet peut simplement être :
    //
    // PHP
    // JavaScript
    // Python
    // etc.
    if !manifest_path.is_file() {
        return Ok(None);
    }

    // ================================================
    // ÉTAPE 2
    //
    // DEMANDER LES MÉTADONNÉES À CARGO
    // ================================================

    let mut command = MetadataCommand::new();

    // Nous indiquons explicitement
    // quel Cargo.toml analyser.
    command.manifest_path(&manifest_path);

    // exec() exécute cargo metadata
    // et transforme le JSON obtenu
    // en structures Rust.
    let metadata = command
        .exec()
        .context("Impossible de récupérer les métadonnées Cargo")?;

    // ================================================
    // ÉTAPE 3
    //
    // IDENTIFIER LES MEMBRES DU WORKSPACE
    // ================================================

    // cargo metadata peut contenir :
    //
    // notre package
    // +
    // ses dépendances
    //
    // mais nous ne voulons pas transformer
    // toutes les dépendances crates.io
    // en packages internes du projet.
    //
    // Metadata::workspace_packages()
    //
    // retourne uniquement les packages
    // appartenant au workspace actuel.
    let workspace_packages = metadata.workspace_packages();

    let mut packages = Vec::new();

    // ================================================
    // ÉTAPE 4
    //
    // CONVERTIR CHAQUE PACKAGE
    // ================================================

    for package in workspace_packages {
        let mut targets = Vec::new();

        // Un package peut fournir plusieurs targets.
        //
        // Exemple :
        //
        // lib
        // bin
        // test
        // example
        for target in &package.targets {
            targets.push(convert_target(target, project_root));
        }

        let manifest_relative =
            make_relative_path(project_root, package.manifest_path.as_std_path());

        packages.push(CargoPackageInfo {
            // PackageId est volontairement opaque.
            //
            // Nous le convertissons simplement
            // en texte pour notre ID interne.
            id: package.id.to_string(),

            name: package.name.to_string(),

            version: package.version.to_string(),

            edition: package.edition.to_string(),

            manifest_path: manifest_relative,

            targets,
        });
    }

    // ================================================
    // ÉTAPE 5
    //
    // CONSTRUIRE LE RÉSULTAT CODE ATLAS
    // ================================================

    Ok(Some(CargoWorkspaceInfo {
        workspace_root: metadata.workspace_root.to_string(),

        target_directory: metadata.target_directory.to_string(),

        packages,
    }))
}
