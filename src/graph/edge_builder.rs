use std::collections::HashMap;

use crate::model::{
    edge::{CodeEdge, RelationKind},
    node::{CodeNode, NodeKind},
};

/// Construit les relations `CONTAINS`
/// entre les fichiers et les éléments de code
/// trouvés dans ces fichiers.
///
///
/// Exemple :
///
///     src/user.rs
///         │
///         ├── User
///         ├── User::new()
///         └── create_user()
///
/// produit :
///
///     src/user.rs -> User
///
///     src/user.rs -> User::new()
///
///     src/user.rs -> create_user()
///
///
/// # Paramètre `file_nodes`
///
/// Type :
///
///     &[CodeNode]
///
/// Contient les Nodes représentant
/// les fichiers physiques du projet.
///
/// Exemple :
///
///     file:src/main.rs
///     file:src/user.rs
///
///
/// # Paramètre `code_nodes`
///
/// Type :
///
///     &[CodeNode]
///
/// Contient les éléments découverts
/// à l'intérieur du code.
///
/// Exemple :
///
///     Struct User
///     Function main
///     Method new
///
///
/// # Retour
///
///     Vec<CodeEdge>
///
/// La fonction retourne toutes les relations
/// qu'elle a réussi à construire.
pub fn build_contains_edges(file_nodes: &[CodeNode], code_nodes: &[CodeNode]) -> Vec<CodeEdge> {
    // Nous allons construire une table permettant
    // de retrouver très rapidement le Node fichier
    // correspondant à un chemin.
    //
    //
    // Cette HashMap va conceptuellement contenir :
    //
    // "src/main.rs"
    //      ↓
    // "file:src/main.rs"
    //
    //
    // "src/user.rs"
    //      ↓
    // "file:src/user.rs"
    //
    //
    // La clé est donc :
    //
    // &str
    //
    // et la valeur :
    //
    // &str
    //
    //
    // Nous n'avons pas besoin de copier les Strings.
    //
    // Nous empruntons celles déjà présentes
    // dans les CodeNode.
    let mut files_by_path: HashMap<&str, &str> = HashMap::new();

    // Première passe :
    //
    // nous indexons tous les Nodes fichiers.
    for file_node in file_nodes {
        // Théoriquement `file_nodes`
        // contient uniquement des fichiers.
        //
        // Mais nous vérifions quand même.
        //
        // Cela rend notre fonction plus résistante
        // si elle reçoit un jour une collection
        // contenant autre chose.
        if file_node.kind != NodeKind::File {
            continue;
        }

        // `file_node.path` est :
        //
        // Option<String>
        //
        // Nous voulons seulement continuer
        // si le fichier possède réellement
        // un chemin.
        //
        //
        // `as_deref()` transforme :
        //
        // Option<String>
        //
        // en :
        //
        // Option<&str>
        //
        // sans recopier le texte.
        let Some(path) = file_node.path.as_deref() else {
            continue;
        };

        // Même chose pour l'ID :
        //
        // `id` est une String.
        //
        // `as_str()` nous donne une référence
        // vers son contenu :
        //
        // &str
        let file_id = file_node.id.as_str();

        // Maintenant notre index contient :
        //
        // chemin -> identifiant
        files_by_path.insert(path, file_id);
    }

    // Collection finale des relations.
    let mut edges = Vec::new();

    // Deuxième passe :
    //
    // nous regardons chaque élément découvert
    // dans le code.
    for code_node in code_nodes {
        // Un élément de code doit connaître
        // le fichier dont il provient.
        //
        // Exemple :
        //
        // Some("src/user.rs")
        //
        // Si aucun fichier n'est connu,
        // impossible de créer CONTAINS.
        let Some(code_path) = code_node.path.as_deref() else {
            continue;
        };

        // Nous cherchons maintenant :
        //
        // "src/user.rs"
        //
        // dans notre HashMap.
        //
        //
        // `get()` retourne :
        //
        // Option<&&str>
        //
        // Le double `&` vient du fait que :
        //
        // 1. notre HashMap contient déjà des &str
        // 2. get() retourne une référence
        //    vers la valeur stockée
        let Some(file_id) = files_by_path.get(code_path) else {
            // Si aucun fichier correspondant
            // n'existe, nous ignorons ce Node.
            continue;
        };

        // `file_id` est ici :
        //
        // &&str
        //
        // `*file_id` retire un niveau de référence
        // et nous donne :
        //
        // &str
        //
        // Puis `to_string()` crée la String
        // dont CodeEdge va devenir propriétaire.
        let source_id = (*file_id).to_string();

        // Nous devons également copier
        // l'identifiant du CodeNode destination.
        let target_id = code_node.id.clone();

        // Création de notre relation.
        let edge = CodeEdge::new(source_id, target_id, RelationKind::Contains);

        edges.push(edge);
    }

    // Dernière expression :
    //
    // le Vec est retourné.
    edges
}
/// Construit les relations entre
/// les types et leurs méthodes.
///
/// Exemple :
///
///     struct User
///
///     impl User {
///
///         fn new() {}
///         fn save() {}
///
///     }
///
/// devient :
///
///     User
///       │
///       ├── HAS_METHOD -> new
///       │
///       └── HAS_METHOD -> save
///
///
/// # Paramètre
///
/// `code_nodes` contient tous les éléments
/// découverts dans le code.
///
/// Nous avons besoin à la fois :
///
/// - des Struct
/// - des Method
pub fn build_has_method_edges(code_nodes: &[CodeNode]) -> Vec<CodeEdge> {
    // Cette HashMap permettra de retrouver
    // très rapidement un type grâce à :
    //
    // son fichier + son nom.
    //
    //
    // Pourquoi également le fichier ?
    //
    // Parce qu'un projet pourrait théoriquement
    // contenir plusieurs types appelés User
    // dans différents modules.
    //
    //
    // La clé sera une String comme :
    //
    // src/user.rs::User
    //
    // La valeur sera l'ID du Node.
    let mut types_by_name: HashMap<String, &str> = HashMap::new();

    // Première passe :
    //
    // on indexe les types.
    for node in code_nodes {
        // Pour l'instant nous nous intéressons
        // aux structs et enums.
        //
        // Nous enrichirons plus tard pour
        // les alias, unions, etc.
        if node.kind != NodeKind::Struct && node.kind != NodeKind::Enum {
            continue;
        }

        let Some(path) = node.path.as_deref() else {
            continue;
        };

        // Création d'une clé suffisamment précise.
        //
        // Exemple :
        //
        // src/user.rs::User
        let key = format!("{}::{}", path, node.name,);

        types_by_name.insert(key, node.id.as_str());
    }

    let mut edges = Vec::new();

    // Deuxième passe :
    //
    // nous recherchons maintenant
    // les méthodes.
    for method_node in code_nodes {
        if !matches!(method_node.kind, NodeKind::Method | NodeKind::Constructor) {
            continue;
        }

        let Some(owner) = method_node.owner.as_deref() else {
            // Certaines méthodes peuvent appartenir
            // à un trait ou à une situation
            // que notre analyseur ne sait pas encore
            // résoudre.
            continue;
        };

        let Some(path) = method_node.path.as_deref() else {
            continue;
        };

        // On reconstruit exactement la même clé.
        //
        // Exemple :
        //
        // src/user.rs::User
        let key = format!("{}::{}", path, owner,);

        // Recherche du Node correspondant
        // au propriétaire.
        let Some(owner_id) = types_by_name.get(&key) else {
            // Cas possible :
            //
            // impl String {
            // ...
            //
            // ou un type défini ailleurs.
            //
            // Nous ne créons aucune fausse relation.
            continue;
        };

        let edge = CodeEdge::new(
            (*owner_id).to_string(),
            method_node.id.clone(),
            RelationKind::HasMethod,
        );

        edges.push(edge);
    }

    edges
}
