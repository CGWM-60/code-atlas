use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::language::ProgrammingLanguage;
use crate::model::project::{ProjectFile, SourceScope};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]

pub enum NodeKind {
    Project,
    ArchitectureZone,
    ArchitectureGroup,
    EntryPoint,
    Handler,
    Worker,
    Job,
    Listener,
    Cli,
    Infrastructure,
    Shared,
    Controller,
    Command,
    Entity,
    EventSubscriber,
    EventListener,
    MessageHandler,
    Template,
    ExternalService,
    /// Workspace complet.
    ///
    /// Exemple :
    ///
    /// workspace Cargo contenant plusieurs packages.
    Workspace,

    /// Package appartenant à un workspace.
    ///
    /// Un package correspond généralement
    /// à un Cargo.toml.
    Package,

    /// Crate réellement compilée par Rust.
    ///
    /// Un package peut contenir plusieurs crates :
    ///
    /// - library
    /// - binary
    /// - test
    /// - example
    Crate,

    /// Module logique d'un langage.
    ///
    /// Exemple Rust :
    ///
    /// crate::auth
    ///
    /// Nous ne l'utilisons pas encore comme Node,
    /// mais nous préparons déjà le type.
    Module,

    /// Fichier physique.
    ///
    /// Exemple :
    ///
    /// src/main.rs
    File,

    /// Fonction.
    ///
    /// Exemple :
    ///
    /// fn login()
    Function,

    /// Structure Rust.
    ///
    /// Exemple :
    ///
    /// struct User
    Struct,

    /// Enum Rust ou équivalent.
    Enum,

    /// Trait Rust.
    ///
    /// Exemple :
    ///
    /// trait Repository
    Trait,

    Interface,

    TypeAlias,

    Constant,

    /// Classe pour des langages comme PHP,
    /// TypeScript, Python, Java...
    Class,

    /// Méthode appartenant à une classe,
    /// une structure ou un objet.
    Method,

    Constructor,

    /// Composant graphique.
    ///
    /// Exemple :
    ///
    /// Flutter Widget
    /// React Component
    Component,

    Page,

    Service,

    Repository,

    Provider,

    Database,

    /// Route applicative.
    ///
    /// Exemple :
    ///
    /// /login
    Route,

    /// Endpoint HTTP.
    ///
    /// Exemple :
    ///
    /// POST /api/login
    ApiEndpoint,

    /// Table de base de données.
    DatabaseTable,

    Model,

    Event,

    Queue,

    WebSocket,

    Config,

    Environment,

    Unknown,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::ArchitectureZone => "ArchitectureZone",
            Self::ArchitectureGroup => "ArchitectureGroup",
            Self::EntryPoint => "EntryPoint",
            Self::Handler => "Handler",
            Self::Worker => "Worker",
            Self::Job => "Job",
            Self::Listener => "Listener",
            Self::Cli => "Cli",
            Self::Infrastructure => "Infrastructure",
            Self::Shared => "Shared",
            Self::Controller => "Controller",
            Self::Command => "Command",
            Self::Entity => "Entity",
            Self::EventSubscriber => "EventSubscriber",
            Self::EventListener => "EventListener",
            Self::MessageHandler => "MessageHandler",
            Self::Template => "Template",
            Self::ExternalService => "ExternalService",
            Self::Workspace => "Workspace",
            Self::Package => "Package",
            Self::Crate => "Crate",
            Self::Module => "Module",
            Self::File => "File",
            Self::Function => "Function",
            Self::Struct => "Struct",
            Self::Enum => "Enum",
            Self::Trait => "Trait",
            Self::Interface => "Interface",
            Self::TypeAlias => "TypeAlias",
            Self::Constant => "Constant",
            Self::Class => "Class",
            Self::Method => "Method",
            Self::Constructor => "Constructor",
            Self::Component => "Component",
            Self::Page => "Page",
            Self::Route => "Route",
            Self::ApiEndpoint => "ApiEndpoint",
            Self::Service => "Service",
            Self::Repository => "Repository",
            Self::Provider => "Provider",
            Self::Database => "Database",
            Self::DatabaseTable => "DatabaseTable",
            Self::Model => "Model",
            Self::Event => "Event",
            Self::Queue => "Queue",
            Self::WebSocket => "WebSocket",
            Self::Config => "Config",
            Self::Environment => "Environment",
            Self::Unknown => "Unknown",
        }
    }
}

