import { useEffect, useState } from "react";
import { api, errorMessage, projectApi } from "../../services/api";
import type { Candidate } from "../../pages/HybridSearchPage";
type Configuration = { provider: string; api_key: string };
type Stats = { version: string; total: number; updated: number; reused: number; remaining: number; skipped: string[] };
export function NeuralSearchControls({ projectId, configuration, query, onResults }: { projectId: string; configuration?: Configuration; query: string; onResults: (items: Candidate[]) => void }) {
 const provider = configuration?.provider ?? "openai";
 const [model, setModel] = useState(""); const [busy, setBusy] = useState(false); const [error, setError] = useState(""); const [stats, setStats] = useState<Stats | null>(null);
 useEffect(() => { setModel(localStorage.getItem(`atlas-embedding-model:${provider}`) ?? ({ openai: "text-embedding-3-small", mistral: "codestral-embed", openrouter: "openai/text-embedding-3-small" }[provider] ?? "")); setStats(null); }, [provider]);
 async function run(index: boolean) {
  if (!configuration || !model.trim()) return;
  setBusy(true); setError(""); localStorage.setItem(`atlas-embedding-model:${provider}`, model.trim());
  try { const input = { configuration: { ...configuration, model: model.trim() }, query };
   if (index) setStats(await api<Stats>(`${projectApi(projectId)}/intelligence/embeddings`, { method: "POST", body: JSON.stringify(input) }));
   else onResults((await api<{ items: Candidate[] }>(`${projectApi(projectId)}/intelligence/semantic-search`, { method: "POST", body: JSON.stringify(input) })).items);
  } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
 }
 return <details className="intelligence-card"><summary>Embeddings neuronaux optionnels</summary><p>Vectoriser envoie les unités de code expurgées au provider {provider}. Les vecteurs restent dans SQLite. Seules les unités nouvelles ou modifiées sont recalculées ; aucun envoi automatique au scan.</p>{!configuration && <p>Renseignez une clé dans les paramètres IA pour activer ces actions.</p>}<label className="search-field">Modèle d’embedding<input aria-label="Modèle d’embedding" value={model} onChange={e => setModel(e.target.value)} disabled={busy} /></label><div className="inline-form"><button disabled={busy || !configuration || !model.trim()} onClick={() => void run(true)}>{busy ? "Traitement…" : stats?.remaining ? `Continuer (${stats.remaining} unités)` : "Vectoriser les unités modifiées"}</button><button disabled={busy || !configuration || !model.trim() || !query.trim()} onClick={() => void run(false)}>Rechercher avec les embeddings</button></div>{busy && <p role="status">Requête au provider en cours…</p>}{stats && <p>{stats.updated} unités vectorisées · {stats.reused} réutilisées · {stats.remaining} restantes · {stats.version}</p>}{!!stats?.skipped.length && <details><summary>{stats.skipped.length} unités trop grandes</summary>{stats.skipped.map((s, i) => <p key={i}>{s}</p>)}</details>}{error && <div role="alert" className="intelligence-error"><strong>Échec du provider</strong><p>{error}</p><p>Les lots déjà enregistrés seront réutilisés lors du prochain essai.</p></div>}</details>;
}
