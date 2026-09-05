import { useEffect, useMemo, useState, type ChangeEvent } from "react";
import {
  Braces,
  Copy,
  Download,
  GitBranch,
  Play,
  Save,
  Search,
  Upload,
} from "lucide-react";

type ApiParameter = {
  name: string;
  location: "path" | "query" | "header" | "cookie";
  required: boolean;
  schema_type: string;
  source: string;
};
type ApiResponseMetadata = {
  status: number;
  content_type?: string;
  schema?: unknown;
  source: string;
};
type ApiEndpoint = {
  id: string;
  project_id: string;
  method: string;
  path: string;
  name: string;
  framework: string;
  transport: string;
  handler_node_id?: string;
  source_path?: string;
  source_start_line?: number;
  source_end_line?: number;
  auth_requirements: string[];
  parameters: ApiParameter[];
  request_content_types: string[];
  request_schema?: unknown;
  responses: ApiResponseMetadata[];
  tags: string[];
  feature_ids: string[];
  provenance: string;
  security_finding_ids: string[];
  quality_finding_ids: string[];
  has_tests: boolean;
};
type Coverage = {
  total: number;
  by_method: Record<string, number>;
  documented: number;
  request_schema: number;
  response_schema: number;
  tested: number;
  security_findings: number;
  untested: number;
};
type ApiResponse = {
  status: number;
  duration_ms: number;
  size: number;
  headers: Record<string, string>;
  cookies: string[];
  body: string;
  content_type?: string;
  truncated: boolean;
};
type History = {
  id: string;
  method: string;
  url: string;
  status?: number;
  duration_ms?: number;
  response_size?: number;
  created_at: number;
};
type SavedRequest = { id: string; name: string; request: ApiRequest };
type Collection = {
  id: string;
  name: string;
  request_ids: string[];
  variable_extractions: Record<string, string>;
};
type ContractTest = {
  id: string;
  name: string;
  assertions: Array<{ kind: string; key?: string; expected?: unknown }>;
};
type ApiRequest = {
  endpoint_id?: string;
  method: string;
  url: string;
  headers: Record<string, string>;
  body?: string;
  environment: string;
  confirm_destructive: boolean;
};
type GroupMode = "path" | "framework" | "feature";
type ResponseTab = "pretty" | "raw" | "headers" | "cookies";

