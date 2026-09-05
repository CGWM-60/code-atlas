# Audit initial et architecture cible

État initial : dépôt propre. Inventaire effectué avant modification applicative.

## Existant vérifié

- Rust : scanner, extracteurs multilangages, ProjectGraph indexé, résolution conservative, recherche exacte/fuzzy, impact et Flow.
- Knowledge : Features et memberships, Findings persistés, API Explorer et contrats, Library, documentation, portage avec aperçu et confirmation.
- SQLite : six migrations idempotentes, cache AST durable et snapshots, cache IA versionné, métadonnées Git. Le README décrit à tort le cache AST comme uniquement mémoire.
- MCP : outils réutilisant les services du domaine ; assistant actuel limité au contexte d’un nœud et sans conversation durable.
- Frontend : App.tsx (4358 lignes), composants Flow/API/source séparés ; navigation existante à conserver.

## Écarts vérifiés

- Pas de VectorStore, HybridRetriever, reranker, conversations ni UiAction.
- Pas de pages Tests, Estimate ou Git Review.
- App.tsx et routes.rs concentrent trop de responsabilités ; appels fetch dispersés.
- Le contexte existant recherche la question entière ; pas de boucle d’expansion ni vérification de fraîcheur des preuves.
- Aucun scénario Playwright versionné ; tests frontend principalement de rendu serveur.

## Architecture retenue

Conserver Scanner → AST → ProjectGraph → Repository. Ajouter des modules de domaine retrieval, assistant et intelligence ; des routes dédiées composées dans le routeur existant ; un client frontend et un panneau assistant séparés. Réutiliser les outils MCP et les vues source, Features, Findings et Flow. SQLite reste le seul stockage. Les réponses déterministes restent explicitement distinctes des réponses de provider ; aucun score heuristique ne vaut preuve de couverture ou de sécurité.

## Inventaire des fichiers

- `src/features.rs` (1595 lignes)
- `src/flow.rs` (361 lignes)
- `src/api_explorer.rs` (1234 lignes)
- `src/test_analysis.rs` (49 lignes)
- `src/my_code.rs` (203 lignes)
- `src/test_imports.rs` (25 lignes)
- `src/library.rs` (707 lignes)
- `src/lib.rs` (28 lignes)
- `src/documentation.rs` (341 lignes)
- `src/findings.rs` (974 lignes)
- `src/main.rs` (76 lignes)
- `src/porting.rs` (629 lignes)
- `src/context_engine.rs` (340 lignes)
- `src/mcp.rs` (834 lignes)
- `src/engine.rs` (525 lignes)
- `src/watcher/mod.rs` (68 lignes)
- `src/framework/mod.rs` (334 lignes)
- `src/framework/symfony.rs` (336 lignes)
- `src/graph/module_resolver.rs` (477 lignes)
- `src/graph/edge_builder.rs` (359 lignes)
- `src/graph/mod.rs` (9 lignes)
- `src/graph/impact.rs` (40 lignes)
- `src/graph/cargo_graph_builder.rs` (368 lignes)
- `src/graph/project_map.rs` (457 lignes)
- `src/graph/search.rs` (85 lignes)
- `src/graph/call_resolver.rs` (486 lignes)
- `src/graph/architecture.rs` (746 lignes)
- `src/graph/project_graph.rs` (259 lignes)
- `src/language/detector.rs` (102 lignes)
- `src/language/mod.rs` (138 lignes)
- `src/storage/mod.rs` (2 lignes)
- `src/storage/repository.rs` (1156 lignes)
- `src/ai/context_builder.rs` (149 lignes)
- `src/ai/provider.rs` (730 lignes)
- `src/ai/service.rs` (103 lignes)
- `src/ai/mod.rs` (4 lignes)
- `src/model/cargo.rs` (259 lignes)
- `src/model/analysis.rs` (117 lignes)
- `src/model/edge.rs` (353 lignes)
- `src/model/import.rs` (170 lignes)
- `src/model/mod.rs` (8 lignes)
- `src/model/project.rs` (91 lignes)
- `src/model/node.rs` (387 lignes)
- `src/model/module.rs` (199 lignes)
- `src/model/call.rs` (213 lignes)
- `src/api/mod.rs` (5 lignes)
- `src/api/routes.rs` (3152 lignes)
- `src/api/state.rs` (107 lignes)
- `src/api/dto.rs` (185 lignes)
- `src/scanner/project_scanner.rs` (152 lignes)
- `src/scanner/mod.rs` (1 lignes)
- `src/analyzer/generic_analyzer.rs` (409 lignes)
- `src/analyzer/dispatcher.rs` (21 lignes)
- `src/analyzer/cargo_analyzer.rs` (342 lignes)
- `src/analyzer/mod.rs` (4 lignes)
- `src/analyzer/rust_analyzer.rs` (1882 lignes)
- `frontend/src/App.tsx` (4358 lignes)
- `frontend/src/graphView.test.ts` (81 lignes)
- `frontend/src/FeatureSourceExplorer.tsx` (290 lignes)
- `frontend/src/main.tsx` (2 lignes)
- `frontend/src/styles.css` (271 lignes)
- `frontend/src/productViews.test.tsx` (32 lignes)
- `frontend/src/graphView.ts` (152 lignes)
- `frontend/src/vite-env.d.ts` (3 lignes)
- `frontend/src/ApiPage.tsx` (1024 lignes)
- `frontend/src/libraryView.test.tsx` (28 lignes)
- `frontend/src/FlowPage.tsx` (373 lignes)
- `frontend/src/EvidenceCodeInspector.tsx` (173 lignes)
- `tests/integration.rs` (950 lignes)
- `tests/fixtures/mixed/Cargo.toml` (4 lignes)
- `tests/fixtures/mixed/schema.sql` (1 lignes)
- `tests/fixtures/mixed/Cargo.lock` (7 lignes)
- `tests/fixtures/mixed/go/main.go` (5 lignes)
- `tests/fixtures/mixed/python/service.py` (4 lignes)
- `tests/fixtures/mixed/php/UserController.php` (7 lignes)
- `tests/fixtures/mixed/flutter/main.dart` (4 lignes)
- `tests/fixtures/mixed/src/auth.rs` (6 lignes)
- `tests/fixtures/mixed/src/main.rs` (3 lignes)
- `tests/fixtures/mixed/web/components/UserCard.tsx` (1 lignes)
- `tests/fixtures/mixed/web/app/users/page.tsx` (2 lignes)
- `tests/fixtures/mixed/web/app/api/users/route.ts` (2 lignes)
- `migrations/004_product_completion.sql` (46 lignes)
- `migrations/003_knowledge.sql` (94 lignes)
- `migrations/006_api_explorer.sql` (81 lignes)
- `migrations/002_registry_library.sql` (81 lignes)
- `migrations/005_finding_scan_cache.sql` (6 lignes)
- `migrations/001_initial.sql` (12 lignes)

