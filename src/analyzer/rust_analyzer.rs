#![allow(
    clippy::collapsible_if,
    clippy::doc_overindented_list_items,
    clippy::empty_line_after_doc_comments
)]

use anyhow::{Context, Ok, Result};

use crate::{
    language::ProgrammingLanguage,
    model::{
        analysis::FileAnalysis,
        call::{CallKind, CallReference},
        import::ImportReference,
        module::ModuleReference,
        node::{CodeNode, NodeKind},
        project::ProjectFile,
    },
};
use std::{fs, path::Path};
use tree_sitter::{Language, Node, Parser, Tree};
/// Analyse du code source Rust et retourne
/// l'arbre syntaxique construit par Tree-sitter.
///
/// # Paramètre
///
/// `source: &str`
///
/// `source` contient le texte complet d'un fichier Rust.
///
/// Exemple :
///
///     "fn main() {
///         println!(\"Hello\");
///     }"
///
/// Nous utilisons `&str` et non `String`.
///
/// Pourquoi ?
///
/// Parce que cette fonction n'a pas besoin de devenir
/// propriétaire du code source.
///
/// Elle a seulement besoin de le lire.
///
/// Le caractère `&` signifie donc que nous
/// empruntons la chaîne de caractères.
///
///
/// # Retour
///
/// La fonction retourne :
///
///     Result<Tree>
///
/// `Tree` représente l'arbre syntaxique créé
/// par Tree-sitter.
///
/// `Result` signifie que l'opération peut :
///
///     réussir -> Ok(Tree)
///
/// ou :
///
///     échouer -> Err(...)
///

pub fn parse_rust_source(source: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    let language: Language = tree_sitter_rust::LANGUAGE.into();
    parser
        .set_language(&language)
        .context("Impossible de charger la grammaire Rust dans Tree-sitter")?;

    let tree = parser
        .parse(source, None)
        .context("Tree-sister n'a pas réussi à produire un arbre syntaxique ")?;

    Ok(tree)
}
/// Analyse un fichier Rust complet et retourne
/// les éléments de code trouvés dans ce fichier.
///
/// Exemple :
///
/// src/user.rs
///
/// peut produire :
///
///     Struct User
///     Enum Role
///     Function create_user
///     Function delete_user
///
///
/// # Paramètre `project_root`
///
/// Type :
///
///     &Path
///
/// Il représente le dossier racine du projet.
///
/// Exemple :
///
///     /Users/julien/code-atlas
///
///
/// # Paramètre `file`
///
/// Type :
///
///     &ProjectFile
///
/// Il représente le fichier détecté précédemment
/// par notre scanner.
///
/// Exemple :
///
///     ProjectFile {
///         path: "src/user.rs",
///         ...
///     }
///
///
/// # Retour
///
///     Result<Vec<CodeNode>>
///
/// Cela signifie :
///
/// soit :
///
///     Ok(Vec<CodeNode>)
///
/// soit :
///
///     Err(...)
pub fn analyze_rust_file(project_root: &Path, file: &ProjectFile) -> Result<FileAnalysis> {
    let full_path = project_root.join(&file.path);

    let source = fs::read_to_string(&full_path).with_context(|| {
        format!(
            "impossible de lire le fichier Rust :{}",
            full_path.display()
        )
    })?;

    let tree = parse_rust_source(&source)?;
    let root_node = tree.root_node();
    let mut nodes = Vec::new();

    walk_rust_node(root_node, &source, &file.path, file.language, &mut nodes);

    // je te prête nodes ET je t'autorise à le modifier.
    // Maintenant que les CodeNode ont été construits,
    // nous pouvons rechercher les appels.
    //
    // Pourquoi APRÈS ?
    //
    // Parce qu'un CallReference doit connaître
    // l'identifiant de la fonction qui contient l'appel.
    //
    // Exemple :
    //
    // fn login() {
    //
    //     verify_password();
    //
    // }
    //
    // Pour créer le CallReference de verify_password(),
    // nous devons déjà connaître le CodeNode de login().
    let mut calls = Vec::new();

    collect_rust_calls(root_node, &source, &file.path, &nodes, &mut calls);
    let mut imports = Vec::new();
    collect_rust_imports(root_node, &source, &file.path, &mut imports);
    // Collection contenant toutes
    // les déclarations `mod`.
    let mut modules = Vec::new();

    collect_rust_modules(root_node, &source, &file.path, &mut modules);

    Ok(FileAnalysis {
        nodes,
        calls,
        imports,
        modules,
    })
}

