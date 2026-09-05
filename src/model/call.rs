use serde::{Deserialize, Serialize};

/// Représente la forme syntaxique d'un appel
/// trouvé dans le code Rust.
///
/// Il ne s'agit PAS encore de la fonction réellement
/// appelée.
///
/// Nous décrivons seulement ce que Tree-sitter
/// nous a permis d'observer dans le code.
///
/// Exemple :
///
///     login()
///
/// correspond à :
///
///     CallKind::Function
///
///
/// Exemple :
///
///     user.save()
///
/// correspond à :
///
///     CallKind::Method
///
///
/// Exemple :
///
///     User::new()
///
/// correspond à :
///
///     CallKind::Scoped
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallKind {
    /// Appel simple d'une fonction.
    ///
    /// Exemple :
    ///
    ///     login()
    ///
    ///     verify_password()
    Function,

    /// Appel utilisant le point.
    ///
    /// Exemple :
    ///
    ///     user.save()
    ///
    ///     self.validate()
    ///
    ///     repository.find()
    Method,

    /// Appel utilisant `::`.
    ///
    /// Exemple :
    ///
    ///     User::new()
    ///
    ///     auth::login()
    ///
    ///     crate::jwt::generate()
    Scoped,

    /// Forme syntaxique que notre moteur
    /// ne sait pas encore interpréter précisément.
    ///
    /// Nous préférons conserver l'information
    /// plutôt que de la perdre ou d'inventer
    /// une résolution.
    Unknown,
}
/// Représente un appel découvert dans le code
/// mais pas forcément encore résolu.
///
///
/// Exemple de code :
///
///     fn create_user() {
///         User::new();
///     }
///
/// Nous pourrions obtenir :
///
///     caller_id:
///         "function:src/user.rs:20:create_user"
///
///     raw_target:
///         "User::new"
///
///     target_name:
///         "new"
///
///     qualifier:
///         Some("User")
///
///     kind:
///         CallKind::Scoped
///
///     line:
///         21
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallReference {
    /// Identifiant du CodeNode dans lequel
    /// l'appel a été trouvé.
    ///
    /// Exemple :
    ///
    ///     function:src/auth.rs:20:login
    pub caller_id: String,

    /// Chemin du fichier où se trouve l'appel.
    ///
    /// Exemple :
    ///
    ///     src/auth.rs
    pub path: String,

    /// Ligne où commence l'appel.
    ///
    /// Cela nous sera utile plus tard pour :
    ///
    /// - afficher l'appel
    /// - ouvrir directement le code
    /// - déboguer une résolution
    pub line: usize,

    /// Texte exact correspondant à la partie appelée.
    ///
    /// Exemple :
    ///
    ///     verify_password
    ///
    /// ou :
    ///
    ///     user.save
    ///
    /// ou :
    ///
    ///     User::new
    ///
    /// Cette donnée est volontairement conservée.
    ///
    /// Même si notre resolver échoue,
    /// l'utilisateur pourra toujours voir
    /// ce que le code contient réellement.
    pub raw_target: String,

    /// Nom terminal de la fonction ou méthode.
    ///
    /// Exemple :
    ///
    ///     login()
    ///
    /// donne :
    ///
    ///     "login"
    ///
    ///
    ///     User::new()
    ///
    /// donne :
    ///
    ///     "new"
    ///
    ///
    ///     user.save()
    ///
    /// donne :
    ///
    ///     "save"
    pub target_name: String,

    /// Partie placée avant le nom lorsque celle-ci existe.
    ///
    /// Exemple :
    ///
    ///     User::new()
    ///
    /// donne :
    ///
    ///     Some("User")
    ///
    ///
    ///     auth::jwt::generate()
    ///
    /// donne :
    ///
    ///     Some("auth::jwt")
    ///
    ///
    ///     user.save()
    ///
    /// donne :
    ///
    ///     Some("user")
    ///
    ///
    ///     login()
    ///
    /// donne :
    ///
    ///     None
    pub qualifier: Option<String>,

    /// Forme syntaxique de l'appel.
    pub kind: CallKind,
}
