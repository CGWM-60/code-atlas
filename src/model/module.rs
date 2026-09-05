use serde::{Deserialize, Serialize};

/// Représente une déclaration `mod`
/// découverte dans le code Rust.
///
/// Exemple :
///
///     mod auth;
///
/// ou :
///
///     mod auth {
///         ...
///     }
///
///
/// Cette structure décrit ce que nous avons
/// réellement observé dans le code.
///
/// Elle ne dit pas encore si le fichier
/// correspondant existe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleReference {
    /// Fichier dans lequel la déclaration
    /// `mod` a été trouvée.
    ///
    /// Exemple :
    ///
    ///     src/main.rs
    pub source_path: String,

    /// Nom du module.
    ///
    /// Exemple :
    ///
    ///     mod auth;
    ///
    /// donne :
    ///
    ///     "auth"
    pub module_name: String,

    /// Ligne de la déclaration.
    pub line: usize,

    /// Indique si le module possède
    /// directement un corps.
    ///
    /// Exemple :
    ///
    ///     mod auth {
    ///     }
    ///
    /// donne :
    ///
    ///     true
    ///
    ///
    /// Alors que :
    ///
    ///     mod auth;
    ///
    /// donne :
    ///
    ///     false
    pub is_inline: bool,

    /// Modules inline parents éventuels.
    ///
    /// Exemple :
    ///
    ///     mod api {
    ///
    ///         mod auth {
    ///
    ///             mod login;
    ///
    ///         }
    ///     }
    ///
    /// Pour `login`, cette collection contiendra :
    ///
    ///     [
    ///         "api",
    ///         "auth"
    ///     ]
    ///
    /// Cela nous permettra plus tard de construire :
    ///
    ///     crate::api::auth::login
    pub inline_parent_modules: Vec<String>,

    /// Texte original.
    ///
    /// Exemple :
    ///
    ///     "pub mod auth;"
    ///
    /// Utile pour le debug et la future interface.
    pub raw_mod: String,
}
/// État de résolution d'un module.
///
/// Une déclaration `mod auth;` n'est pas
/// automatiquement considérée comme résolue.
///
/// Nous vérifions réellement les fichiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleResolutionStatus {
    /// Nous avons trouvé exactement
    /// un fichier correspondant.
    Resolved,

    /// Le module est défini directement
    /// dans le fichier :
    ///
    ///     mod auth {
    ///     }
    Inline,

    /// Aucun fichier correspondant trouvé.
    Missing,

    /// Plusieurs fichiers correspondent.
    ///
    /// Exemple invalide normalement en Rust :
    ///
    /// auth.rs
    ///
    /// ET
    ///
    /// auth/mod.rs
    ///
    /// Nous refusons de choisir arbitrairement.
    Ambiguous,
}
/// Représente un module après tentative
/// de résolution.
///
/// Exemple :
///
///     mod auth;
///
/// situé dans :
///
///     src/main.rs
///
/// avec :
///
///     src/auth.rs
///
/// donnera approximativement :
///
///     module_path = "crate::auth"
///     target_path = Some("src/auth.rs")
///     status = Resolved
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedModule {
    /// Chemin logique Rust du module.
    ///
    /// Exemple :
    ///
    ///     crate::auth
    ///
    ///     crate::services::users
    pub module_path: String,

    /// Nom simple du module.
    pub module_name: String,

    /// Fichier qui contient la déclaration `mod`.
    ///
    /// Exemple :
    ///
    ///     src/main.rs
    pub declared_in: String,

    /// Ligne de déclaration.
    pub line: usize,

    /// Fichier contenant réellement
    /// le contenu du module externe.
    ///
    /// Exemple :
    ///
    ///     Some("src/auth.rs")
    ///
    ///
    /// Pour un module inline nous gardons :
    ///
    ///     None
    ///
    /// car nous ne voulons pas encore prétendre
    /// que tout le fichier représente ce module.
    pub target_path: Option<String>,

    /// Résultat de la résolution.
    pub status: ModuleResolutionStatus,
}
