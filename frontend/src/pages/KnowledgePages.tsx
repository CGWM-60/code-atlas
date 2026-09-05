import Markdown from "react-markdown";
import { useEffect, useState } from "react";
import { RefreshCw, Plus, Trash2, FolderOpen, Sparkles, BookOpen, LoaderCircle } from "lucide-react";
import type { McpStatus, ProjectSummary, MyCodeItem, Finding, ProjectDocumentation } from "../types/contracts";
import { api, errorMessage } from "../services/api";
export function McpPage() {
  const [value, setValue] = useState<McpStatus | null>(null);
  const [error, setError] = useState("");
  async function refresh() {
    try {
      const data = await api<McpStatus>("/api/mcp/status");
      setValue(data);
      setError("");
    } catch (reason) {
      setError(
        reason instanceof Error
          ? reason.message
          : "Diagnostic MCP indisponible",
      );
    }
  }
  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(timer);
  }, []);
  return (
    <section className="page-view mcp-page">
      <div className="page-heading">
        <div>
          <span>Model Context Protocol · diagnostic toutes les 5 secondes</span>
          <h1>État du serveur MCP</h1>
          <p>
            Le serveur MCP expose les connaissances persistées de Code Atlas aux
            clients compatibles, en lecture seule.
          </p>
        </div>
        <button onClick={() => void refresh()}>
          <RefreshCw size={14} />
          Vérifier maintenant
        </button>
      </div>
      {error && (
        <div className="documentation-error">
          <b>Erreur MCP</b>
          <span>{error}</span>
        </div>
      )}
      {value && (
        <>
          <div className="mcp-status-grid">
            <b className={value.status === "operational" ? "ok" : ""}>
              Serveur{" "}
              <span>
                {value.status === "operational" ? "Opérationnel" : value.status}
              </span>
            </b>
            <b>
              Transport <span>{value.transport}</span>
            </b>
            <b>
              Protocole <span>{value.protocol_version}</span>
            </b>
            <b>
              Base <span>{value.database}</span>
            </b>
            <b>
              Projets <span>{value.registered_projects}</span>
            </b>
            <b>
              Outils <span>{value.tool_count}</span>
            </b>
          </div>
          <article className="mcp-guide">
            <h2>Comment l’utiliser</h2>
            <p>
              Lance Code Atlas comme serveur MCP stdio avec la même base SQLite
              que l’application :
            </p>
            <pre>{value.command}</pre>
            <p>
              Dans ton client MCP, configure cette commande comme serveur local.
              Le client envoie ensuite <code>initialize</code>, puis{" "}
              <code>tools/list</code> et <code>tools/call</code>. Aucun outil
              MCP n’accepte un chemin arbitraire : il faut utiliser un
              identifiant de projet enregistré.
            </p>
            <h3>Exemple de configuration</h3>
            <pre>
              {JSON.stringify(
                {
                  mcpServers: {
                    "code-atlas": {
                      command: "/chemin/vers/code-atlas",
                      args: ["mcp", "--db", "/chemin/vers/code-atlas.sqlite"],
                    },
                  },
                },
                null,
                2,
              )}
            </pre>
            <h3>Contrôle en temps réel</h3>
            <p>
              Dernière vérification :{" "}
              {new Date(value.checked_at * 1000).toLocaleString("fr-FR")}. Ce
              contrôle interroge le même registre SQLite et le même catalogue
              d’outils que le serveur stdio.
            </p>
          </article>
          <article className="mcp-tools">
            <h2>Outils disponibles</h2>
            <div>
              {value.tools.map((tool) => (
                <code key={tool}>{tool}</code>
              ))}
            </div>
          </article>
        </>
      )}
    </section>
  );
}
export function ProjectsPage({
  projects,
  onOpen,
  onAnalyze,
  onDelete,
  onAdd,
}: {
  projects: ProjectSummary[];
  onOpen: (id: string) => void;
  onAnalyze: (root: string) => void;
  onDelete: (id: string) => void;
  onAdd: () => void;
}) {
  return (
    <section className="page-view">
      <div className="page-heading">
        <div>
          <span>Project Registry</span>
          <h1>Your Projects</h1>
          <p>
            Open the latest saved graph immediately or run an incremental
            analysis.
          </p>
        </div>
        <button className="primary page-cta" onClick={onAdd}>
          <Plus size={15} />
          Analyze another project
        </button>
      </div>
      <div className="project-grid">
        {projects.map((project) => (
          <article className="project-card" key={project.id}>
            <div>
              <span>{project.analysis_status}</span>
              <h2>{project.name}</h2>
              <code>{project.root}</code>
            </div>
            <p>{project.languages?.join(" · ") || "Languages unavailable"}</p>
            <div className="project-metrics">
              <b>
                {project.node_count.toLocaleString()}
                <small>Nodes</small>
              </b>
              <b>
                {project.edge_count.toLocaleString()}
                <small>Relations</small>
              </b>
              <b>
                {project.file_count.toLocaleString()}
                <small>Files</small>
              </b>
              <b>
                {project.unresolved_count.toLocaleString()}
                <small>Unresolved</small>
              </b>
            </div>
            <time>
              Last analysis:{" "}
              {new Date(project.last_analyzed_at * 1000).toLocaleString()}
            </time>
            <div className="card-actions">
              <button onClick={() => onOpen(project.id)}>Open</button>
              <button onClick={() => onAnalyze(project.root)}>
                <RefreshCw size={13} />
                Re-analyze
              </button>
              <button className="danger" onClick={() => onDelete(project.id)}>
                <Trash2 size={13} />
              </button>
            </div>
          </article>
        ))}
        {projects.length === 0 && (
          <div className="empty-card">
            <FolderOpen size={35} />
            <h2>No saved projects yet</h2>
            <button onClick={onAdd}>Analyze your first project</button>
          </div>
        )}
      </div>
    </section>
  );
}
export function MyCodePage({
  items,
  kind,
  onKind,
  onInspect,
  onExplain,
  onImpact,
  onAdd,
}: {
  items: MyCodeItem[];
  kind: string;
  onKind: (value: string) => void;
  onInspect: (id: string) => void;
  onExplain: (id: string) => void;
  onImpact: (id: string) => void;
  onAdd: (id: string) => void;
}) {
  const filters = [
    ["functions", "Fonctions"],
    ["methods", "Méthodes"],
    ["classes", "Classes"],
    ["service", "Services"],
    ["component", "Composants"],
    ["provider", "Providers"],
    ["route", "Routes"],
    ["candidates", "Candidats"],
    ["recently_modified", "Modifiés récemment"],
    ["untested", "Sans test"],
    ["complex", "Complexes"],
    ["high_coupling", "Fort couplage"],
    ["all", "Tout"],
  ];
  return (
    <section className="page-view">
      <div className="page-heading">
        <div>
          <span>Code propriétaire</span>
          <h1>Mon code</h1>
          <p>
            Uniquement le code du projet et du workspace. Les dépendances et
            sources générées sont exclues.
          </p>
        </div>
      </div>
      <div className="my-code-filters">
        {filters.map(([value, label]) => (
          <button
            className={kind === value ? "active" : ""}
            key={value}
            onClick={() => onKind(value)}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="my-code-list">
        {items.map((item) => (
          <article key={item.node.id}>
            <div>
              <span>
                {item.node.kind} · {item.node.language ?? "Inconnu"}
              </span>
              <h2>{item.node.name}</h2>
              <code>
                {item.node.path}:{item.node.start_line ?? "—"}–
                {item.node.end_line ?? "—"}
              </code>
            </div>
            <div className="my-code-metrics">
              <b>
                {item.incoming_usages}
                <small>Appelants</small>
              </b>
              <b>
                {item.outgoing_dependencies}
                <small>Dépendances</small>
              </b>
              <b>
                {item.complexity ?? "—"}
                <small>Complexité</small>
              </b>
              <b>
                {item.tests}
                <small>Tests</small>
              </b>
              <b>
                {item.reuse_score ?? "—"}
                <small>Réemploi</small>
              </b>
              <b>
                {item.knowledge_value ?? "—"}
                <small>Connaissance</small>
              </b>
            </div>
            <div className="card-actions">
              <button onClick={() => onInspect(item.node.id)}>
                Voir le code
              </button>
              <button onClick={() => onExplain(item.node.id)}>
                <Sparkles size={13} />
                Expliquer
              </button>
              <button onClick={() => onImpact(item.node.id)}>Impact</button>
              {item.library_status === "candidate" && (
                <button onClick={() => onAdd(item.node.id)}>
                  <BookOpen size={13} />
                  Ajouter à la bibliothèque
                </button>
              )}
              <span className={`library-status ${item.library_status}`}>
                {item.library_status.replace("_", " ")}
              </span>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
export function FindingsPage({
  title,
  description,
  items,
  onInspect,
  onStatus,
  onExplain,
  onReview,
}: {
  title: string;
  description: string;
  items: Finding[];
  onInspect: (finding: Finding) => void;
  onStatus: (finding: Finding, status: Finding["status"]) => void;
  onExplain: (finding: Finding) => void;
  onReview?: () => void;
}) {
  const [severity, setSeverity] = useState("");
  const [status, setStatus] = useState("");
  const [path, setPath] = useState("");
  const visible = items.filter(item => (!severity || item.severity === severity) && (!status || item.status === status) && (!path || (item.path ?? "").toLowerCase().includes(path.toLowerCase())));
  const counts = items.reduce<Record<string, number>>(
    (result, item) => ({
      ...result,
      [item.severity]: (result[item.severity] ?? 0) + 1,
    }),
    {},
  );
  const severityLabels: Record<string, string> = {
    critical: "critique",
    high: "élevé",
    medium: "moyen",
    low: "faible",
    info: "info",
  };
  return (
    <section className="page-view">
      <div className="page-heading">
        <div>
          <span>Analyse déterministe avant IA</span>
          <h1>{title}</h1>
          <p>{description}</p>
        </div>
        {onReview && (
          <button className="primary page-cta" onClick={onReview}>
            <Sparkles size={14} />
            Lancer la revue IA
          </button>
        )}
      </div>
      <div className="finding-summary">
        {["critical", "high", "medium", "low", "info"].map((severity) => (
          <b className={severity} key={severity}>
            {counts[severity] ?? 0}
            <small>{severityLabels[severity]}</small>
          </b>
        ))}
      </div>
      <div className="finding-filters">
        <label>Sévérité<select value={severity} onChange={e => setSeverity(e.target.value)}><option value="">Toutes</option>{["critical", "high", "medium", "low", "info"].map(v => <option key={v} value={v}>{severityLabels[v]}</option>)}</select></label>
        <label>Statut<select value={status} onChange={e => setStatus(e.target.value)}><option value="">Tous</option>{["open", "accepted", "ignored", "false_positive", "fixed"].map(v => <option key={v} value={v}>{v}</option>)}</select></label>
        <label>Fichier<input value={path} onChange={e => setPath(e.target.value)} placeholder="Filtrer par chemin" /></label>
      </div>
      {!visible.length && <div className="empty-card"><h2>Aucun constat dans ce périmètre</h2><p>Adaptez les filtres ou relancez l’analyse. L’absence de constat ne prouve pas l’absence de risque.</p></div>}
      <div className="finding-list">
        {visible.map((finding) => (
          <article
            key={finding.id}
            className={finding.status === "ignored" ? "muted" : ""}
          >
            <i className={finding.severity} />
            <div>
              <span>
                {finding.category} · {finding.detector} · confiance{" "}
                {Math.round(finding.confidence * 100)}%
              </span>
              <h2>{finding.title}</h2>
              <p>{finding.description}</p>
              <code>
                {finding.primary_span?.path ?? finding.path ?? "projet"}:
                {finding.primary_span?.start_line ?? finding.start_line ?? "—"}:
                {finding.primary_span?.start_column ?? "—"}–
                {finding.primary_span?.end_line ?? finding.end_line ?? "—"}:
                {finding.primary_span?.end_column ?? "—"}
              </code>
              {finding.evidence.length > 0 && (
                <ul>
                  {finding.evidence.map((evidence, index) => (
                    <li key={`${evidence}-${index}`}>{evidence}</li>
                  ))}
                </ul>
              )}
              {finding.ai_analysis && (
                <details>
                  <summary>Analyse IA enregistrée</summary>
                  <pre>{finding.ai_analysis}</pre>
                </details>
              )}
            </div>
            <div className="finding-actions">
              <em>{finding.status}</em>
              {(finding.primary_span || finding.node_ids[0]) && (
                <button onClick={() => onInspect(finding)}>Voir le code</button>
              )}
              <button onClick={() => onExplain(finding)}>
                <Sparkles size={12} />
                Expliquer avec l’IA
              </button>
              <button onClick={() => onStatus(finding, "accepted")}>
                Confirmer
              </button>
              <button onClick={() => onStatus(finding, "ignored")}>
                Ignorer
              </button>
              <button onClick={() => onStatus(finding, "false_positive")}>
                Faux positif
              </button>
              <button onClick={() => onStatus(finding, "fixed")}>
                Corrigé
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
export function DocsPage({
  documentation,
  projectId,
  onRefresh,
}: {
  documentation: ProjectDocumentation | null;
  projectId: string;
  onRefresh: () => void;
}) {
  if (!documentation)
    return (
      <section className="page-view">
        <div className="empty-card">
          <LoaderCircle className="spinner" size={28} />
          <h2>Construction de la documentation…</h2>
        </div>
      </section>
    );
  const sections: [string, string[]][] = [
    ["Architecture", documentation.architecture],
    ["Applications", documentation.applications],
    ["Points d’entrée", documentation.entry_points],
    ["Features", documentation.features],
    ["Domaines", documentation.domains],
    ["Flux principaux", documentation.main_flows],
    ["Modules", documentation.modules],
    ["API", documentation.apis],
    ["Modèle de données", documentation.data_model],
    ["Services externes", documentation.external_services],
    ["Workers", documentation.workers],
    ["Événements", documentation.events],
    ["Configuration", documentation.configuration],
    ["Sécurité", documentation.security_notes],
    ["Développement", documentation.development],
    ["Déploiement", documentation.deployment],
    ["Limites connues", documentation.known_limitations],
  ];
  return (
    <section className="page-view">
      <div className="page-heading">
        <div>
          <span>
            Connaissance projet ·{" "}
            {documentation.cache_hit ? "cache à jour" : "génération fraîche"}
          </span>
          <h1>Documentation de {documentation.project_name}</h1>
          <div className="rendered-markdown"><Markdown skipHtml>{documentation.overview}</Markdown></div>
        </div>
        <div className="docs-actions">
          <button onClick={onRefresh}>
            <RefreshCw size={14} />
            Actualiser
          </button>
          <a
            href={`/api/projects/${projectId}/docs/markdown`}
            target="_blank"
            rel="noreferrer"
          >
            Exporter en Markdown
          </a>
        </div>
      </div>
      <article className="docs-purpose">
        <h2>Objectif</h2>
        <div className="rendered-markdown"><Markdown skipHtml>{documentation.purpose}</Markdown></div>
      </article>
      <nav className="docs-toc" aria-label="Table des matières">{sections.map(([title], index) => <a key={title} href={`#doc-section-${index}`}>{title}</a>)}</nav>
      <div className="docs-grid">
        {sections.map(([title, items], index) => (
          <section key={title} id={`doc-section-${index}`}>
            <h2>{title}</h2>
            {items.length ? (
              <ul>
                {items.slice(0, 100).map((item, index) => (
                  <li key={`${item}-${index}`}><Markdown skipHtml>{item}</Markdown></li>
                ))}
              </ul>
            ) : (
              <p>Non détecté.</p>
            )}
          </section>
        ))}
      </div>
    </section>
  );
}
