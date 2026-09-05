use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::model::{
    module::{ModuleReference, ModuleResolutionStatus, ResolvedModule},
    project::ProjectFile,
};

/// Détermine le chemin logique Rust
/// correspondant à un fichier.
///
/// Cette fonction suppose pour l'instant
/// une crate Rust classique située dans `src/`.
///
///
/// Exemple :
///
///     src/main.rs
///
/// devient :
///
///     crate
///
///
/// Exemple :
///
///     src/auth.rs
///
/// devient :
///
///     crate::auth
///
///
/// Exemple :
///
///     src/services/user.rs
///
/// devient :
///
///     crate::services::user
///
///
/// Exemple ancien style :
///
///     src/services/mod.rs
///
/// devient :
///
///     crate::services
fn rust_module_path_for_file(relative_path: &str) -> Option<String> {
    let path = Path::new(relative_path);

    // Pour cette première version,
    // nous travaillons avec la convention Cargo :
    //
    // src/...
    //
    // strip_prefix() transforme par exemple :
    //
    // src/services/user.rs
    //
    // en :
    //
    // services/user.rs
    let inside_src = path.strip_prefix("src").ok()?;

    // Nom du fichier :
    //
    // user.rs
    // mod.rs
    // main.rs
    let file_name = inside_src.file_name()?.to_str()?;

    // Le crate root est particulier.
    if file_name == "main.rs" || file_name == "lib.rs" {
        return Some("crate".to_string());
    }

    let mut segments: Vec<String> = Vec::new();

    // On ajoute d'abord les dossiers parents.
    //
    // Exemple :
    //
    // services/user.rs
    //
    // parent :
    //
    // services
    if let Some(parent) = inside_src.parent() {
        for component in parent.components() {
            let text = component.as_os_str().to_str()?.to_string();

            if !text.is_empty() {
                segments.push(text);
            }
        }
    }

    // mod.rs est particulier.
    //
    // src/services/mod.rs
    //
    // représente déjà :
    //
    // crate::services
    //
    // Nous ne voulons donc surtout pas produire :
    //
    // crate::services::mod
    if file_name != "mod.rs" {
        let stem = inside_src.file_stem()?.to_str()?.to_string();

        segments.push(stem);
    }

    if segments.is_empty() {
        return Some("crate".to_string());
    }

    Some(format!("crate::{}", segments.join("::")))
}