/// Parcourt récursivement un nœud Tree-sitter
/// ainsi que tous ses enfants.
///
/// Cette fonction constitue l'un des premiers
/// véritables moteurs de Code Atlas.
///
///
/// Exemple d'arbre :
///
/// source_file
/// │
/// ├── struct_item
/// │   └── field_declaration_list
/// │       ├── field_declaration
/// │       └── field_declaration
/// │
/// └── function_item
///     ├── parameters
///     └── block
///
/// La fonction va visiter chacun de ces éléments.
///
///
/// # `node`
///
/// Le Node Tree-sitter actuellement visité.
///
///
/// # `source`
///
/// Le texte complet du fichier.
///
/// Nous en aurons besoin pour récupérer
/// le texte correspondant à certains Nodes,
/// notamment leur nom.
///
///
/// # `relative_path`
///
/// Chemin du fichier dans le projet.
///
/// Exemple :
///
///     src/user.rs
///
///
/// # `language`
///
/// Langage du fichier.
///
/// Ici normalement Rust.
///
///
/// # `nodes`
///
/// Liste dans laquelle nous allons ajouter
/// les CodeNode découverts.
///
/// `&mut` signifie que nous avons le droit
/// de modifier cette liste.
/// */
/// Représente le résultat complet
/// de l'analyse d'un seul fichier Rust.
///
/// Pour l'instant nous récupérons :
///
/// 1. les déclarations :
///
///     Struct
///     Enum
///     Trait
///     Function
///     Method
///
/// 2. les appels présents dans le fichier.
///
/// Plus tard nous pourrons ajouter :
///
///     imports
///     variables
///     impl
///     macros
///     types
///     références
/// Résultat complet de l'analyse
/// d'un fichier Rust.
///
/// Un seul fichier peut maintenant produire :
///
/// - des Nodes
/// - des appels
/// - des imports

fn walk_rust_node(
    node: Node<'_>,
    source: &str,
    relative_path: &str,
    language: ProgrammingLanguage,
    nodes: &mut Vec<CodeNode>,
) {
    if let Some(kind) = classify_rust_node(node) {
        if let Some(code_node) = create_code_node(node, kind, source, relative_path, language) {
            nodes.push(code_node);
        }
    }

    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        walk_rust_node(child, source, relative_path, language, nodes);
    }
}
/// Convertit un type de Node Tree-sitter
/// en type de Node Code Atlas.
///
/// Tree-sitter possède ses propres noms :
///
///     function_item
///     struct_item
///     enum_item
///     trait_item
///
/// Notre application possède ses propres types :
///
///     NodeKind::Function
///     NodeKind::Struct
///     NodeKind::Enum
///     NodeKind::Trait
///
/// Cette fonction sert donc de pont entre
/// Tree-sitter et notre modèle métier.
fn classify_rust_node(node: Node<'_>) -> Option<NodeKind> {
    // `node.kind()` retourne le nom syntaxique
    // donné par la grammaire Tree-sitter.
    //
    // Exemple :
    //
    // "function_item"
    //
    // ou :
    //
    // "struct_item"
    match node.kind() {
        "function_item" => {
            // Une fonction Rust peut être :
            //
            // fn hello()
            //
            // mais également une méthode :
            //
            // impl User {
            //     fn save(&self) {}
            // }
            //
            // Dans le deuxième cas,
            // la fonction se trouve à l'intérieur
            // d'un `impl_item`.
            if has_ancestor(node, "impl_item") || has_ancestor(node, "trait_item") {
                Some(NodeKind::Method)
            } else {
                Some(NodeKind::Function)
            }
        }

        // Une signature de fonction sans corps
        // apparaît notamment dans certains traits.
        //
        // Exemple :
        //
        // trait Repository {
        //     fn save(&self);
        // }
        "function_signature_item" => Some(NodeKind::Method),

        "struct_item" => Some(NodeKind::Struct),

        "enum_item" => Some(NodeKind::Enum),

        "trait_item" => Some(NodeKind::Trait),

        "type_item" => Some(NodeKind::TypeAlias),

        "const_item" | "static_item" => Some(NodeKind::Constant),

        // Tous les autres Nodes Tree-sitter
        // ne nous intéressent pas encore.
        //
        // Exemple :
        //
        // block
        // identifier
        // parameters
        // field_declaration
        //
        // Nous les analyserons plus tard
        // si nécessaire.
        _ => None,
    }
}

