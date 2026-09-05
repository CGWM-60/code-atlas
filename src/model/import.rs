use serde::{Deserialize, Serialize};

/// Représente un import Rust découvert
/// dans un fichier.
///
/// Exemple :
///
///     use crate::auth::verify_password;
///
/// sera transformé en une structure
/// contenant notamment :
///
///     full_path:
///         "crate::auth::verify_password"
///
///     imported_name:
///         "verify_password"
///
///     alias:
///         None
///
///
/// Autre exemple :
///
///     use crate::services::UserService as Service;
///
/// donnera :
///
///     full_path:
///         "crate::services::UserService"
///
///     imported_name:
///         "UserService"
///
///     alias:
///         Some("Service")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReference {
    /// Fichier dans lequel le `use`
    /// a été trouvé.
    ///
    /// Exemple :
    ///
    ///     src/main.rs
    pub source_path: String,

    /// Ligne du `use`.
    ///
    /// Tree-sitter compte les lignes à partir
    /// de zéro, mais nous stockons ici
    /// des numéros humains commençant à 1.
    pub line: usize,

    /// Chemin complet importé.
    ///
    /// Exemple :
    ///
    ///     crate::auth::verify_password
    ///
    /// ou :
    ///
    ///     std::collections::HashMap
    pub full_path: String,

    /// Nom réel de l'élément importé.
    ///
    /// Exemple :
    ///
    ///     crate::auth::verify_password
    ///
    /// donne :
    ///
    ///     verify_password
    pub imported_name: String,

    /// Alias éventuel donné par `as`.
    ///
    /// Exemple :
    ///
    ///     use crate::user::User as Account;
    ///
    /// donne :
    ///
    ///     Some("Account")
    ///
    ///
    /// Sans alias :
    ///
    ///     None
    pub alias: Option<String>,

    /// Indique si nous avons :
    ///
    ///     use quelque_chose::*;
    ///
    /// Exemple :
    ///
    ///     use crate::models::*;
    ///
    /// donnera :
    ///
    ///     true
    pub is_wildcard: bool,

    /// Texte complet de la déclaration `use`.
    ///
    /// Exemple :
    ///
    ///     use crate::auth::{login, logout};
    ///
    /// Même si cette déclaration produit
    /// plusieurs ImportReference,
    /// nous conservons le texte original.
    ///
    /// Cela sera très utile pour :
    ///
    /// - le debug
    /// - l'interface
    /// - expliquer une résolution
    pub raw_use: String,
}
impl ImportReference {
    /// Retourne le nom réellement utilisable
    /// dans le fichier courant.
    ///
    ///
    /// Exemple sans alias :
    ///
    ///     use crate::auth::login;
    ///
    /// dans le fichier nous écrivons :
    ///
    ///     login();
    ///
    /// Donc local_name() retourne :
    ///
    ///     "login"
    ///
    ///
    /// Exemple avec alias :
    ///
    ///     use crate::auth::login as connect;
    ///
    /// dans le fichier nous écrivons :
    ///
    ///     connect();
    ///
    /// Donc local_name() doit retourner :
    ///
    ///     "connect"
    ///
    ///
    /// # Retour
    ///
    /// Nous retournons `&str`.
    ///
    /// Nous n'avons pas besoin de créer
    /// une nouvelle String.
    ///
    /// Nous retournons simplement une référence
    /// vers une String déjà présente
    /// dans notre ImportReference.
    pub fn local_name(&self) -> &str {
        // Si un alias existe,
        // nous retournons l'alias.
        //
        // Sinon nous retournons imported_name.
        self.alias.as_deref().unwrap_or(self.imported_name.as_str())
    }
}
