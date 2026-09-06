# Code Atlas

Code Atlas is a local code intelligence workspace. It connects source analysis, graphs,
Features, findings, API exploration, documentation, tests, change estimates and Git review
in one React application backed by Rust and SQLite. A project assistant retrieves evidence
and opens the relevant source or workspace view through structured UI actions.

Core analysis works without an API key. Optional chat, reranking and neural embeddings use
an explicitly configured provider. Local concept vectors remain available offline and are
not presented as neural embeddings.

See [the workspace guide](docs/INTELLIGENCE_WORKSPACE.md) for workflows, API contracts,
persistence and limits, and [the delivery report](docs/DELIVERY_REPORT.md) for validation.

## Pipeline

```text
SOURCE CODE / GIT URL
        │
        ▼
Scanner + language detection + SHA-256
        │
        ▼
Tree-sitter analyzers / Cargo metadata
        │
        ▼
Symbols + imports + calls + unresolved references
        │
        ▼
Resolvers + framework adapters
        │
        ▼
Indexed ProjectGraph ──► search / impact / AI retrieval
        │
        ▼
SQLite ──► Axum API / WebSocket ──► React Flow + Monaco
```

## Requirements

- Rust stable (the lockfile currently resolves crates requiring Rust 1.97 or newer)
- Node.js 20+ and npm for rebuilding the frontend
- Git for analyzing remote Git repositories
- macOS or Linux

## Install and run

```bash
cd frontend
npm install
npm run build
cd ..
cargo run --release -- serve
```

Open <http://127.0.0.1:3000>. Select **Browse** to navigate the folders visible to the local
server, choose the project root, then select **Analyze**. An absolute path or HTTP(S) Git URL
can still be pasted directly. Git repositories are cloned with argument-safe `git clone`
into `.code-atlas/repos/`.

The server listens only on loopback by default. To change it:

```bash
cargo run --release -- serve --bind 0.0.0.0:3000 --db /safe/path/atlas.sqlite
```

CLI-only analysis is also available:

```bash
cargo run -- analyze /path/to/project
cargo run -- analyze /path/to/project --json
```

## Architecture

- `src/engine.rs`: end-to-end orchestration and hash-based AST reuse.
- `src/analyzer/`: Cargo/Rust analyzer plus the multi-language dispatcher.
- `src/framework/`: Next.js, Flutter/Riverpod/GoRouter, Rust/PHP/Laravel-style routes,
  inheritance and SQL schema adapters.
- `src/graph/`: indexed navigation, call/module resolution, fuzzy search and impact BFS.
- `src/storage/`: SQLite repository and migrations.
- `src/api/`: Axum REST API, source boundary checks and WebSocket events.
- `src/ai/`: provider abstraction, secure retrieval, OpenAI Responses and compatible chat
  providers for Mistral and OpenRouter.
- `src/watcher/`: portable filesystem watching with debounce.
- `frontend/`: React, TypeScript, React Flow and read-only Monaco inspector.

The graph uses stable textual IDs. Its indexes map IDs/names/paths and incoming/outgoing
edges to vector offsets, giving average O(1) lookup without unsafe self-references.

## Supported languages

| Language | Deterministic extraction |
| --- | --- |
| Rust | functions, methods, constructors, structs, enums, traits, aliases, constants/statics, modules, imports and calls |
| TypeScript / JavaScript | functions, components, classes, methods, interfaces/types, imports and calls |
| PHP | namespaces/imports, classes, interfaces, traits, functions, methods and calls |
| Dart | classes, widgets, functions and imports; conservative line parser |
| Python | modules/imports, classes, functions, methods and calls |
| Go | packages/imports, functions, receiver methods, structs and interfaces |

Rust calls support local/free functions, imports, `self.method()`, `Type::method()` and a
small local receiver inference for `let value: Type = ...` and `let value = Type::new()`.
Ambiguous calls remain in `unresolved_calls`; Code Atlas never creates a decorative guess.

## Framework support

- Next.js App Router pages and route handlers, including nested monorepos
- React component identification for named JSX/TSX functions
- Flutter widgets, Riverpod provider declarations and GoRouter route rendering
- Symfony route attributes and Laravel-style `Route::get/post/...` declarations
- Axum-style `.route("/path", get(handler))`
- SQL `CREATE TABLE` declarations
- simple `extends`, `implements`, and Rust `impl Trait for Type`

Framework recognition is intentionally isolated from syntax parsing.

## Graph model

Node kinds include Project, Workspace, Package, Crate, Module, File, Function, Method,
Constructor, Struct, Enum, Trait, Interface, TypeAlias, Class, Component, Page, Route,
ApiEndpoint, Provider and DatabaseTable. The model also reserves service, repository,
database, model, event, queue, WebSocket, configuration and environment kinds.

Relations include CONTAINS, DECLARES, ENTRY_POINT, IMPORTS, EXPORTS, CALLS, USES,
HAS_METHOD, EXTENDS, IMPLEMENTS, RETURNS, CREATES, READS, WRITES, RENDERS, ROUTES_TO,
HANDLED_BY, DEPENDS_ON, EMITS and LISTENS.