/// Détermine le dossier de base dans lequel
/// Rust cherche les sous-modules d'un fichier.
///
///
/// CAS 1
///
///     src/main.rs
///
/// contenant :
///
///     mod auth;
///
/// Rust cherche dans :
///
///     src/auth.rs
///
/// ou :
///
///     src/auth/mod.rs
///
/// Donc la base est :
///
///     src
///
///
/// CAS 2
///
///     src/model/mod.rs
///
/// contenant :
///
///     mod user;
///
/// Rust cherche :
///
///     src/model/user.rs
///
/// Donc la base est :
///
///     src/model
///
///
/// CAS 3
///
///     src/services.rs
///
/// contenant :
///
///     mod user;
///
/// Rust cherche :
///
///     src/services/user.rs
///
/// Donc la base est :
///
///     src/services
fn child_module_base_directory(source_path: &str) -> Option<PathBuf> {
    let path = Path::new(source_path);

    let parent = path.parent()?;

    let file_name = path.file_name()?.to_str()?;

    // Les crate roots et les mod.rs
    // utilisent directement leur dossier parent.
    if file_name == "main.rs" || file_name == "lib.rs" || file_name == "mod.rs" {
        return Some(parent.to_path_buf());
    }

    // Pour :
    //
    // src/services.rs
    //
    // on veut :
    //
    // src/services
    let stem = path.file_stem()?.to_str()?;

    Some(parent.join(stem))
}
/// Construit le chemin logique complet
/// d'une déclaration `mod`.
///
///
/// Exemple :
///
/// fichier :
///
///     src/main.rs
///
/// déclaration :
///
///     mod auth;
///
/// résultat :
///
///     crate::auth
///
///
/// Exemple :
///
/// fichier :
///
///     src/model/mod.rs
///
/// déclaration :
///
///     mod node;
///
/// résultat :
///
///     crate::model::node
fn build_declared_module_path(module: &ModuleReference) -> Option<String> {
    // Module logique correspondant
    // au fichier actuel.
    let source_module = rust_module_path_for_file(&module.source_path)?;

    // Nous construisons progressivement
    // les segments.
    let mut result = source_module;

    // Ajouter les éventuels parents inline.
    //
    // Exemple :
    //
    // crate
    //
    // +
    //
    // api
    // auth
    //
    // =
    //
    // crate::api::auth
    for parent in &module.inline_parent_modules {
        result.push_str("::");

        result.push_str(parent);
    }

    // Puis le module actuel.
    result.push_str("::");

    result.push_str(&module.module_name);

    Some(result)
}
/// Résout toutes les déclarations `mod`
/// trouvées dans un projet.
///
///
/// # `modules`
///
/// Déclarations découvertes dans l'AST.
///
///
/// # `project_files`
///
/// Tous les fichiers physiques découverts
/// par notre scanner.
///
///
/// # Retour
///
/// Une collection de ResolvedModule.
pub fn resolve_rust_modules(
    modules: &[ModuleReference],
    project_files: &[ProjectFile],
) -> Vec<ResolvedModule> {
    // Nous construisons un HashSet
    // de tous les chemins existants.
    //
    // Exemple :
    //
    // src/main.rs
    // src/auth.rs
    // src/model/mod.rs
    //
    //
    // Pourquoi HashSet ?
    //
    // Parce que nous allons très souvent demander :
    //
    // "ce fichier existe-t-il ?"
    let existing_files: HashSet<&str> = project_files
        .iter()
        .map(|file| file.path.as_str())
        .collect();

    let mut resolved_modules = Vec::new();

    for module in modules {
        let Some(module_path) = build_declared_module_path(module) else {
            // Si nous ne savons même pas
            // construire le chemin logique,
            // nous évitons d'inventer.
            continue;
        };

        // ================================================
        // MODULE INLINE
        // ================================================
        //
        // mod auth {
        //
        // }
        //
        // Aucun fichier externe à chercher.
        if module.is_inline {
            resolved_modules.push(ResolvedModule {
                module_path,

                module_name: module.module_name.clone(),

                declared_in: module.source_path.clone(),

                line: module.line,

                target_path: None,

                status: ModuleResolutionStatus::Inline,
            });

            continue;
        }

        // ================================================
        // MODULE EXTERNE
        // ================================================

        let Some(mut base_directory) = child_module_base_directory(&module.source_path) else {
            continue;
        };

        // Si le module se trouve dans
        // des modules inline parents :
        //
        // mod api {
        //     mod auth;
        // }
        //
        // il faut ajouter :
        //
        // api/
        for inline_parent in &module.inline_parent_modules {
            base_directory = base_directory.join(inline_parent);
        }

        // Première possibilité :
        //
        // auth.rs
        let candidate_file = base_directory.join(format!("{}.rs", module.module_name));

        // Deuxième possibilité :
        //
        // auth/mod.rs
        let candidate_mod_rs = base_directory.join(&module.module_name).join("mod.rs");

        // Transformation PathBuf -> String.
        let candidate_file_text = candidate_file.to_string_lossy().to_string();

        let candidate_mod_rs_text = candidate_mod_rs.to_string_lossy().to_string();

        let file_exists = existing_files.contains(candidate_file_text.as_str());

        let mod_rs_exists = existing_files.contains(candidate_mod_rs_text.as_str());

        // ================================================
        // EXACTEMENT UN FICHIER
        // ================================================

        if file_exists && !mod_rs_exists {
            resolved_modules.push(ResolvedModule {
                module_path,

                module_name: module.module_name.clone(),

                declared_in: module.source_path.clone(),

                line: module.line,

                target_path: Some(candidate_file_text),

                status: ModuleResolutionStatus::Resolved,
            });

            continue;
        }

        if !file_exists && mod_rs_exists {
            resolved_modules.push(ResolvedModule {
                module_path,

                module_name: module.module_name.clone(),

                declared_in: module.source_path.clone(),

                line: module.line,

                target_path: Some(candidate_mod_rs_text),

                status: ModuleResolutionStatus::Resolved,
            });

            continue;
        }

        // ================================================
        // DEUX FICHIERS
        // ================================================

        if file_exists && mod_rs_exists {
            resolved_modules.push(ResolvedModule {
                module_path,

                module_name: module.module_name.clone(),

                declared_in: module.source_path.clone(),

                line: module.line,

                target_path: None,

                status: ModuleResolutionStatus::Ambiguous,
            });

            continue;
        }

        // ================================================
        // RIEN TROUVÉ
        // ================================================

        resolved_modules.push(ResolvedModule {
            module_path,

            module_name: module.module_name.clone(),

            declared_in: module.source_path.clone(),

            line: module.line,

            target_path: None,

            status: ModuleResolutionStatus::Missing,
        });
    }

    resolved_modules
}
