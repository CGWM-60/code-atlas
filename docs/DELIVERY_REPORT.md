# Rapport de livraison — Code Atlas

Validation locale du 6 septembre 2026. Ce rapport décrit les fonctionnalités intégrées et les limites effectivement observées. Le cahier des charges complet n’est pas déclaré satisfait à 100 % : les écarts restants sont détaillés en J.

## A. Fonctionnalités ajoutées

- Recherche hybride avec unités sémantiques persistées, vecteurs locaux, scores explicables et reranking déterministe.
- Embeddings neuronaux optionnels OpenAI/Mistral/OpenRouter, modèle éditable, incrémentalité, reprise par lots et recherche hybride associée.
- Assistant global, conversations SQLite par projet, citations vérifiées, outils internes, stratégies spécialisées et actions de navigation.
- Palette de commandes, panneau redimensionnable, reprise/renommage/suppression des conversations et événements d’activité.
- Pages Tests, Estimation, Git et Dépendances reliées aux API et aux éléments du graphe.
- Propagation taint interprocédurale bornée et étapes source/appel/sink cliquables.

## B. Fonctionnalités complétées

- Réutilisation du scanner, du graphe, des Features, des Findings, de Flow, d’Impact, de Library, d’API Explorer, de la documentation et de MCP.
- Endpoints Next.js distincts pour les exports HTTP observés, utilisables dans API Explorer.
- Navigation vers le fichier et la plage d’une preuve, y compris entre deux fichiers.
- Invalidation de documentation lors d’un changement du corps source sans changement d’identifiant de symbole.
- Estimation enrichie par impact, dépendances, API, sécurité, besoins de tests et signaux de complexité.
- Git : décodage des chemins avec espaces/UTF-8/guillemets, références validées, associations au graphe actuel et recommandations de revue.
- Documentation Markdown avec sommaire ; vues lisibles pour l’impact, les documents de Feature, les spécifications consultées et les plans de portage.
- Correction de clés React dupliquées qui laissaient plusieurs anciennes pages montées.

## C. Architecture modifiée

Le pipeline existant Scanner → AST → ProjectGraph → Repository est conservé. Les nouveaux domaines sont isolés dans `retrieval`, `embeddings`, `assistant`, `assistant_tools`, `intelligence`, `taint` et `dependencies`. Les routes d’intelligence sont composées dans le routeur existant.

Le frontend sépare pages de connaissance, pages d’intelligence, assistant, palette, actions UI, client API, contrats et composants documentaires. Cette séparation réduit les ajouts dans App.tsx, sans prétendre avoir achevé le découpage de tout le code historique.

## D. Migrations ajoutées

- **007_intelligence.sql** : unités sémantiques, conversations, messages, indexes et relations par projet.
- **008_neural_embeddings.sql** : vecteurs neuronaux versionnés, hashes et dates, avec clé étrangère composite vers les unités sémantiques.

Les migrations sont additives/idempotentes. Les tests vérifient la réouverture, la conservation des projets/conversations et les suppressions en cascade. Aucune remise à zéro de la base utilisateur n’a été effectuée pour les tests navigateur ; ceux-ci utilisent une base temporaire distincte.

## E. Pages UI créées/modifiées

| Vue | Résultat intégré / observation navigateur |
| --- | --- |
| Projets | Analyse d’un vrai dossier temporaire depuis l’UI |
| Carte | Graphe affiché et navigation par symbole |
| Flux | Vue existante conservée, accessible par assistant ; état initial explicite |
| Recherche | Recherche hybride, scores, réindexation et commandes neuronales |
| Features | Inspection et source multifichiers ; documents lisibles |
| Mon code | Page conservée dans le module de connaissance |
| Bibliothèque | Navigation et état vide explicite ; spécification lisible |
| API | Endpoint Next.js observé affiché dans l’explorateur |
| Documentation | Chargement, Markdown et table des matières |
| Qualité / Sécurité | Filtres, statuts et preuves ; ouverture source puis sink testée |
| Tests | Scénarios, tests liés, couverture inconnue et téléchargement Playwright |
| Estimation | Mission, périmètre, heures en fourchettes et analyses associées |
| Git | Diff réel, anciennes/nouvelles lignes, impacts candidats et recommandations |
| Dépendances | Packages réellement lus dans le manifeste, filtres et provenance |
| MCP | Vue de capacités conservée ; six nouveaux outils |
| Assistant | Question, action automatique, suivi Tests, fermeture/reprise et renommage testés |