## API

```text
GET  /api/health
POST /api/projects/analyze
GET  /api/projects
GET  /api/projects/:id
GET  /api/projects/:id/graph
GET  /api/projects/:id/nodes?offset=0&limit=200
GET  /api/projects/:id/nodes/:node_id
GET  /api/projects/:id/nodes/:node_id/incoming
GET  /api/projects/:id/nodes/:node_id/outgoing
GET  /api/projects/:id/search?q=login&kind=Function
GET  /api/projects/:id/impact/:node_id?depth=4
POST /api/projects/:id/ai/query
GET  /api/fs/directories?path=/absolute/folder
WS   /api/events
```

Set `watch: true` in the analyze request to receive `file_changed` and `graph_updated`
events. Changed files are reparsed; unchanged `FileAnalysis` values are reused by hash,
then global resolution is safely rebuilt.

## Search and impact

Search filters by text, path, kind and language, ranks exact/prefix/substring matches and
uses a small subsequence fallback. Impact analysis follows incoming edges breadth-first to
a configurable depth (maximum 20), tracks visited IDs, and separates direct from transitive
impact.

## SQLite and cache

Migrations 001–008 run idempotently when the repository opens, retaining existing projects,
Features, findings and library data. SQLite stores graph snapshots, file hashes, durable
AST analysis cache and versioned AI/documentation outputs. Migrations 007 and 008 add
semantic units, conversations/messages and optional neural vectors with project-scoped
foreign keys and cascading deletion.

Semantic units and assistant citations contain redacted source excerpts in SQLite. They
are not a substitute for the source files: citations are revalidated against paths, ranges
and hashes. Embeddings are reused when their enriched content hash and model version
match. Test plans and estimates use the existing hash/version-based durable cache.

## AI configuration

Select the settings button in the application header to configure **OpenAI**, **Mistral** or
**OpenRouter**. The frontend saves the selected provider, API key and model in that browser's
local storage only. The key is sent through the local Rust server when an AI question is
submitted; it is never written to SQLite or the analyzed project. OpenAI uses Responses;
Mistral and OpenRouter use their OpenAI-compatible Chat Completions endpoints.

Suggested starting models are `gpt-5`, `mistral-large-latest` and `openrouter/auto`, but the
model field is editable so any model enabled for the corresponding account can be used.

Environment-based OpenAI configuration remains available as a server-side fallback:

```bash
export OPENAI_API_KEY="..."
export OPENAI_MODEL="gpt-5.6-luna"       # optional
export OPENAI_BASE_URL="https://api.openai.com/v1" # optional
export OPENAI_TEMPERATURE="0.2"          # optional
export OPENAI_MAX_OUTPUT_TOKENS="1200"   # optional
```

Requests use `store: false`. `.env`, key/PEM files and filenames suggesting secrets or
credentials are excluded from scanning/retrieval. Repository contents are labeled as
untrusted data in the model instructions. Tests use a mock provider and never require a key.

## Development and validation

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings

cd frontend
npm run lint
npm run build
npm test
npx playwright install chromium
npm run test:e2e
```

Fixtures under `tests/fixtures/` exercise Rust, TypeScript, PHP, Dart/Flutter, Python, Go,
Next.js, routes, providers and SQL. Integration tests cover the scanner, analyzers,
resolvers, search, impact, incremental reuse, SQLite, HTTP API and mock AI retrieval.

## Known limitations

- Dart uses a conservative deterministic parser rather than a Tree-sitter grammar; complex
  nested declarations may remain unknown.
- Resolution is intentionally not a compiler/LSP: dynamic dispatch, generated code, macro
  expansion, complex JS bundler aliases, Python runtime imports and PHP DI inference can
  remain unresolved.
- Composer PSR-4 mapping and Cargo dependency edges are not yet surfaced as dedicated graph
  edges, although Cargo workspace/package/target metadata is represented.
- ZIP upload/extraction is not implemented; extract the archive and analyze its folder.
- The graph UI renders at most 1,500 visible nodes at once. Filters and search are the
  intended zoom mechanism for larger projects.
- Provider-backed answers require a key. The global assistant also operates in local mode
  with deterministic retrieval, source citations and view actions.
- Estimates and taint paths are heuristic. Functional test coverage is unknown without
  verified assertions/results. Generated Playwright files currently validate navigation.
- Git associates diffs with the current graph; introduced/resolved findings require
  separate analyses of both revisions and are not claimed automatically.
- Frontend API-key persistence uses browser local storage as explicitly selected in settings;
  use Code Atlas only from its trusted local origin and clear site data to remove the key.

## Roadmap

Compiler/LSP-assisted resolution, broader framework metadata, historical graph comparison,
verified functional coverage and richer test generation remain areas for further work.
See the delivery report for the exact boundaries of this implementation.