/// Représente un élément de notre graphe de code.
///
/// Plus tard chaque rectangle affiché sur notre
/// cartographie correspondra quasiment à un CodeNode.
///
/// Exemple :
///
///     main.rs
///
/// pourra devenir :
///
///     CodeNode {
///         id: "file:src/main.rs",
///         kind: File,
///         ...
///     }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeNode {
    /// Identifiant unique du nœud.
    ///
    /// Nous utilisons volontairement une String
    /// plutôt qu'un simple nombre.
    ///
    /// Exemple :
    ///
    /// file:src/main.rs
    ///
    /// Cela nous donnera plus tard des IDs explicites :
    ///
    /// function:src/auth.rs:login
    /// route:/api/login
    /// table:users
    pub id: String,

    /// Type de l'élément.
    pub kind: NodeKind,

    /// Nom affiché à l'utilisateur.
    ///
    /// Exemple :
    ///
    /// main.rs
    ///
    /// ou plus tard :
    ///
    /// login
    /// UserService
    /// /api/login
    pub name: String,

    /// Fichier contenant l'élément.
    ///
    /// Option signifie que certains Nodes
    /// pourraient ne pas correspondre directement
    /// à un fichier.
    ///
    /// Pour un fichier :
    ///
    /// Some("src/main.rs")
    ///
    /// Pour certains nœuds virtuels :
    ///
    /// None
    pub path: Option<String>,

    /// Langage correspondant au Node.
    ///
    /// Exemple :
    ///
    /// Some(ProgrammingLanguage::Rust)
    pub language: Option<ProgrammingLanguage>,

    /// Première ligne correspondant à l'élément.
    ///
    /// Pour l'instant nous ne connaissons pas encore
    /// les lignes précises.
    ///
    /// Tree-sitter nous donnera bientôt cette information.
    pub start_line: Option<usize>,

    /// Dernière ligne correspondant à l'élément.
    pub end_line: Option<usize>,
    /// Nom de l'élément propriétaire du Node,
    /// lorsqu'il existe.
    ///
    /// Exemple :
    ///
    /// impl User {
    ///
    ///     fn save(&self) {}
    ///
    /// }
    ///
    /// Pour `save` :
    ///
    ///     owner = Some("User")
    ///
    ///
    /// Pour une fonction libre :
    ///
    /// fn login() {}
    ///
    /// nous aurons :
    ///
    ///     owner = None
    ///
    ///
    /// Cette donnée nous aidera à construire
    /// plus tard une relation :
    ///
    /// User
    ///   │
    ///   │ HAS_METHOD
    ///   ▼
    /// save()
    pub owner: Option<String>,

    #[serde(default)]
    pub source_scope: SourceScope,
}

/*
Option permet de retourner explicitement une absence quan on ne connait pas de valeur

*/

pub fn build_file_nodes(files: &[ProjectFile]) -> Vec<CodeNode> {
    // Nous créons le Vec qui recevra nos Nodes.
    //
    // `with_capacity(files.len())` signifie :
    //
    // "je sais déjà approximativement combien
    // d'éléments je vais ajouter".
    //
    // Si nous avons 500 fichiers,
    // Rust réserve directement de la place pour 500 éléments.
    //
    // Ce n'est pas obligatoire.
    //
    // Nous pourrions écrire :
    //
    // let mut nodes = Vec::new();
    //
    // Mais with_capacity évite certaines réallocations mémoire.
    let mut nodes = Vec::with_capacity(files.len());

    for file in files.iter() {
        let path = Path::new(&file.path);

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| file.path.clone());

        let id = format!("file:{}", file.path);

        let node = CodeNode {
            id,
            kind: NodeKind::File,
            name,
            path: Some(file.path.clone()),
            language: Some(file.language),
            start_line: None,
            end_line: None,
            owner: None,
            source_scope: file.source_scope,
        };
        nodes.push(node);
    }
    nodes
}
