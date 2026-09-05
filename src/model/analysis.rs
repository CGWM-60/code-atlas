use crate::model::{
    call::CallReference, import::ImportReference, module::ModuleReference, node::CodeNode,
};

/// Représente le résultat standard
/// de l'analyse d'UN fichier source.
///
/// Très important :
///
/// cette structure n'appartient PAS à Rust.
///
/// Elle appartient à Code Atlas.
///
/// Peu importe que nous analysions :
///
/// - Rust
/// - PHP
/// - TypeScript
/// - Dart
/// - Python
///
/// l'analyseur devra essayer de produire
/// ce format commun.
///
///
/// Cela permet au reste de Code Atlas
/// de ne pas connaître les détails
/// de chaque langage.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FileAnalysis {
    /// Tous les éléments de code découverts.
    ///
    /// Exemple Rust :
    ///
    /// Struct
    /// Function
    /// Method
    ///
    ///
    /// Exemple PHP :
    ///
    /// Class
    /// Method
    /// Function
    ///
    ///
    /// Exemple TypeScript :
    ///
    /// Class
    /// Function
    /// Component
    pub nodes: Vec<CodeNode>,

    /// Tous les appels détectés.
    ///
    /// Exemple :
    ///
    /// login()
    ///
    /// user.save()
    ///
    /// service.create()
    pub calls: Vec<CallReference>,

    /// Tous les imports détectés.
    ///
    /// Rust :
    ///
    ///     use crate::auth::login;
    ///
    ///
    /// TypeScript :
    ///
    ///     import { login } from "./auth";
    ///
    ///
    /// Python :
    ///
    ///     from auth import login
    ///
    ///
    /// PHP :
    ///
    ///     use App\Service\UserService;
    pub imports: Vec<ImportReference>,

    /// Modules déclarés.
    ///
    /// Pour l'instant cette partie est
    /// surtout utilisée par Rust.
    ///
    /// Les autres analyseurs pourront simplement
    /// laisser cette collection vide
    /// s'ils n'utilisent pas cette notion.
    pub modules: Vec<ModuleReference>,
}
impl FileAnalysis {
    /// Crée une analyse totalement vide.
    ///
    /// Cela sera pratique pour certains langages
    /// ou certaines situations.
    ///
    /// Exemple :
    ///
    /// let analysis = FileAnalysis::empty();
    ///
    ///
    /// Le résultat contiendra :
    ///
    /// nodes   = []
    /// calls   = []
    /// imports = []
    /// modules = []
    pub fn empty() -> Self {
        Self::default()
    }
}