Les captures sont produites sous `output/playwright/`. Le parcours compact est vérifié à 820 px sans débordement horizontal du document ; palette au clavier et fermeture Échap sont testées. Cela ne constitue pas un audit exhaustif de chaque combinaison de page, état d’erreur, appareil et technologie d’assistance.

## F. Assistant et outils disponibles

Rôles : ArchitectureAgent, FeatureAgent, SecurityAgent, QualityAgent, TestingAgent, DocumentationAgent, EstimationAgent, GitReviewAgent.

Outils internes : `search_code`, `semantic_search`, `search_symbols`, `get_node`, `get_source`, `get_callers`, `get_callees`, `get_dependencies`, `get_impact`, `list_features`, `get_feature`, `get_feature_graph`, `get_feature_source`, `find_similar_features`, `get_security_findings`, `get_quality_findings`, `explain_finding`, `trace_flow`, `list_api_endpoints`, `get_api_endpoint`, `get_api_contract`, `get_library_entries`, `get_project_docs`, `get_context`, `get_git_diff`, `estimate_change`, `generate_test_plan`, `get_dependency_inventory`.

Le registre réutilise les services/MCP existants. L’assistant observe des limites d’expansion, de sources, de contexte et d’outils. Le budget empêche de nouvelles opérations au lieu de tronquer a posteriori leur journal. Les réponses locales et celles d’un provider sont distinguées.

## G. UI Actions disponibles

`NAVIGATE_PAGE`, `OPEN_NODE`, `FOCUS_NODE`, `SELECT_NODE`, `OPEN_FILE`, `OPEN_SOURCE_RANGE`, `HIGHLIGHT_SOURCE_RANGE`, `OPEN_FEATURE`, `OPEN_FEATURE_SOURCE`, `OPEN_FEATURE_GRAPH`, `OPEN_FINDING`, `OPEN_SECURITY`, `OPEN_QUALITY`, `OPEN_FLOW`, `TRACE_FLOW`, `OPEN_IMPACT`, `OPEN_API`, `OPEN_API_ENDPOINT`, `OPEN_LIBRARY_ENTRY`, `OPEN_DOCUMENTATION`, `FOCUS_GRAPH`, `FIT_GRAPH`, `FILTER_GRAPH`, `SHOW_RELATED_NODES`, `SHOW_DIFF`, `SHOW_TEST_PLAN`, `SHOW_ESTIMATE`.

Les citations peuvent porter leur hash de validation. `SHOW_DIFF` transporte éventuellement Base/Cible. Les actions sont exécutées par un dispatcher explicite ; le mode automatique ouvre la première action, les autres restent proposées.

## H. Tests ajoutés

15 tests Rust dans `tests/intelligence.rs` couvrent notamment : persistance/isolation du vector store, invalidation incrémentale, recherche française et exacte, conversations et preuves, estimation/couverture honnête, Git en lecture seule et injection de référence, migrations/réouverture, HTTP réel, endpoints Next.js, cache documentaire, orchestration avec provider de test, embeddings normalisés/invalidation, six écosystèmes de dépendances et taint positif/négatif.

4 tests frontend du dispatcher couvrent plages source, rejet des plages invalides, destinations Feature/graphe et vues Finding/Tests/Estimate/Git. Les tests frontend existants sont conservés.

Un scénario Playwright intégré utilise le serveur Rust réel et un dépôt temporaire : analyse, carte, Feature, source, palette, question assistant, action automatique, suivi Tests, estimation, Git, API, preuve sécurité entre fichiers, dépendances, reprise/renommage, parcours des pages et clavier à largeur réduite. Aucun provider distant n’est simulé en permanence dans le produit.

## I. Résultats des commandes de validation

Commandes réellement exécutées via le wrapper RTK du workspace :

| Commande | Résultat |
| --- | --- |
| `cargo fmt --check` | OK |
| `cargo check` | OK |
| `cargo test` | **58 tests réussis**, 4 suites |
| `cargo clippy --all-targets --all-features -- -D warnings` | OK après correction d’une copie d’itérateur inutile |
| `npm run lint` | OK — vérification TypeScript de l’application |
| `npm run build` | OK — bundle produit |
| `npm test` | **14 tests réussis**, 4 fichiers |
| `npm run test:e2e` | **1 scénario intégré réussi** sur Chromium |