/// Vérifie si un Node possède un ancêtre
/// d'un certain type.
///
/// Exemple :
///
/// impl User {
///
///     fn save(&self) {
///     }
/// }
///
/// L'arbre ressemble grossièrement à :
///
/// impl_item
///     │
///     └── declaration_list
///             │
///             └── function_item
///
/// Si nous sommes sur `function_item`
/// et que nous recherchons `impl_item`,
/// cette fonction retournera true.
///
///
/// # `node`
///
/// Node à partir duquel nous remontons l'arbre.
///
///
/// # `ancestor_kind`
///
/// Nom Tree-sitter recherché.
///
/// Exemple :
///
///     "impl_item"
fn has_ancestor(node: Node<'_>, ancestor_kind: &str) -> bool {
    // `parent()` retourne :
    //
    // Option<Node>
    //
    // car le nœud racine n'a aucun parent.
    let mut current = node.parent();

    // Tant qu'un parent existe...
    while let Some(parent) = current {
        // On regarde son type.
        if parent.kind() == ancestor_kind {
            return true;
        }

        // Sinon nous montons encore
        // d'un niveau.
        current = parent.parent();
    }

    // Nous sommes remontés jusqu'à la racine
    // sans trouver le type recherché.
    false
}
/// Recherche le propriétaire Rust d'une méthode.
///
/// Exemple :
///
/// impl User {
///
///     fn save(&self) {
///     }
///
/// }
///
/// Si `node` représente `save`,
/// la fonction remonte ses parents jusqu'à
/// trouver :
///
///     impl_item
///
/// puis elle récupère le champ :
///
///     type
///
/// qui correspond ici à :
///
///     User
///
///
/// # Retour
///
///     Option<String>
///
/// Cela donnera :
///
///     Some("User")
///
/// si un `impl` est trouvé.
///
/// Sinon :
///
///     None
fn find_rust_method_owner(node: Node<'_>, source: &str) -> Option<String> {
    // On commence par le parent direct
    // de la fonction.
    let mut current = node.parent();

    // Tant qu'un parent existe...
    while let Some(parent) = current {
        // Sommes-nous arrivés dans :
        //
        // impl User {
        //
        // }
        if parent.kind() == "impl_item" {
            // La grammaire Rust Tree-sitter
            // donne le nom "type" à la cible
            // de l'impl.
            //
            // Exemple :
            //
            // impl User
            //
            // donnera un Node correspondant
            // à :
            //
            // User
            let type_node = parent.child_by_field_name("type")?;

            // Nous récupérons le texte exact
            // correspondant à ce Node.
            let owner = type_node.utf8_text(source.as_bytes()).ok()?.to_string();

            return Some(owner);
        }

        // Sinon on continue à remonter
        // dans l'arbre syntaxique.
        current = parent.parent();
    }

    // Aucun impl_item trouvé.
    None
}
/// Transforme un Node Tree-sitter intéressant
/// en CodeNode utilisable par Code Atlas.
///
/// Cette fonction récupère notamment :
///
/// - son nom
/// - son type
/// - son fichier
/// - sa ligne de début
/// - sa ligne de fin
/// - son langage
///
///
/// La fonction retourne Option<CodeNode>
/// car certains Nodes pourraient théoriquement
/// ne pas avoir de nom récupérable.
fn create_code_node(
    node: Node<'_>,
    kind: NodeKind,
    source: &str,
    relative_path: &str,
    language: ProgrammingLanguage,
) -> Option<CodeNode> {
    // Les éléments qui nous intéressent possèdent
    // généralement un champ Tree-sitter nommé "name".
    //
    // Exemple :
    //
    // function_item
    //     name: (identifier)
    //
    // Tree-sitter sait donc directement
    // quel enfant correspond au nom.
    let name_node = node.child_by_field_name("name")?;

    // Le Node ne contient pas directement
    // une String représentant son texte.
    //
    // Il contient essentiellement des positions
    // dans le fichier source.
    //
    // `utf8_text()` lui donne le fichier complet
    // et lui demande :
    //
    // "donne-moi le texte qui correspond
    // à cette zone".
    //
    // Par exemple :
    //
    // name_node
    //
    // pourrait pointer vers :
    //
    // create_user
    let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

    let kind = if kind == NodeKind::Method && name == "new" {
        NodeKind::Constructor
    } else {
        kind
    };

    // Tree-sitter compte les lignes à partir de zéro.
    //
    // Donc :
    //
    // première ligne = row 0
    //
    // Mais un humain parle de :
    //
    // ligne 1
    //
    // Nous ajoutons donc 1.
    let start_line = node.start_position().row + 1;

    let end_line = node.end_position().row + 1;

    // Nous créons un préfixe lisible
    // correspondant au type du Node.
    //
    // Ce préfixe sera utilisé dans notre ID.
    let id_prefix = match kind {
        NodeKind::Function => "function",

        NodeKind::Method => "method",

        NodeKind::Constructor => "constructor",

        NodeKind::Struct => "struct",

        NodeKind::Enum => "enum",

        NodeKind::Trait => "trait",

        NodeKind::TypeAlias => "type_alias",

        NodeKind::Constant => "constant",

        // Cette fonction est actuellement appelée
        // uniquement pour les types précédents.
        //
        // Mais le match doit rester exhaustif,
        // car NodeKind possède d'autres variantes.
        _ => "node",
    };

    // Création d'un identifiant unique.
    //
    // Exemple :
    //
    // function:src/user.rs:25:create_user
    //
    // Pourquoi inclure la ligne ?
    //
    // Parce qu'il est possible d'avoir des éléments
    // portant le même nom à différents endroits.
    let id = format!("{}:{}:{}:{}", id_prefix, relative_path, start_line, name,);

    // Seules les méthodes ont besoin,
    // pour l'instant,
    // de rechercher un propriétaire.
    //
    // Une fonction libre :
    //
    // fn login()
    //
    // n'a pas de propriétaire.
    //
    // Une méthode :
    //
    // impl User {
    //     fn save() {}
    // }
    //
    // possède User comme propriétaire.
    let owner = if matches!(kind, NodeKind::Method | NodeKind::Constructor) {
        find_rust_method_owner(node, source)
    } else {
        None
    };
    // Construction finale de notre objet métier.
    Some(CodeNode {
        id,

        kind,

        name,

        // `relative_path` est actuellement &str.
        //
        // CodeNode doit posséder sa propre String.
        //
        // Nous devons donc créer cette String.
        path: Some(relative_path.to_string()),

        language: Some(language),

        start_line: Some(start_line),

        end_line: Some(end_line),
        owner,
        source_scope: crate::model::project::SourceScope::classify(relative_path),
    })
}

