import { useEffect, useMemo, useState } from "react";
import Editor from "@monaco-editor/react";
import { FileCode2, Search } from "lucide-react";

export type FeatureSourceSnippet = {
  project_id: string;
  node_id: string;
  name: string;
  language?: string;
  architecture_role: string;
  path: string;
  start_line: number;
  end_line: number;
  source: string;
  source_hash: string;
  membership_reason: string;
};
type FeatureSourceFile = {
  path: string;
  language?: string;
  architecture_role: string;
  source_hash: string;
  fragments: FeatureSourceSnippet[];
};
type SearchHit = {
  path: string;
  node_id: string;
  symbol: string;
  line: number;
  preview: string;
};

export function FeatureSourceExplorer({
  projectId,
  featureId,
  initial,
}: {
  projectId: string;
  featureId: string;
  initial: FeatureSourceSnippet[];
}) {
  const [files, setFiles] = useState<FeatureSourceFile[]>(() =>
    groupInitial(initial),
  );
  const [selected, setSelected] = useState<FeatureSourceSnippet | null>(
    initial[0] ?? null,
  );
  const [entireSource, setEntireSource] = useState<string | null>(null);
  const [mode, setMode] = useState<"fragments" | "file">("fragments");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [status, setStatus] = useState("");
  useEffect(() => {
    void load();
  }, [projectId, featureId]);
  async function load() {
    const response = await fetch(
      "/api/projects/" +
        projectId +
        "/features/" +
        encodeURIComponent(featureId) +
        "/source",
    );
    if (response.ok) {
      const value = await response.json();
      setFiles(value.files);
      const first = value.files[0]?.fragments[0] as
        | FeatureSourceSnippet
        | undefined;
      setSelected((current) => current ?? first ?? null);
    }
  }
  async function entireFile() {
    if (!selected) return;
    setStatus("Chargement du fichier complet…");
    const response = await fetch(
      "/api/projects/" +
        projectId +
        "/features/" +
        encodeURIComponent(featureId) +
        "/source/file?path=" +
        encodeURIComponent(selected.path),
    );
    const value = await response.json();
    if (response.ok) {
      setEntireSource(value.source);
      setMode("file");
      setStatus("Fichier complet chargé explicitement");
    } else setStatus(value.error ?? "Fichier indisponible");
  }
  async function search() {
    if (!query.trim()) {
      setHits([]);
      return;
    }
    const response = await fetch(
      "/api/projects/" +
        projectId +
        "/features/" +
        encodeURIComponent(featureId) +
        "/source?q=" +
        encodeURIComponent(query),
    );
    if (response.ok) setHits((await response.json()).items);
  }
  function selectFragment(fragment: FeatureSourceSnippet) {
    setSelected(fragment);
    setMode("fragments");
    setEntireSource(null);
  }
  function openHit(hit: SearchHit) {
    const fragment = files
      .flatMap((file) => file.fragments)
      .find(
        (item) =>
          item.node_id === hit.node_id &&
          hit.line >= item.start_line &&
          hit.line <= item.end_line,
      );
    if (fragment) selectFragment(fragment);
  }
  const grouped = useMemo(() => {
    const value = new Map<string, FeatureSourceFile[]>();
    for (const file of files)
      value.set(file.architecture_role, [
        ...(value.get(file.architecture_role) ?? []),
        file,
      ]);
    return value;
  }, [files]);
  const source =
    mode === "file" ? (entireSource ?? "") : (selected?.source ?? "");
  return (
    <div className="feature-source-explorer">
      <aside>
        <div className="feature-source-search">
          <Search size={13} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && void search()}
            placeholder="Rechercher dans la Feature"
          />
          <button onClick={() => void search()}>Chercher</button>
        </div>
        {hits.length > 0 && (
          <div className="feature-source-hits">
            {hits.map((hit) => (
              <button
                key={hit.node_id + ":" + hit.line}
                onClick={() => openHit(hit)}
              >
                <b>
                  {hit.symbol} · L{hit.line}
                </b>
                <span>{hit.path}</span>
                <code>{hit.preview}</code>
              </button>
            ))}
          </div>
        )}
        {[...grouped].map(([role, roleFiles]) => (
          <section key={role}>
            <h3>
              {role}
              <span>{roleFiles.length}</span>
            </h3>
            {roleFiles.map((file) => (
              <details
                open={file.fragments.some(
                  (fragment) => fragment.node_id === selected?.node_id,
                )}
                key={file.path}
              >
                <summary>
                  <FileCode2 size={12} />
                  {file.path}
                  <small>{file.fragments.length}</small>
                </summary>
                {file.fragments.map((fragment) => (
                  <button
                    className={
                      fragment.node_id === selected?.node_id &&
                      mode === "fragments"
                        ? "active"
                        : ""
                    }
                    key={fragment.node_id + ":" + fragment.start_line}
                    onClick={() => selectFragment(fragment)}
                  >
                    <b>{fragment.name}</b>
                    <span>
                      L{fragment.start_line}–{fragment.end_line}
                    </span>
                  </button>
                ))}
              </details>
            ))}
          </section>
        ))}
      </aside>
      <main>
        <header>
          <div>
            <b>{selected?.path ?? "Aucun fichier"}</b>
            <span>
              {mode === "file"
                ? "Fichier complet"
                : "Fragments appartenant à la Feature"}
            </span>
          </div>
          <nav>
            <button
              className={mode === "fragments" ? "active" : ""}
              onClick={() => setMode("fragments")}
            >
              Fragments Feature
            </button>
            <button
              className={mode === "file" ? "active" : ""}
              disabled={!selected}
              onClick={() => void entireFile()}
            >
              Fichier complet
            </button>
          </nav>
        </header>
        <div className="feature-source-editor">
          <Editor
            key={(selected?.node_id ?? "none") + mode}
            height="100%"
            theme="vs-dark"
            language={(selected?.language ?? "text").toLowerCase()}
            value={source}
            options={{
              readOnly: true,
              minimap: { enabled: false },
              fontSize: 12,
              scrollBeyondLastLine: false,
              lineNumbers:
                mode === "file"
                  ? "on"
                  : (value) =>
                      String((selected?.start_line ?? 1) + Number(value) - 1),
            }}
          />
        </div>
        {selected && (
          <footer>
            <span>
              Projet <b>{selected.project_id}</b>
            </span>
            <span>
              Langage <b>{selected.language ?? "inconnu"}</b>
            </span>
            <span>
              Lignes{" "}
              <b>
                {mode === "file"
                  ? "fichier complet"
                  : selected.start_line + "–" + selected.end_line}
              </b>
            </span>
            <span>
              Hash <code>{selected.source_hash.slice(0, 16)}</code>
            </span>
            <p>{selected.membership_reason}</p>
          </footer>
        )}
        {status && <p className="feature-action-status">{status}</p>}
      </main>
    </div>
  );
}

function groupInitial(snippets: FeatureSourceSnippet[]) {
  const files = new Map<string, FeatureSourceFile>();
  for (const fragment of snippets) {
    const current = files.get(fragment.path) ?? {
      path: fragment.path,
      language: fragment.language,
      architecture_role: fragment.architecture_role ?? "Other",
      source_hash: fragment.source_hash ?? "",
      fragments: [],
    };
    current.fragments.push(fragment);
    files.set(fragment.path, current);
  }
  return [...files.values()];
}
