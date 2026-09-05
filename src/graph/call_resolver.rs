use crate::model::{
    call::{CallKind, CallReference},
    edge::{CodeEdge, RelationKind},
    import::ImportReference,
    module::{ModuleResolutionStatus, ResolvedModule},
    node::{CodeNode, NodeKind},
};
use std::collections::{HashMap, HashSet};

struct ResolverIndex<'a> {
    node_by_id: HashMap<&'a str, &'a CodeNode>,
    functions: HashMap<(&'a str, &'a str), Vec<&'a CodeNode>>,
    methods: HashMap<(&'a str, &'a str, &'a str), Vec<&'a CodeNode>>,
    types: HashSet<(&'a str, &'a str)>,
    imports: HashMap<(&'a str, &'a str), Vec<&'a ImportReference>>,
    modules: HashMap<&'a str, Vec<&'a ResolvedModule>>,
}

impl<'a> ResolverIndex<'a> {
    fn new(
        nodes: &'a [CodeNode],
        imports: &'a [ImportReference],
        modules: &'a [ResolvedModule],
    ) -> Self {
        let mut index = Self {
            node_by_id: HashMap::new(),
            functions: HashMap::new(),
            methods: HashMap::new(),
            types: HashSet::new(),
            imports: HashMap::new(),
            modules: HashMap::new(),
        };
        for node in nodes {
            index.node_by_id.insert(&node.id, node);
            let Some(path) = node.path.as_deref() else {
                continue;
            };
            if node.kind == NodeKind::Function {
                index
                    .functions
                    .entry((path, &node.name))
                    .or_default()
                    .push(node);
            }
            if matches!(node.kind, NodeKind::Method | NodeKind::Constructor)
                && let Some(owner) = node.owner.as_deref()
            {
                index
                    .methods
                    .entry((path, owner, &node.name))
                    .or_default()
                    .push(node);
            }
            if matches!(node.kind, NodeKind::Struct | NodeKind::Enum) {
                index.types.insert((path, &node.name));
            }
        }
        for import in imports {
            index
                .imports
                .entry((&import.source_path, import.local_name()))
                .or_default()
                .push(import);
        }
        for module in modules {
            index
                .modules
                .entry(&module.module_path)
                .or_default()
                .push(module);
        }
        index
    }
}

fn unique_node<'a>(nodes: Option<&Vec<&'a CodeNode>>) -> Option<&'a CodeNode> {
    let nodes = nodes?;
    (nodes.len() == 1).then_some(nodes[0])
}

/// Résultat d'une tentative de résolution
/// des appels.
///
/// Nous séparons volontairement :
///
/// - les relations certaines
/// - les appels non résolus
///
/// Code Atlas ne doit jamais cacher
/// qu'il ne sait pas quelque chose.
pub struct CallResolutionResult {
    /// Relations CALLS que notre moteur
    /// a réussi à résoudre.
    pub edges: Vec<CodeEdge>,

    /// Appels trouvés mais pour lesquels
    /// aucune cible certaine n'a été déterminée.
    pub unresolved: Vec<CallReference>,
}

/// Résout un appel de méthode utilisant `self`.
///
/// Exemple :
///
/// impl User {
///
///     fn save(&self) {
///     }
///
///     fn process(&self) {
///
///         self.save();
///
///     }
/// }
///
/// Nous savons que `process` appartient à User.
///
/// Donc `self` représente User.
///
/// Nous pouvons rechercher :
///
///     owner = User
///     method = save
fn resolve_self_method<'a>(
    call: &CallReference,
    index: &ResolverIndex<'a>,
) -> Option<&'a CodeNode> {
    // Premièrement :
    //
    // retrouver le Node qui effectue l'appel.
    let caller = index.node_by_id.get(call.caller_id.as_str())?;

    // Le caller doit posséder un owner.
    //
    // Exemple :
    //
    // process
    // owner = User
    let owner = caller.owner.as_deref()?;

    unique_node(
        index
            .methods
            .get(&(call.path.as_str(), owner, call.target_name.as_str())),
    )
}

/// Tente de résoudre un appel associé
/// directement à un type.
///
/// Exemple :
///
///     User::new()
///
/// Si le fichier contient :
///
///     struct User
///
///     impl User {
///         fn new() {}
///     }
///
/// alors la cible peut être déterminée.
///
///
/// Pour le moment nous limitons cette résolution
/// au même fichier.
///
/// Pourquoi ?
///
/// Parce que résoudre correctement :
///
///     crate::user::User::new()
///
/// à travers plusieurs fichiers nécessite
/// déjà d'analyser les modules et les imports.
///
/// Nous le ferons plus tard.
fn resolve_scoped_method<'a>(
    call: &CallReference,
    index: &ResolverIndex<'a>,
) -> Option<&'a CodeNode> {
    let qualifier = call.qualifier.as_deref()?;

    // Pour le moment nous refusons volontairement
    // les chemins complexes.
    //
    // Exemple :
    //
    // auth::User
    //
    // sera traité plus tard lorsque nous
    // comprendrons les imports/modules.
    if qualifier.contains("::") {
        return None;
    }

    // Avant de chercher la méthode,
    // nous vérifions que le type correspondant
    // existe réellement dans ce fichier.
    //
    // Cela évite de supposer que :
    //
    // foo::bar()
    //
    // signifie forcément :
    //
    // Type foo -> méthode bar
    if !index.types.contains(&(call.path.as_str(), qualifier)) {
        return None;
    }
    unique_node(
        index
            .methods
            .get(&(call.path.as_str(), qualifier, call.target_name.as_str())),
    )
}