/// Recherche la fonction ou méthode
/// contenant un Node Tree-sitter.
///
/// Exemple :
///
/// fn login() {
///
///     verify_password();
///
/// }
///
/// Si `node` correspond à l'appel
/// `verify_password()`,
///
/// la fonction remonte l'arbre jusqu'à
/// retrouver :
///
///     function_item
///
/// correspondant à `login`.
///
///
/// # Retour
///
///     Option<Node>
///
/// `Some(Node)` si une fonction englobante
/// a été trouvée.
///
/// `None` si l'appel n'est pas situé
/// dans une fonction connue.
fn find_enclosing_function<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    // Nous commençons au parent direct.
    //
    // Nous ne testons pas directement `node`
    // car ici `node` représente normalement
    // un call_expression.
    let mut current = node.parent();

    // Tant qu'un parent existe...
    while let Some(parent) = current {
        // Une fonction Rust avec corps
        // utilise `function_item`.
        //
        // Nous ne recherchons pas ici
        // function_signature_item,
        // car une signature sans corps
        // ne peut pas contenir d'appel.
        if parent.kind() == "function_item" {
            return Some(parent);
        }

        // Nous remontons d'un niveau.
        current = parent.parent();
    }

    // Nous avons atteint la racine
    // sans trouver de fonction.
    None
}
/// Retrouve le CodeNode correspondant
/// à un `function_item` Tree-sitter.
///
/// Nous utilisons trois informations :
///
/// - le chemin
/// - le nom
/// - la ligne de début
///
/// Exemple :
///
/// Tree-sitter :
///
///     login
///     src/auth.rs
///     ligne 42
///
/// doit correspondre au CodeNode :
///
///     function:src/auth.rs:42:login
fn find_function_code_node<'a>(
    function_node: Node<'_>,
    source: &str,
    relative_path: &str,
    code_nodes: &'a [CodeNode],
) -> Option<&'a CodeNode> {
    // On demande à Tree-sitter
    // le champ syntaxique "name".
    let name_node = function_node.child_by_field_name("name")?;

    // On récupère le texte correspondant.
    let function_name = name_node.utf8_text(source.as_bytes()).ok()?;

    // Tree-sitter commence les lignes à 0.
    //
    // Notre CodeNode les stocke à partir de 1.
    let start_line = function_node.start_position().row + 1;

    // `.iter()` parcourt les CodeNode
    // sans les déplacer.
    //
    // `.find()` s'arrête dès que
    // la condition est vraie.
    code_nodes.iter().find(|code_node| {
        // Vérification du chemin.
        let same_path = code_node.path.as_deref() == Some(relative_path);

        // Vérification du nom.
        let same_name = code_node.name == function_name;

        // Vérification de la ligne.
        let same_line = code_node.start_line == Some(start_line);

        // Le CodeNode doit représenter
        // une fonction ou une méthode.
        let callable = matches!(
            code_node.kind,
            NodeKind::Function | NodeKind::Method | NodeKind::Constructor
        );

        same_path && same_name && same_line && callable
    })
}

/// Représentation temporaire d'une cible
/// d'appel après analyse syntaxique.
///
/// Cette structure reste interne
/// à rust_analyzer.rs.
///
/// Elle nous aide simplement à transformer
/// un Node Tree-sitter en CallReference.
struct ParsedCallTarget {
    /// Forme de l'appel.
    kind: CallKind,

    /// Texte complet.
    ///
    /// Exemple :
    ///
    /// User::new
    raw_target: String,

    /// Nom terminal.
    ///
    /// Exemple :
    ///
    /// new
    target_name: String,