Avertissement non bloquant : Vite signale un bundle JavaScript d’environ 709 Ko minifié (217 Ko gzip), supérieur à son seuil de 500 Ko. Les dépendances nécessaires ont été installées pendant l’implémentation ; aucune réinstallation complète n’était nécessaire pour cette dernière validation.

## J. Limites restantes et exigences partielles

- Les appels réels aux providers externes n’ont pas été validés avec des clés facturées. La disponibilité et les limites exactes du modèle choisi restent dépendantes du compte.
- Les vecteurs locaux sont déterministes, pas neuronaux. Les unités trop grandes sont signalées/ignorées côté neuronal. La recherche SQLite charge les vecteurs du projet : aucun benchmark de très gros dépôt n’a été effectué.
- `framework` reste parfois inconnu. Les unités virtuelles sans source et tous les niveaux sémantiques possibles ne sont pas systématiquement vectorisés.
- L’assistant local est un ensemble de stratégies déterministes. La vérification des sources ne prouve pas toutes les conclusions, relations mentionnées en prose ou règles métier d’une réponse IA.
- L’activité est diffusée par WebSocket ; le texte final ne bénéficie pas encore d’un streaming token par token.
- La génération de tests est actuellement limitée à des tests Playwright de navigation et à des scénarios proposés. La génération complète unit/integration/API/métier et le calcul d’une couverture fonctionnelle vérifiée restent à compléter.
- Les estimations sont heuristiques et non calibrées ; aucune précision contractuelle n’est revendiquée.
- Git n’analyse pas deux snapshots historiques du graphe. Les findings introduits/résolus et l’impact architectural historique ne sont donc pas automatiquement établis. Les binaires/non suivis ne sont pas inclus.
- Taint est borné et incomplet pour alias, conditions, nettoyages et certains langages. Ce n’est pas une preuve complète d’exploitabilité.
- Les formats de dépendances complexes ne sont pas tous résolus ; aucune donnée externe de vulnérabilité n’est intégrée.
- Le découpage historique d’App.tsx/routes.rs, la centralisation de tous les anciens appels `fetch` et de tout l’état, ainsi que le découpage du bundle restent partiels. Les contrats sont centralisés manuellement, pas générés depuis Rust.
- L’homogénéisation UI n’est pas exhaustive : certains libellés historiques restent en anglais ; l’audit de tous les états chargement/vide/erreur de chaque page et l’accessibilité complète restent à étendre.

Ces points empêchent de déclarer le cahier des charges intégralement terminé ou le produit certifié prêt pour tous les usages professionnels.

## K. Fichiers majeurs

Backend : `src/retrieval.rs`, `src/embeddings.rs`, `src/assistant.rs`, `src/assistant_tools.rs`, `src/intelligence.rs`, `src/taint.rs`, `src/dependencies.rs`, `src/api/intelligence.rs`, `src/api/state.rs`, `src/storage/repository.rs`, `src/findings.rs`, `src/documentation.rs`, `src/framework/mod.rs`, `src/mcp.rs`, `src/lib.rs`.

SQLite : `migrations/007_intelligence.sql`, `migrations/008_neural_embeddings.sql`.

Frontend : `frontend/src/App.tsx`, `frontend/src/features/assistant/`, `frontend/src/pages/`, `frontend/src/components/KnowledgeDocument.tsx`, `frontend/src/components/ImpactView.tsx`, `frontend/src/services/api.ts`, `frontend/src/state/uiActions.ts`, `frontend/src/types/contracts.ts`, `frontend/src/EvidenceCodeInspector.tsx`, `frontend/src/styles.css`.

Validation/documentation : `tests/intelligence.rs`, `frontend/src/state/uiActions.test.ts`, `frontend/e2e/workspace.spec.ts`, `frontend/playwright.config.ts`, `frontend/package.json`, `Cargo.toml`, lockfiles, `README.md`, `docs/PRODUCT_AUDIT.md`, `docs/INTELLIGENCE_WORKSPACE.md`, ce rapport.