export function ApiPage({
  projectId,
  onInspect,
  onTrace,
}: {
  projectId: string;
  onInspect: (id: string) => void;
  onTrace: (id: string) => void;
}) {
  const [endpoints, setEndpoints] = useState<ApiEndpoint[]>([]);
  const [coverage, setCoverage] = useState<Coverage | null>(null);
  const [selected, setSelected] = useState<ApiEndpoint | null>(null);
  const [query, setQuery] = useState("");
  const [groupMode, setGroupMode] = useState<GroupMode>("path");
  const [baseUrl, setBaseUrl] = useState(
    () =>
      localStorage.getItem("code-atlas.api-base." + projectId) ??
      "http://127.0.0.1:3000",
  );
  const [url, setUrl] = useState("");
  const [method, setMethod] = useState("GET");
  const [headers, setHeaders] = useState("{}");
  const [body, setBody] = useState("");
  const [environment, setEnvironment] = useState("local");
  const [authMode, setAuthMode] = useState("none");
  const [authName, setAuthName] = useState("Authorization");
  const [authValue, setAuthValue] = useState("");
  const [response, setResponse] = useState<ApiResponse | null>(null);
  const [responseTab, setResponseTab] = useState<ResponseTab>("pretty");
  const [status, setStatus] = useState("");
  const [history, setHistory] = useState<History[]>([]);
  const [saved, setSaved] = useState<SavedRequest[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [contracts, setContracts] = useState<ContractTest[]>([]);
  const [clientLanguage, setClientLanguage] = useState("fetch");
  useEffect(() => {
    void refresh();
  }, [projectId]);
  async function refresh() {
    const [endpointResponse, historyResponse, savedResponse, collectionResponse, contractResponse] = await Promise.all([
      fetch("/api/projects/" + projectId + "/api/endpoints"),
      fetch("/api/projects/" + projectId + "/api/history"),
      fetch("/api/projects/" + projectId + "/api/requests"),
      fetch("/api/projects/" + projectId + "/api/collections"),
      fetch("/api/projects/" + projectId + "/api/contracts"),
    ]);
    if (endpointResponse.ok) {
      const value = await endpointResponse.json();
      setEndpoints(value.items);
      setCoverage(value.coverage);
      if (!selected && value.items[0]) selectEndpoint(value.items[0]);
    }
    if (historyResponse.ok) setHistory((await historyResponse.json()).items);
    if (savedResponse.ok) setSaved((await savedResponse.json()).items);
    if (collectionResponse.ok)
      setCollections((await collectionResponse.json()).items);
    if (contractResponse.ok)
      setContracts((await contractResponse.json()).items);
  }
  function selectEndpoint(endpoint: ApiEndpoint) {
    setSelected(endpoint);
    setMethod(endpoint.method === "ANY" ? "GET" : endpoint.method);
    setUrl(baseUrl.replace(/\/$/, "") + endpoint.path);
    const defaults = Object.fromEntries(
      endpoint.parameters
        .filter((parameter) => parameter.location === "header")
        .map((parameter) => [parameter.name, ""]),
    );
    setHeaders(JSON.stringify(defaults, null, 2));
    setBody(
      endpoint.request_schema
        ? JSON.stringify(exampleFromSchema(endpoint.request_schema), null, 2)
        : "",
    );
    setResponse(null);
  }
  function parsedHeaders() {
    try {
      return JSON.parse(headers) as Record<string, string>;
    } catch {
      throw new Error("Le JSON des en-têtes est invalide.");
    }
  }
  function currentRequest(confirm_destructive = false): ApiRequest {
    const values = parsedHeaders();
    if (authMode === "bearer" && authValue)
      values.Authorization = "Bearer " + authValue;
    if (authMode === "basic" && authValue)
      values.Authorization = "Basic " + btoa(authValue);
    if (authMode === "api-header" && authValue)
      values[authName || "X-API-Key"] = authValue;
    if (authMode === "api-query" && authValue) {
      const parsed = new URL(url);
      parsed.searchParams.set(authName || "api_key", authValue);
      return {
        endpoint_id: selected?.id,
        method,
        url: parsed.toString(),
        headers: values,
        body: body || undefined,
        environment,
        confirm_destructive,
      };
    }
    if (authMode === "cookie" && authValue) values.Cookie = authValue;
    return {
      endpoint_id: selected?.id,
      method,
      url,
      headers: values,
      body: body || undefined,
      environment,
      confirm_destructive,
    };
  }
  async function send() {
    try {
      const destructive = ["POST", "PUT", "PATCH", "DELETE"].includes(method);
      const remote = !/^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])/.test(url);
      const needsConfirm =
        destructive &&
        (environment === "production" ||
          environment === "custom" ||
          (remote && method === "DELETE"));
      const confirm = needsConfirm
        ? window.confirm(
            "Cette requête peut modifier des données distantes. Confirmer l’envoi ?",
          )
        : false;
      if (needsConfirm && !confirm) return;
      setStatus("Envoi de la requête…");
      const result = await fetch(
        "/api/projects/" + projectId + "/api/execute",
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(currentRequest(confirm)),
        },
      );
      const value = await result.json();
      if (!result.ok) throw new Error(value.error ?? "Échec HTTP");
      setResponse(value);
      setResponseTab("pretty");
      setStatus("Réponse " + value.status + " en " + value.duration_ms + " ms");
      void refresh();
    } catch (reason) {
      setStatus(reason instanceof Error ? reason.message : "Requête invalide");
    }
  }
  async function copyCurl() {
    try {
      const result = await fetch("/api/projects/" + projectId + "/api/curl", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(currentRequest()),
      });
      const value = await result.json();
      if (result.ok) {
        await navigator.clipboard.writeText(value.curl);
        setStatus("Commande cURL copiée");
      }
    } catch (reason) {
      setStatus(
        reason instanceof Error ? reason.message : "Impossible de générer cURL",
      );
    }
  }
  async function saveRequest() {
    const name = window
      .prompt(
        "Nom de la requête sauvegardée",
        selected?.name ?? "Nouvelle requête",
      )
      ?.trim();
    if (!name) return;
    try {
      const result = await fetch(
        "/api/projects/" + projectId + "/api/requests",
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            name,
            favorite: false,
            request: currentRequest(),
          }),
        },
      );
      const value = await result.json();
      setStatus(
        result.ok
          ? "Requête sauvegardée sans credentials"
          : (value.error ?? "Sauvegarde impossible"),
      );
      void refresh();
    } catch (reason) {
      setStatus(
        reason instanceof Error ? reason.message : "Sauvegarde impossible",
      );
    }
  }
  async function createContract() {
    if (!selected) return;
    const expected = Number(
      window.prompt("Code de statut attendu", "200") ?? "200",
    );
    try {
      const result = await fetch(
        "/api/projects/" + projectId + "/api/contracts",
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            name: "Contrat " + selected.method + " " + selected.path,
            request: currentRequest(),
            assertions: [{ kind: "status", expected }],
          }),
        },
      );
      const value = await result.json();
      setStatus(
        result.ok
          ? "Test de contrat sauvegardé"
          : (value.error ?? "Création impossible"),
      );
    } catch (reason) {
      setStatus(
        reason instanceof Error ? reason.message : "Création impossible",
      );
    }
  }
  async function createCollection() {
    const name = window.prompt("Nom de la collection")?.trim();
    if (!name) return;
    const requestIds = saved
      .filter((request) =>
        window.confirm("Ajouter « " + request.name + " » à la collection ?"),
      )
      .map((request) => request.id);
    const extraction = window
      .prompt(
        "Extraction optionnelle depuis chaque réponse (exemple : token=$.token)",
        "",
      )
      ?.trim();
    const variableExtractions: Record<string, string> = {};
    if (extraction) {
      const separator = extraction.indexOf("=");
      if (separator <= 0 || !extraction.slice(separator + 1).trim()) {
        setStatus("Extraction invalide : utilise nom=$.chemin");
        return;
      }
      variableExtractions[extraction.slice(0, separator).trim()] = extraction
        .slice(separator + 1)
        .trim();
    }
    const result = await fetch(
      "/api/projects/" + projectId + "/api/collections",
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          name,
          request_ids: requestIds,
          variable_extractions: variableExtractions,
        }),
      },
    );
    setStatus(
      result.ok
        ? "Collection sauvegardée"
        : "Création de collection impossible",
    );
    void refresh();
  }
  async function runCollection(collection: Collection) {
    const result = await fetch(
      "/api/projects/" +
        projectId +
        "/api/collections/" +
        encodeURIComponent(collection.id) +
        "/run",
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          headers: authHeaders(authMode, authName, authValue),
          variables: { base_url: baseUrl },
          confirm_destructive: false,
        }),
      },
    );
    const value = await result.json();
    setStatus(
      result.ok
        ? "Collection exécutée : " + value.runs.length + " requête(s)"
        : (value.error ?? "Échec de collection"),
    );
  }
  async function runContract(contract: ContractTest) {
    const result = await fetch(
      "/api/projects/" +
        projectId +
        "/api/contracts/" +
        encodeURIComponent(contract.id) +
        "/run",
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          headers: authHeaders(authMode, authName, authValue),
          variables: { base_url: baseUrl },
          confirm_destructive: false,
        }),
      },
    );
    const value = await result.json();
    setStatus(
      result.ok
        ? value.passed
          ? "Test de contrat réussi"
          : "Test de contrat échoué : " +
            value.results.filter((item: { passed: boolean }) => !item.passed)
              .length +
            " assertion(s)"
        : (value.error ?? "Échec du test de contrat"),
    );
    if (result.ok) setResponse(value.response);
  }
  async function exportOpenApi() {
    const result = await fetch("/api/projects/" + projectId + "/api/openapi");
    if (!result.ok) return;
    const document = await result.json();
    downloadJson("openapi-code-atlas.json", document);
  }
  async function importOpenApi(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) return;
    try {
      const document = JSON.parse(await file.text());
      const result = await fetch(
        "/api/projects/" + projectId + "/api/openapi",
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(document),
        },
      );
      const value = await result.json();
      setStatus(
        result.ok
          ? "OpenAPI fusionné avec " + value.merged + " endpoint(s)"
          : (value.error ?? "Import invalide"),
      );
      void refresh();
    } catch {
      setStatus("Le fichier OpenAPI JSON est invalide.");
    }
  }
  const filtered = useMemo(
    () =>
      endpoints.filter((endpoint) =>
        [
          endpoint.path,
          endpoint.method,
          endpoint.framework,
          endpoint.handler_node_id ?? "",
          ...endpoint.tags,
        ]
          .join(" ")
          .toLowerCase()
          .includes(query.toLowerCase()),
      ),
    [endpoints, query],
  );
  const groups = useMemo(
    () => groupEndpoints(filtered, groupMode),
    [filtered, groupMode],
  );
  const prettyBody = useMemo(() => {
    if (!response) return "";
    try {
      return JSON.stringify(JSON.parse(response.body), null, 2);
    } catch {
      return response.body;
    }
  }, [response]);
  const clientCode = useMemo(
    () =>
      clientSnippet(
        clientLanguage,
        currentRequestSafe(method, url, headers, body),
      ),
    [clientLanguage, method, url, headers, body],
  );
  return (
    <section className="page-view api-page">
      <div className="page-heading">
        <div>
          <span>Découverte automatique · multi-framework</span>
          <h1>API Explorer</h1>
          <p>
            Inspecte, trace et teste les endpoints réellement détectés dans le
            ProjectGraph.
          </p>
        </div>
        <div className="api-heading-actions">
          <button onClick={() => void exportOpenApi()}>
            <Download size={14} />
            OpenAPI
          </button>
          <label>
            <Upload size={14} />
            Importer
            <input
              type="file"
              accept=".json,application/json"
              onChange={(event) => void importOpenApi(event)}
            />
          </label>
        </div>
      </div>
      {coverage && (
        <div className="api-coverage">
          <b>
            {coverage.total}
            <small>Endpoints</small>
          </b>
          <b>
            {coverage.tested}
            <small>Testés</small>
          </b>
          <b>
            {coverage.request_schema}
            <small>Schéma requête</small>
          </b>
          <b>
            {coverage.response_schema}
            <small>Schéma réponse</small>
          </b>
          <b>
            {coverage.security_findings}
            <small>Avec sécurité</small>
          </b>
          <b>
            {coverage.untested}
            <small>Non testés</small>
          </b>
        </div>
      )}
      <div className="api-layout">
        <aside className="api-tree">
          <div className="search">
            <Search size={14} />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Méthode, route, handler…"
            />
          </div>
          <select
            value={groupMode}
            onChange={(event) => setGroupMode(event.target.value as GroupMode)}
          >
            <option value="path">Grouper par chemin</option>
            <option value="framework">Grouper par framework</option>
            <option value="feature">Grouper par Feature</option>
          </select>
          {[...groups].map(([group, items]) => (
            <section key={group}>
              <h3>
                {group}
                <span>{items.length}</span>
              </h3>
              {items.map((endpoint) => (
                <button
                  className={selected?.id === endpoint.id ? "active" : ""}
                  key={endpoint.id}
                  onClick={() => selectEndpoint(endpoint)}
                >
                  <em className={endpoint.method.toLowerCase()}>
                    {endpoint.method}
                  </em>
                  <b>{endpoint.path}</b>
                  <small>{endpoint.framework}</small>
                </button>
              ))}
            </section>
          ))}
        </aside>
        <main className="api-workbench">
          {selected ? (
            <>
              <header>
                <div>
                  <span>
                    {selected.framework} · {selected.provenance}
                  </span>
                  <h2>
                    <em>{method}</em> {selected.path}
                  </h2>
                  <p>{selected.handler_node_id ?? "Handler non résolu"}</p>
                </div>
                <div>
                  <button
                    onClick={() =>
                      onInspect(selected.handler_node_id ?? selected.id)
                    }
                  >
                    Voir le code
                  </button>
                  <button onClick={() => onTrace(selected.id)}>
                    <GitBranch size={13} />
                    Tracer le flux
                  </button>
                </div>
              </header>
              <div className="api-metadata">
                <span>{selected.parameters.length} paramètres</span>
                <span>{selected.responses.length} réponses</span>
                <span>{selected.feature_ids.length} Features</span>
                <span>{selected.security_finding_ids.length} Security</span>
                <span>{selected.quality_finding_ids.length} Quality</span>
              </div>
              <div className="api-request">
                <div className="api-url">
                  <select
                    value={method}
                    onChange={(event) => setMethod(event.target.value)}
                  >
                    {[
                      "GET",
                      "POST",
                      "PUT",
                      "PATCH",
                      "DELETE",
                      "OPTIONS",
                      "HEAD",
                    ].map((value) => (
                      <option key={value}>{value}</option>
                    ))}
                  </select>
                  <input
                    value={url}
                    onChange={(event) => setUrl(event.target.value)}
                  />
                  <button className="primary" onClick={() => void send()}>
                    <Play size={14} />
                    Envoyer
                  </button>
                </div>
                <div className="api-request-grid">
                  <label>
                    Base URL
                    <input
                      value={baseUrl}
                      onChange={(event) => {
                        setBaseUrl(event.target.value);
                        localStorage.setItem(
                          "code-atlas.api-base." + projectId,
                          event.target.value,
                        );
                      }}
                    />
                  </label>
                  <label>
                    Environnement
                    <select
                      value={environment}
                      onChange={(event) => setEnvironment(event.target.value)}
                    >
                      <option value="local">Local</option>
                      <option value="development">Development</option>
                      <option value="staging">Staging</option>
                      <option value="production">Production</option>
                      <option value="custom">Custom</option>
                    </select>
                  </label>
                  <label>
                    Authentification
                    <select
                      value={authMode}
                      onChange={(event) => setAuthMode(event.target.value)}
                    >
                      <option value="none">No Auth</option>
                      <option value="bearer">Bearer Token</option>
                      <option value="basic">Basic Auth</option>
                      <option value="api-header">API Key Header</option>
                      <option value="api-query">API Key Query</option>
                      <option value="cookie">Cookie/session</option>
                    </select>
                  </label>
                  {authMode !== "none" && (
                    <label>
                      Nom / credential
                      <div className="auth-fields">
                        <input
                          value={authName}
                          onChange={(event) => setAuthName(event.target.value)}
                        />
                        <input
                          type="password"
                          value={authValue}
                          onChange={(event) => setAuthValue(event.target.value)}
                          placeholder="Non persisté"
                        />
                      </div>
                    </label>
                  )}
                </div>
                <div className="api-editors">
                  <label>
                    Headers JSON
                    <textarea
                      value={headers}
                      onChange={(event) => setHeaders(event.target.value)}
                    />
                  </label>
                  <label>
                    Body
                    <textarea
                      value={body}
                      onChange={(event) => setBody(event.target.value)}
                      placeholder="JSON, form ou texte"
                    />
                  </label>
                </div>
                <div className="api-actions">
                  <button onClick={() => void copyCurl()}>
                    <Copy size={13} />
                    Copier cURL
                  </button>
                  <button onClick={() => void saveRequest()}>
                    <Save size={13} />
                    Sauvegarder
                  </button>
                  <button onClick={() => void createContract()}>
                    <Braces size={13} />
                    Créer un test
                  </button>
                  <select
                    value={clientLanguage}
                    onChange={(event) => setClientLanguage(event.target.value)}
                  >
                    <option value="fetch">JavaScript fetch</option>
                    <option value="python">Python requests</option>
                    <option value="php">PHP</option>
                    <option value="go">Go</option>
                    <option value="rust">Rust reqwest</option>
                    <option value="dart">Dart http</option>
                  </select>
                  <button
                    onClick={() =>
                      void navigator.clipboard.writeText(clientCode)
                    }
                  >
                    <Copy size={13} />
                    Client
                  </button>
                </div>
              </div>
              {status && <p className="feature-action-status">{status}</p>}
              {response && (
                <div className="api-response">
                  <header>
                    <b
                      className={"status-" + Math.floor(response.status / 100)}
                    >
                      {response.status}
                    </b>
                    <span>{response.duration_ms} ms</span>
                    <span>{response.size} octets</span>
                    {response.truncated && <em>tronquée</em>}
                  </header>
                  <nav>
                    {(["pretty", "raw", "headers", "cookies"] as const).map(
                      (tab) => (
                        <button
                          className={responseTab === tab ? "active" : ""}
                          onClick={() => setResponseTab(tab)}
                          key={tab}
                        >
                          {tab}
                        </button>
                      ),
                    )}
                  </nav>
                  <pre>
                    {responseTab === "pretty"
                      ? prettyBody
                      : responseTab === "raw"
                        ? response.body
                        : JSON.stringify(
                            responseTab === "headers"
                              ? response.headers
                              : response.cookies,
                            null,
                            2,
                          )}
                  </pre>
                </div>
              )}
              <details className="api-client-preview">
                <summary>Aperçu du client généré</summary>
                <pre>{clientCode}</pre>
              </details>
              <div className="api-assets">
                <section>
                  <h3>
                    Requêtes sauvegardées{" "}
                    <button onClick={() => void createCollection()}>
                      Nouvelle collection
                    </button>
                  </h3>
                  {saved.map((request) => (
                    <button
                      key={request.id}
                      onClick={() => {
                        setMethod(request.request.method);
                        setUrl(request.request.url);
                        setHeaders(
                          JSON.stringify(request.request.headers, null, 2),
                        );
                        setBody(request.request.body ?? "");
                      }}
                    >
                      {request.name}
                    </button>
                  ))}
                </section>
                <section>
                  <h3>Collections</h3>
                  {collections.map((collection) => (
                    <button
                      key={collection.id}
                      onClick={() => void runCollection(collection)}
                    >
                      <Play size={12} />
                      {collection.name} · {collection.request_ids.length}
                      {Object.keys(collection.variable_extractions ?? {}).length >
                        0 && " · variables"}
                    </button>
                  ))}
                </section>
                <section>
                  <h3>Tests de contrat</h3>
                  {contracts.map((contract) => (
                    <button
                      key={contract.id}
                      onClick={() => void runContract(contract)}
                    >
                      <Play size={12} />
                      {contract.name} · {contract.assertions.length} assertion(s)
                    </button>
                  ))}
                </section>
                <section>
                  <h3>Historique</h3>
                  {history.slice(0, 20).map((entry) => (
                    <button key={entry.id}>
                      <b>
                        {entry.status ?? "—"} · {entry.method}
                      </b>
                      <span>{entry.url}</span>
                      <small>
                        {new Date(entry.created_at * 1000).toLocaleString(
                          "fr-FR",
                        )}
                      </small>
                    </button>
                  ))}
                </section>
              </div>
            </>
          ) : (
            <div className="empty-card">
              <Braces size={38} />
              <h2>Aucun endpoint</h2>
              <p>
                Analyse le projet pour alimenter automatiquement l’API Explorer.
              </p>
            </div>
          )}
        </main>
      </div>
    </section>
  );
}