    /// Partie éventuelle placée avant.
    ///
    /// Exemple :
    ///
    /// User
    qualifier: Option<String>,
}
/// Récupère le texte correspondant
/// à un Node Tree-sitter.
///
/// Exemple :
///
/// si le Node représente :
///
///     User::new
///
/// la fonction retourne :
///
///     Some("User::new")
///
///
/// Le texte retourné est une nouvelle String,
/// indépendante du buffer source.
fn node_text(node: Node<'_>, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(|text| text.to_string())
}
/// Analyse le Node placé dans le champ
/// `function` d'un `call_expression`.
///
/// Exemple :
///
///     login()
///
/// Tree-sitter nous donne environ :
///
///     call_expression
///       function: identifier
///
///
/// Exemple :
///
///     user.save()
///
/// devient :
///
///     call_expression
///       function: field_expression
///
///
/// Exemple :
///
///     User::new()
///
/// devient :
///
///     call_expression
///       function: scoped_identifier
fn parse_call_target(function_node: Node<'_>, source: &str) -> Option<ParsedCallTarget> {
    // Nous regardons quel type syntaxique
    // représente la cible.
    match function_node.kind() {
        // ---------------------------------
        // CAS 1
        //
        // login()
        // verify_password()
        // create_user()
        // ---------------------------------
        "identifier" => {
            let name = node_text(function_node, source)?;

            Some(ParsedCallTarget {
                kind: CallKind::Function,

                raw_target: name.clone(),

                target_name: name,

                qualifier: None,
            })
        }

        // ---------------------------------
        // CAS 2
        //
        // user.save()
        // self.validate()
        // repository.find()
        // ---------------------------------
        "field_expression" => {
            // Tree-sitter décompose :
            //
            // user.save
            //
            // en :
            //
            // value = user
            // field = save

            let value_node = function_node.child_by_field_name("value")?;

            let field_node = function_node.child_by_field_name("field")?;

            let qualifier = node_text(value_node, source)?;

            let target_name = node_text(field_node, source)?;

            let raw_target = node_text(function_node, source)?;

            Some(ParsedCallTarget {
                kind: CallKind::Method,

                raw_target,

                target_name,

                qualifier: Some(qualifier),
            })
        }

        // ---------------------------------
        // CAS 3
        //
        // User::new()
        // auth::login()
        // crate::jwt::generate()
        // ---------------------------------
        "scoped_identifier" => {
            // scoped_identifier possède notamment :
            //
            // path
            // name

            let name_node = function_node.child_by_field_name("name")?;

            let target_name = node_text(name_node, source)?;

            // Le path est optionnel dans la grammaire,
            // donc nous le traitons comme Option.
            let qualifier = function_node
                .child_by_field_name("path")
                .and_then(|path_node| node_text(path_node, source));

            let raw_target = node_text(function_node, source)?;

            Some(ParsedCallTarget {
                kind: CallKind::Scoped,

                raw_target,

                target_name,

                qualifier,
            })
        }

        // ---------------------------------
        // CAS 4
        //
        // foo::<String>()
        // User::new::<Type>()
        //
        // Tree-sitter utilise ici
        // generic_function.
        // ---------------------------------
        "generic_function" => {
            // generic_function possède lui-même
            // un champ "function".
            //
            // Exemple :
            //
            // foo::<String>
            //
            // la partie fonction est :
            //
            // foo
            let inner_function = function_node.child_by_field_name("function")?;

            // On réutilise notre propre fonction.
            //
            // C'est une petite récursion :
            //
            // generic_function
            //       ↓
            // identifier / scoped_identifier
            let mut parsed = parse_call_target(inner_function, source)?;

            // Mais raw_target doit conserver
            // le texte complet comprenant
            // les génériques.
            //
            // Exemple :
            //
            // User::new::<String>
            parsed.raw_target = node_text(function_node, source)?;

            Some(parsed)
        }

        // ---------------------------------
        // Cas que nous ne savons
        // pas encore interpréter.
        // ---------------------------------
        _ => {
            let raw_target = node_text(function_node, source)?;

            Some(ParsedCallTarget {
                kind: CallKind::Unknown,

                target_name: raw_target.clone(),

                raw_target,

                qualifier: None,
            })
        }
    }
}
/// Transforme un `call_expression` Tree-sitter
/// en CallReference Code Atlas.
///
/// Cette fonction doit réussir à déterminer :
///
/// - qui effectue l'appel
/// - ce qui est appelé
/// - à quelle ligne
fn create_call_reference(
    call_node: Node<'_>,
    source: &str,
    relative_path: &str,
    code_nodes: &[CodeNode],
) -> Option<CallReference> {
    // Premièrement :
    //
    // dans quelle fonction sommes-nous ?
    let enclosing_function = find_enclosing_function(call_node)?;

    // Deuxièmement :
    //
    // quel CodeNode représente
    // cette fonction ?
    let caller = find_function_code_node(enclosing_function, source, relative_path, code_nodes)?;

    // Un call_expression possède un champ
    // Tree-sitter nommé "function".
    //
    // Exemple :
    //
    // verify_password()
    //
    // `function` correspond à :
    //
    // verify_password
    let function_node = call_node.child_by_field_name("function")?;

    // On analyse maintenant la forme.
    let target = parse_call_target(function_node, source)?;

    // Ligne humaine :
    //
    // Tree-sitter row 0
    //
    // devient :
    //
    // ligne 1
    let line = call_node.start_position().row + 1;

    Some(CallReference {
        // Le caller existe déjà.
        //
        // Nous copions uniquement son ID.
        caller_id: caller.id.clone(),

        path: relative_path.to_string(),

        line,

        raw_target: target.raw_target,

        target_name: target.target_name,

        qualifier: target.qualifier,

        kind: target.kind,
    })
}

