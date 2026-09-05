use serde::{Deserialize, Serialize};

/// Représente le type de relation entre
/// deux Nodes de notre graphe.
///
/// Exemple :
///
///     src/main.rs
///         │
///         │ Contains
///         ▼
///     main()
///
///
/// Plus tard nous utiliserons également :
///
///     login()
///         │
///         │ Calls
///         ▼
///     verify_password()
///
///
/// Grâce à un enum, nous empêchons notre programme
/// d'inventer des types de relations incorrects.
///
/// Par exemple une String permettrait :
/// /// Représente le type de relation entre
/// deux Nodes de notre graphe.
///
/// Exemple :
///
///     src/main.rs
///         │
///         │ Contains
///         ▼
///     main()
///
///
/// Plus tard nous utiliserons également :
///
///     login()
///         │
///         │ Calls
///         ▼
///     verify_password()
///
///
/// Grâce à un enum, nous empêchons notre programme
/// d'inventer des types de relations incorrects.
///
/// Par exemple une String permettrait :
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationKind {
    Declares,
    /// Relie une unité compilable
    /// à son fichier source principal.
    ///
    /// Exemple :
    ///
    /// Crate cli
    ///     │
    ///     │ ENTRY_POINT
    ///     ▼
    /// src/main.rs
    EntryPoint,

    /// Un élément en contient un autre.
    ///
    /// Exemple :
    ///
    /// src/user.rs
    ///     │
    ///     └── User
    Contains,

    /// Un type possède une méthode.
    ///
    /// Exemple :
    ///
    /// User
    ///   │
    ///   └── save()
    HasMethod,

    /// Une fonction appelle une autre fonction.
    ///
    /// Exemple :
    ///
    /// login()
    ///    │
    ///    └── verify_password()
    Calls,

    /// Un fichier ou module importe
    /// un autre élément.
    ///
    /// Exemple :
    ///
    /// main.rs
    ///    │
    ///    └── auth.rs
    Imports,

    Exports,

    /// Un type implémente quelque chose.
    ///
    /// Exemple Rust :
    ///
    /// UserRepository
    ///      │
    ///      └── Repository
    Implements,

    Extends,

    Returns,

    Creates,

    Reads,

    Writes,

    Renders,

    RoutesTo,

    HandledBy,

    DependsOn,

    Emits,

    Listens,

    /// Relation générale indiquant
    /// qu'un élément en utilise un autre.
    Uses,
}
impl RelationKind {
    /// Retourne le nom texte correspondant
    /// à une relation.
    ///
    /// Exemple :
    ///
    ///     RelationKind::Calls.as_str()
    ///
    /// retourne :
    ///
    ///     "CALLS"
    ///
    ///
    /// `&self`
    ///
    /// signifie :
    ///
    /// "je veux lire la valeur actuelle,
    /// sans en prendre possession
    /// et sans la modifier".
    ///
    ///
    /// `&'static str`
    ///
    /// signifie que nous retournons
    /// une chaîne constante intégrée
    /// dans le programme.
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationKind::Contains => "CONTAINS",

            RelationKind::HasMethod => "HAS_METHOD",

            RelationKind::Calls => "CALLS",

            RelationKind::Imports => "IMPORTS",

            RelationKind::Implements => "IMPLEMENTS",

            RelationKind::Uses => "USES",
            RelationKind::EntryPoint => "ENTRY_POINT",
            RelationKind::Declares => "DECLARES",
            RelationKind::Exports => "EXPORTS",
            RelationKind::Extends => "EXTENDS",
            RelationKind::Returns => "RETURNS",
            RelationKind::Creates => "CREATES",
            RelationKind::Reads => "READS",
            RelationKind::Writes => "WRITES",
            RelationKind::Renders => "RENDERS",
            RelationKind::RoutesTo => "ROUTES_TO",
            RelationKind::HandledBy => "HANDLED_BY",
            RelationKind::DependsOn => "DEPENDS_ON",
            RelationKind::Emits => "EMITS",
            RelationKind::Listens => "LISTENS",
        }
    }
}
/// Représente une relation entre
/// deux CodeNode.
///
/// Un Edge possède toujours :
///
/// - une source
/// - une destination
/// - un type de relation
///
///
/// Exemple :
///
///     src/user.rs
///         │
///         │ CONTAINS
///         ▼
///     create_user()
///
/// devient :
///
///     source_id = "file:src/user.rs"
///
///     target_id =
///     "function:src/user.rs:45:create_user"
///
///     relation = RelationKind::Contains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEdge {
    /// Identifiant unique de la relation.
    ///
    /// Exemple :
    ///
    /// edge:CONTAINS:file:src/user.rs->function:...
    ///
    /// Cet ID nous permettra plus tard
    /// d'identifier précisément une flèche
    /// dans notre graphe.
    pub id: String,

    /// ID du Node depuis lequel
    /// la relation commence.
    ///
    /// Exemple :
    ///
    /// file:src/user.rs
    pub source_id: String,

    /// ID du Node vers lequel
    /// la relation pointe.
    ///
    /// Exemple :
    ///
    /// function:src/user.rs:42:create_user
    pub target_id: String,

    /// Nature de la relation.
    ///
    /// Exemple :
    ///
    /// Contains
    /// Calls
    /// Imports
    pub relation: RelationKind,
}
impl CodeEdge {
    /// Crée une nouvelle relation entre deux Nodes.
    ///
    ///
    /// # Paramètre `source_id`
    ///
    /// Type :
    ///
    ///     String
    ///
    /// ID du Node source.
    ///
    /// Exemple :
    ///
    ///     "file:src/user.rs"
    ///
    ///
    /// # Paramètre `target_id`
    ///
    /// Type :
    ///
    ///     String
    ///
    /// ID du Node destination.
    ///
    /// Exemple :
    ///
    ///     "function:src/user.rs:42:create_user"
    ///
    ///
    /// # Paramètre `relation`
    ///
    /// Type :
    ///
    ///     RelationKind
    ///
    /// Nature de la relation.
    ///
    /// Exemple :
    ///
    ///     RelationKind::Contains
    ///
    ///
    /// # Retour
    ///
    /// La fonction retourne directement :
    ///
    ///     CodeEdge
    ///
    /// et NON :
    ///
    ///     Result<CodeEdge>
    ///
    /// parce que cette opération ne possède
    /// actuellement aucune situation normale
    /// dans laquelle elle pourrait échouer.
    pub fn new(source_id: String, target_id: String, relation: RelationKind) -> Self {
        // On fabrique d'abord l'identifiant
        // de la relation.
        //
        // Exemple :
        //
        // edge:CONTAINS:file:src/main.rs->function:...
        //
        //
        // Nous utilisons ici les références :
        //
        // &source_id
        // &target_id
        //
        // pour éviter que format! ne nous empêche
        // de réutiliser les Strings ensuite.
        let id = format!("edge:{}:{}->{}", relation.as_str(), source_id, target_id);

        // `Self` représente ici :
        //
        // CodeEdge
        //
        // Nous pourrions donc également écrire :
        //
        // CodeEdge {
        //     ...
        // }
        Self {
            id,
            source_id,
            target_id,
            relation,
        }
    }
}