function groupEndpoints(endpoints: ApiEndpoint[], mode: GroupMode) {
  const groups = new Map<string, ApiEndpoint[]>();
  for (const endpoint of endpoints) {
    const key =
      mode === "framework"
        ? endpoint.framework
        : mode === "feature"
          ? (endpoint.feature_ids[0] ?? "Sans Feature")
          : (endpoint.tags[0] ?? "root");
    groups.set(key, [...(groups.get(key) ?? []), endpoint]);
  }
  return groups;
}
function exampleFromSchema(schema: unknown): unknown {
  if (!schema || typeof schema !== "object") return {};
  const value = schema as {
    type?: string;
    properties?: Record<string, unknown>;
    example?: unknown;
  };
  if (value.example !== undefined) return value.example;
  if (value.type === "object" || value.properties)
    return Object.fromEntries(
      Object.entries(value.properties ?? {}).map(([name, child]) => [
        name,
        exampleFromSchema(child),
      ]),
    );
  if (value.type === "array") return [];
  if (value.type === "boolean") return false;
  if (value.type === "integer" || value.type === "number") return 0;
  return "";
}
function authHeaders(mode: string, name: string, value: string) {
  if (!value) return {};
  if (mode === "bearer") return { Authorization: "Bearer " + value };
  if (mode === "basic") return { Authorization: "Basic " + btoa(value) };
  if (mode === "api-header") return { [name || "X-API-Key"]: value };
  if (mode === "cookie") return { Cookie: value };
  return {};
}
function currentRequestSafe(
  method: string,
  url: string,
  headers: string,
  body: string,
): ApiRequest {
  let values: Record<string, string> = {};
  try {
    values = JSON.parse(headers);
  } catch {
    values = {};
  }
  return {
    method,
    url,
    headers: values,
    body: body || undefined,
    environment: "local",
    confirm_destructive: false,
  };
}
function clientSnippet(language: string, request: ApiRequest) {
  const headers = JSON.stringify(request.headers);
  const body = request.body ? JSON.stringify(request.body) : "undefined";
  if (language === "python")
    return (
      "import requests\nresponse = requests.request(" +
      JSON.stringify(request.method) +
      ", " +
      JSON.stringify(request.url) +
      ", headers=" +
      headers +
      ", data=" +
      body +
      ")\nprint(response.status_code, response.text)"
    );
  if (language === "php")
    return (
      "$response = (new \\\\GuzzleHttp\\\\Client())->request(" +
      JSON.stringify(request.method) +
      ", " +
      JSON.stringify(request.url) +
      ', [\"headers\" => ' +
      headers +
      "]);"
    );
  if (language === "go")
    return (
      "req, _ := http.NewRequest(" +
      JSON.stringify(request.method) +
      ", " +
      JSON.stringify(request.url) +
      ", nil)\nresp, err := http.DefaultClient.Do(req)"
    );
  if (language === "rust")
    return (
      "let response = reqwest::Client::new().request(reqwest::Method::" +
      request.method +
      ", " +
      JSON.stringify(request.url) +
      ").send().await?;"
    );
  if (language === "dart")
    return (
      "final response = await http." +
      request.method.toLowerCase() +
      "(Uri.parse(" +
      JSON.stringify(request.url) +
      "));"
    );
  return (
    "const response = await fetch(" +
    JSON.stringify(request.url) +
    ", {method: " +
    JSON.stringify(request.method) +
    ", headers: " +
    headers +
    ", body: " +
    body +
    "});\nconsole.log(response.status, await response.text());"
  );
}
function downloadJson(name: string, value: unknown) {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }),
  );
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
}