/// Parcourt récursivement l'AST Rust
/// afin de découvrir les `call_expression`.
///
/// Cette fonction ressemble volontairement
/// à `walk_rust_node`.
///
/// Mais elle possède une responsabilité différente :
///
/// `walk_rust_node`
///     -> découvre les déclarations
///
/// `collect_rust_calls`
///     -> découvre les appels
///
/// Cette séparation nous permettra plus tard
/// d'améliorer chaque analyse indépendamment.
fn collect_rust_calls(
    node: Node<'_>,
    source: &str,
    relative_path: &str,
    code_nodes: &[CodeNode],
    calls: &mut Vec<CallReference>,
) {
    // Est-ce que le Node actuel
    // représente un appel ?
    if node.kind() == "call_expression" {
        // Nous essayons de créer
        // un CallReference.
        if let Some(call_reference) = create_call_reference(node, source, relative_path, code_nodes)
        {
            calls.push(call_reference);
        }
    }

    // Comme précédemment,
    // nous parcourons ensuite tous
    // les enfants syntaxiques.
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        collect_rust_calls(child, source, relative_path, code_nodes, calls);
    }
}

/// Assemble deux morceaux de chemin Rust.
///
/// Exemple :
///
///     prefix = "crate::auth"
///
///     suffix = "login"
///
/// retourne :
///
///     "crate::auth::login"
///
///
/// Si `prefix` est vide :
///
///     prefix = ""
///     suffix = "std"
///
/// retourne simplement :
///
///     "std"
fn join_rust_path(prefix: &str, suffix: &str) -> String {
    // Si la partie gauche est vide,
    // inutile d'ajouter "::".
    if prefix.is_empty() {
        return suffix.to_string();
    }

    // Si la partie droite est vide,
    // nous retournons simplement le prefix.
    if suffix.is_empty() {
        return prefix.to_string();
    }

    format!("{}::{}", prefix, suffix,)
}
/// Retourne le dernier élément
/// d'un chemin Rust.
///
/// Exemple :
///
///     crate::auth::verify_password
///
/// devient :
///
///     verify_password
///
///
/// Exemple :
///
///     std::collections::HashMap
///
/// devient :
///
///     HashMap
fn last_path_segment(path: &str) -> String {
    // split("::")
    //
    // découpe :
    //
    // crate::auth::login
    //
    // en :
    //
    // crate
    // auth
    // login
    //
    //
    // `.last()`
    //
    // récupère le dernier élément.
    path.split("::")
        .last()
        // Normalement un path ne sera jamais vide ici.
        //
        // Mais nous gardons un fallback sûr.
        .unwrap_or(path)
        .to_string()
}