## Routes existantes

- `/api/events`
- `/api/features/{feature_id}/compare`
- `/api/features/{feature_id}/export`
- `/api/features/{feature_id}/library`
- `/api/features/{feature_id}/port/apply`
- `/api/features/{feature_id}/port/generate`
- `/api/features/{feature_id}/port/plan`
- `/api/features/{feature_id}/similar`
- `/api/findings/{id}`
- `/api/findings/{id}/explain`
- `/api/fs/directories`
- `/api/health`
- `/api/library`
- `/api/library/features`
- `/api/library/features/{id}`
- `/api/library/{id}`
- `/api/library/{id}/generate-documentation`
- `/api/mcp/status`
- `/api/projects`
- `/api/projects/analyze`
- `/api/projects/{id}`
- `/api/projects/{id}/ai/query`
- `/api/projects/{id}/api/collections`
- `/api/projects/{id}/api/collections/{collection_id}/run`
- `/api/projects/{id}/api/contracts`
- `/api/projects/{id}/api/contracts/{test_id}/run`
- `/api/projects/{id}/api/curl`
- `/api/projects/{id}/api/endpoints`
- `/api/projects/{id}/api/endpoints/{endpoint_id}`
- `/api/projects/{id}/api/environments`
- `/api/projects/{id}/api/execute`
- `/api/projects/{id}/api/history`
- `/api/projects/{id}/api/openapi`
- `/api/projects/{id}/api/requests`
- `/api/projects/{id}/context`
- `/api/projects/{id}/dashboard`
- `/api/projects/{id}/docs`
- `/api/projects/{id}/docs/generate`
- `/api/projects/{id}/docs/markdown`
- `/api/projects/{id}/features`
- `/api/projects/{id}/features/detect`
- `/api/projects/{id}/features/merge`
- `/api/projects/{id}/features/{feature_id}`
- `/api/projects/{id}/features/{feature_id}/accept`
- `/api/projects/{id}/features/{feature_id}/code`
- `/api/projects/{id}/features/{feature_id}/docs`
- `/api/projects/{id}/features/{feature_id}/graph`
- `/api/projects/{id}/features/{feature_id}/members`
- `/api/projects/{id}/features/{feature_id}/source`
- `/api/projects/{id}/features/{feature_id}/source/file`
- `/api/projects/{id}/features/{feature_id}/spec`
- `/api/projects/{id}/features/{feature_id}/split`
- `/api/projects/{id}/flow/trace`
- `/api/projects/{id}/graph`
- `/api/projects/{id}/impact/{node_id}`
- `/api/projects/{id}/library-candidates`
- `/api/projects/{id}/map`
- `/api/projects/{id}/my-code`
- `/api/projects/{id}/nodes`
- `/api/projects/{id}/nodes/{node_id}`
- `/api/projects/{id}/nodes/{node_id}/incoming`
- `/api/projects/{id}/nodes/{node_id}/outgoing`
- `/api/projects/{id}/quality`
- `/api/projects/{id}/search`
- `/api/projects/{id}/security`
- `/api/projects/{id}/security/review`
- `/api/projects/{id}/view`