/// Essaie de transformer les CallReference
/// en véritables relations CALLS.
///
///
/// Principe fondamental :
///
///     certitude
///         ↓
///     CodeEdge
///
///
///     doute
///         ↓
///     unresolved
///
///
/// Nous n'essayons surtout pas de faire
/// une résolution "magique".
pub fn resolve_calls(
    code_nodes: &[CodeNode],
    calls: &[CallReference],
    imports: &[ImportReference],
    modules: &[ResolvedModule],
) -> CallResolutionResult {
    let index = ResolverIndex::new(code_nodes, imports, modules);
    let mut edges = Vec::new();

    let mut unresolved = Vec::new();

    for call in calls {
        // Selon la forme de l'appel,
        // nous utilisons une stratégie différente.
        let target = match call.kind {
            // Exemple :
            //
            // verify_password()
            CallKind::Function => {
                // Première tentative :
                //
                // la fonction est-elle définie
                // directement dans le même fichier ?
                let local = resolve_local_function(call, &index);

                if local.is_some() {
                    local
                } else {
                    // Deuxième tentative :
                    //
                    // la fonction vient-elle
                    // d'un import ?
                    resolve_imported_function(call, &index)
                }
            }

            // Exemple :
            //
            // self.save()
            //
            // Mais :
            //
            // user.save()
            //
            // n'est PAS encore résolu,
            // car nous ne connaissons pas
            // le type de `user`.
            CallKind::Method => {
                if call.qualifier.as_deref() == Some("self") {
                    resolve_self_method(call, &index)
                } else {
                    None
                }
            }

            // Exemple :
            //
            // User::new()
            CallKind::Scoped => resolve_scoped_method(call, &index),

            // Nous ne savons pas encore
            // interpréter suffisamment
            // cette forme.
            CallKind::Unknown => None,
        };

        // Si une cible certaine a été trouvée...
        if let Some(target_node) = target {
            // Création de :
            //
            // caller
            //    │
            //    │ CALLS
            //    ▼
            // target
            edges.push(CodeEdge::new(
                call.caller_id.clone(),
                target_node.id.clone(),
                RelationKind::Calls,
            ));
        } else {
            // Sinon nous conservons
            // le CallReference.
            //
            // Nous ne perdons aucune information.
            unresolved.push(call.clone());
        }
    }

    CallResolutionResult { edges, unresolved }
}
/// Recherche une fonction libre située
/// dans le même fichier que l'appel.
///
/// Exemple :
///
/// fn verify_password() {
/// }
///
/// fn login() {
///
///     verify_password();
///
/// }
///
/// Comme les deux fonctions sont dans
/// le même fichier et qu'une seule fonction
/// `verify_password` correspond,
/// nous pouvons considérer cette résolution
/// comme suffisamment sûre.
///
///
/// La fonction retourne uniquement une cible
/// lorsqu'elle en trouve EXACTEMENT une.
fn resolve_local_function<'a>(
    call: &CallReference,
    index: &ResolverIndex<'a>,
) -> Option<&'a CodeNode> {
    // On construit un iterator contenant
    // uniquement les candidats compatibles.
    unique_node(
        index
            .functions
            .get(&(call.path.as_str(), call.target_name.as_str())),
    )
}

/// Sépare le dernier élément d'un chemin Rust.
///
/// Exemple :
///
///     crate::auth::verify_password
///
/// devient :
///
/// module :
///
///     crate::auth
///
/// item :
///
///     verify_password
///
///
/// # Retour
///
/// Option<(&str, &str)>
///
/// Nous retournons des références
/// vers la String originale.
///
/// Aucune nouvelle String n'est créée.
fn split_rust_item_path(path: &str) -> Option<(&str, &str)> {
    // rsplit_once()
    //
    // cherche le DERNIER "::".
    //
    // C'est exactement ce que nous voulons.
    path.rsplit_once("::")
}
/// Tente de résoudre un appel de fonction
/// grâce aux imports du fichier.
///
///
/// Exemple :
///
///     use crate::auth::verify_password;
///
///     fn login() {
///
///         verify_password();
///
///     }
///
///
/// Étapes :
///
/// 1. trouver l'import appelé verify_password
///
/// 2. récupérer :
///
///        crate::auth::verify_password
///
/// 3. séparer :
///
///        module = crate::auth
///        item   = verify_password
///
/// 4. chercher le fichier correspondant
///    à crate::auth
///
/// 5. chercher verify_password
///    dans ce fichier
fn resolve_imported_function<'a>(
    call: &CallReference,
    index: &ResolverIndex<'a>,
) -> Option<&'a CodeNode> {
    // ================================================
    // ÉTAPE 1
    //
    // TROUVER L'IMPORT
    // ================================================

    let matching_imports = index
        .imports
        .get(&(call.path.as_str(), call.target_name.as_str()))?;
    let explicit = matching_imports
        .iter()
        .filter(|import| !import.is_wildcard)
        .collect::<Vec<_>>();
    if explicit.len() != 1 {
        return None;
    }
    let imported = explicit[0];

    // ================================================
    // ÉTAPE 2
    //
    // SÉPARER MODULE ET ITEM
    // ================================================

    let (module_path, item_name) = split_rust_item_path(&imported.full_path)?;

    // ================================================
    // ÉTAPE 3
    //
    // TROUVER LE MODULE
    // ================================================

    let matching_modules = index.modules.get(module_path)?;
    let resolved = matching_modules
        .iter()
        .filter(|module| module.status == ModuleResolutionStatus::Resolved)
        .collect::<Vec<_>>();
    if resolved.len() != 1 {
        return None;
    }
    let resolved_module = resolved[0];

    let target_path = resolved_module.target_path.as_deref()?;

    // ================================================
    // ÉTAPE 4
    //
    // TROUVER LA FONCTION DANS LE FICHIER
    // ================================================

    unique_node(index.functions.get(&(target_path, item_name)))
}