/// Ajoute un import simple dans notre collection.
///
/// Cette fonction évite de répéter
/// la construction de ImportReference
/// à plusieurs endroits.
///
///
/// # `full_path`
///
/// Chemin réel importé.
///
/// Exemple :
///
///     crate::auth::login
///
///
/// # `alias`
///
/// Alias éventuel.
///
/// Exemple :
///
///     Some("connect")
///
///
/// # `is_wildcard`
///
/// true pour :
///
///     use crate::auth::*;
///
/// false autrement.
fn push_import(
    imports: &mut Vec<ImportReference>,
    source_path: &str,
    line: usize,
    full_path: String,
    alias: Option<String>,
    is_wildcard: bool,
    raw_use: &str,
) {
    // Si nous avons une wildcard :
    //
    // crate::auth::*
    //
    // le "nom importé" sera simplement "*".
    let imported_name = if is_wildcard {
        "*".to_string()
    } else {
        last_path_segment(&full_path)
    };

    imports.push(ImportReference {
        source_path: source_path.to_string(),

        line,

        full_path,

        imported_name,

        alias,

        is_wildcard,

        raw_use: raw_use.to_string(),
    });
}
/// Analyse récursivement un morceau
/// d'une déclaration `use`.
///
/// Cette fonction est la partie centrale
/// de notre analyse des imports.
///
///
/// # Exemple simple
///
///     use crate::auth::login;
///
///
/// # Exemple groupé
///
///     use crate::auth::{
///         login,
///         logout,
///     };
///
///
/// # Exemple imbriqué
///
///     use crate::{
///         auth::{
///             login,
///             logout,
///         },
///         user::User,
///     };
///
///
/// # `node`
///
/// Node Tree-sitter actuellement analysé.
///
///
/// # `prefix`
///
/// Partie du chemin déjà rencontrée.
///
/// Exemple :
///
/// lorsque nous analysons `login` dans :
///
///     use crate::auth::{login};
///
/// prefix contient déjà :
///
///     crate::auth
///
///
/// # `source`
///
/// Texte complet du fichier.
///
///
/// # `source_path`
///
/// Fichier contenant le `use`.
///
///
/// # `line`
///
/// Ligne de la déclaration.
///
///
/// # `raw_use`
///
/// Texte original complet de la déclaration.
///
///
/// # `imports`
///
/// Collection que nous pouvons modifier
/// grâce à `&mut`.
fn expand_use_node(
    node: Node<'_>,
    prefix: &str,
    source: &str,
    source_path: &str,
    line: usize,
    raw_use: &str,
    imports: &mut Vec<ImportReference>,
) {
    match node.kind() {
        // ====================================================
        // CAS SIMPLE
        //
        // login
        //
        // ou :
        //
        // crate
        //
        // self
        //
        // super
        // ====================================================
        "identifier" | "crate" | "super" => {
            let Some(text) = node_text(node, source) else {
                return;
            };

            let full_path = join_rust_path(prefix, &text);

            push_import(imports, source_path, line, full_path, None, false, raw_use);
        }

        // ====================================================
        // CAS :
        //
        // self
        //
        // dans un groupe.
        //
        // Exemple :
        //
        // use crate::auth::{
        //     self,
        //     login,
        // };
        //
        // `self` signifie ici :
        //
        // importer crate::auth lui-même.
        // ====================================================
        "self" => {
            if prefix.is_empty() {
                let Some(text) = node_text(node, source) else {
                    return;
                };

                push_import(imports, source_path, line, text, None, false, raw_use);
            } else {
                push_import(
                    imports,
                    source_path,
                    line,
                    prefix.to_string(),
                    None,
                    false,
                    raw_use,
                );
            }
        }

        // ====================================================
        // CHEMIN COMPLET
        //
        // crate::auth::login
        //
        // std::collections::HashMap
        // ====================================================
        "scoped_identifier" => {
            let Some(text) = node_text(node, source) else {
                return;
            };

            let full_path = join_rust_path(prefix, &text);

            push_import(imports, source_path, line, full_path, None, false, raw_use);
        }

        // ====================================================
        // ALIAS
        //
        // use crate::auth::login as connect;
        // ====================================================
        "use_as_clause" => {
            // Tree-sitter nous donne explicitement
            // le champ `path`.
            let Some(path_node) = node.child_by_field_name("path") else {
                return;
            };

            // Et explicitement le champ `alias`.
            let Some(alias_node) = node.child_by_field_name("alias") else {
                return;
            };

            let Some(path_text) = node_text(path_node, source) else {
                return;
            };

            let Some(alias) = node_text(alias_node, source) else {
                return;
            };

            let full_path = join_rust_path(prefix, &path_text);

            push_import(
                imports,
                source_path,
                line,
                full_path,
                Some(alias),
                false,
                raw_use,
            );
        }

        // ====================================================
        // GROUPE AVEC PREFIX
        //
        // crate::auth::{
        //     login,
        //     logout,
        // }
        // ====================================================
        "scoped_use_list" => {
            // Tree-sitter fournit éventuellement :
            //
            // path
            //
            // Exemple :
            //
            // crate::auth
            let local_prefix = if let Some(path_node) = node.child_by_field_name("path") {
                let Some(path_text) = node_text(path_node, source) else {
                    return;
                };

                join_rust_path(prefix, &path_text)
            } else {
                prefix.to_string()
            };

            // Le contenu entre { ... }
            // est stocké dans le champ `list`.
            let Some(list_node) = node.child_by_field_name("list") else {
                return;
            };

            let mut cursor = list_node.walk();

            // Pour chaque élément du groupe,
            // on rappelle cette même fonction.
            //
            // Voilà notre récursion.
            for child in list_node.named_children(&mut cursor) {
                expand_use_node(
                    child,
                    &local_prefix,
                    source,
                    source_path,
                    line,
                    raw_use,
                    imports,
                );
            }
        }

        // ====================================================
        // LISTE SANS PREFIX PARTICULIER
        //
        // {foo, bar}
        // ====================================================
        "use_list" => {
            let mut cursor = node.walk();

            for child in node.named_children(&mut cursor) {
                expand_use_node(child, prefix, source, source_path, line, raw_use, imports);
            }
        }

        // ====================================================
        // WILDCARD
        //
        // use crate::models::*;
        // ====================================================
        "use_wildcard" => {
            let Some(raw_target) = node_text(node, source) else {
                return;
            };

            // raw_target peut être :
            //
            // crate::models::*
            //
            // ou simplement :
            //
            // *
            //
            //
            // Nous supprimons le "*" final.
            let without_star = raw_target.trim_end_matches('*').trim_end_matches("::");

            let base_path = if without_star.is_empty() {
                prefix.to_string()
            } else {
                join_rust_path(prefix, without_star)
            };

            // Nous reconstruisons un chemin
            // explicite :
            //
            // crate::models::*
            let full_path = if base_path.is_empty() {
                "*".to_string()
            } else {
                format!("{}::*", base_path)
            };

            push_import(imports, source_path, line, full_path, None, true, raw_use);
        }

        // ====================================================
        // TYPE NON ENCORE GÉRÉ
        // ====================================================
        //
        // Très important :
        //
        // nous ne faisons rien.
        //
        // Nous n'inventons pas une interprétation.
        _ => {}
    }
}

/// Parcourt tout l'arbre Rust
/// et collecte toutes les déclarations `use`.
///
/// Exemple :
///
/// use crate::auth::login;
///
/// fn main() {
/// }
///
/// Tree-sitter possède :
///
/// source_file
///     │
///     ├── use_declaration
///     │
///     └── function_item
///
/// Nous allons rechercher tous
/// les `use_declaration`.
fn collect_rust_imports(
    node: Node<'_>,
    source: &str,
    relative_path: &str,
    imports: &mut Vec<ImportReference>,
) {
    // Avons-nous trouvé :
    //
    // use ...;
    if node.kind() == "use_declaration" {
        // Texte complet du use.
        //
        // Exemple :
        //
        // use crate::auth::{login, logout};
        let raw_use = node_text(node, source).unwrap_or_default();

        // Ligne humaine.
        let line = node.start_position().row + 1;

        // La grammaire Tree-sitter Rust
        // expose le contenu du use
        // dans le champ :
        //
        // argument
        let argument = node.child_by_field_name("argument");

        if let Some(argument_node) = argument {
            // Le prefix commence vide.
            //
            // expand_use_node()
            // construira ensuite progressivement
            // crate::...
            expand_use_node(
                argument_node,
                "",
                source,
                relative_path,
                line,
                &raw_use,
                imports,
            );
        }
    }

    // Nous continuons ensuite
    // à descendre récursivement dans l'arbre.
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        collect_rust_imports(child, source, relative_path, imports);
    }
}

/// Recherche les modules INLINE qui entourent
/// une déclaration `mod`.
///
///
/// Exemple :
///
/// mod api {
///
///     mod auth {
///
///         mod login;
///
///     }
///
/// }
///
/// Lorsque `node` représente :
///
///     mod login;
///
/// nous voulons récupérer :
///
///     ["api", "auth"]
///
///
/// Cela nous permettra de construire
/// le chemin logique complet :
///
///     crate::api::auth::login
fn find_inline_module_ancestors(node: Node<'_>, source: &str) -> Vec<String> {
    // Collection temporaire.
    //
    // Nous allons remonter l'arbre,
    // donc les éléments seront trouvés
    // dans l'ordre inverse.
    let mut modules = Vec::new();

    // Parent direct.
    let mut current = node.parent();

    while let Some(parent) = current {
        // Nous cherchons les mod_item parents.
        if parent.kind() == "mod_item" {
            // Un mod_item avec body représente :
            //
            // mod quelque_chose {
            //
            // }
            //
            // Donc un module inline.
            let is_inline = parent.child_by_field_name("body").is_some();

            if is_inline {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    if let Some(name) = node_text(name_node, source) {
                        modules.push(name);
                    }
                }
            }
        }

        current = parent.parent();
    }

    // Nous sommes remontés :
    //
    // login
    // auth
    // api
    //
    // Nous avons donc probablement :
    //
    // ["auth", "api"]
    //
    // reverse() remet l'ordre logique :
    //
    // ["api", "auth"]
    modules.reverse();

    modules
}

/// Parcourt récursivement l'AST
/// afin de trouver toutes les déclarations `mod`.
///
/// Exemple :
///
///     mod auth;
///
/// ou :
///
///     pub mod user {
///
///     }
///
///
/// Tree-sitter représente les deux
/// sous la forme :
///
///     mod_item
fn collect_rust_modules(
    node: Node<'_>,
    source: &str,
    relative_path: &str,
    modules: &mut Vec<ModuleReference>,
) {
    // Le Node actuel représente-t-il
    // une déclaration mod ?
    if node.kind() == "mod_item" {
        // Tree-sitter nous donne directement
        // le champ `name`.
        let name_node = node.child_by_field_name("name");

        if let Some(name_node) = name_node {
            if let Some(module_name) = node_text(name_node, source) {
                // Si `body` existe :
                //
                // mod auth {
                //
                // }
                //
                // alors le module est inline.
                let is_inline = node.child_by_field_name("body").is_some();

                // Numéro de ligne lisible
                // par un humain.
                let line = node.start_position().row + 1;

                // Texte original.
                let raw_mod = node_text(node, source).unwrap_or_default();

                // Modules inline qui entourent
                // éventuellement cette déclaration.
                let inline_parent_modules = find_inline_module_ancestors(node, source);

                modules.push(ModuleReference {
                    source_path: relative_path.to_string(),

                    module_name,

                    line,

                    is_inline,

                    inline_parent_modules,

                    raw_mod,
                });
            }
        }
    }

    // Descendre dans l'arbre.
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        collect_rust_modules(child, source, relative_path, modules);
    }
}
