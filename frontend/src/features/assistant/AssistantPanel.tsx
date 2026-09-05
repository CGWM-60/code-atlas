import Markdown from "react-markdown";
import { useEffect, useRef, useState } from "react";
import { api, errorMessage, projectApi } from "../../services/api";
import type { AssistantResponse, Conversation, Message, UiAction, UiContext } from "./types";

type Props = { projectId: string; projectName: string; context: UiContext; configuration?: { provider: string; api_key: string; model: string }; onAction: (action: UiAction) => Promise<void>; onClose: () => void };
export function AssistantPanel({ projectId, projectName, context, configuration, onAction, onClose }: Props) {
  const [width, setWidth] = useState(() => Number(localStorage.getItem("atlas-assistant-width")) || 380);
  useEffect(() => { document.documentElement.style.setProperty("--atlas-assistant-width", `${width}px`); localStorage.setItem("atlas-assistant-width", String(width)); }, [width]);
  function resize(clientX: number) { setWidth(Math.max(300, Math.min(window.innerWidth * .65, window.innerWidth - clientX))); }
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [conversation, setConversation] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [question, setQuestion] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [activity, setActivity] = useState("");
  const [neural, setNeural] = useState(false);
  const [availableTools, setAvailableTools] = useState<string[]>([]);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [automatic, setAutomatic] = useState(true);
  const generation = useRef(0);
  const selection = useRef(0);
  const end = useRef<HTMLDivElement>(null);
  const base = `${projectApi(projectId)}/conversations`;
  useEffect(() => {
    const current = ++generation.current;
    const controller = new AbortController();
    setMessages([]); setConversation(""); setError(""); setBusy(false);
    api<Conversation[]>(base, { signal: controller.signal }).then(async values => {
      if (generation.current !== current) return;
      setConversations(values);
      const saved = localStorage.getItem(`atlas-conversation:${projectId}`);
      if (values.some(value => value.id === saved)) {
        const history = await api<Message[]>(`${base}/${saved}`, { signal: controller.signal });
        if (generation.current === current) { setConversation(saved!); setMessages(history); }
      }
    }).catch(error => { if (!controller.signal.aborted) setError(errorMessage(error)); });
    return () => { controller.abort(); generation.current++; selection.current++; };
  }, [base, projectId]);
  useEffect(() => {
    const controller = new AbortController();
    api<{ tools: string[] }>("/api/assistant/tools", { signal: controller.signal }).then(v => setAvailableTools(v.tools)).catch(e => { if (!controller.signal.aborted) setError(errorMessage(e)); });
    const socket = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/api/events`);
    socket.onmessage = event => { try { const value = JSON.parse(event.data); if (value.type === "assistant_activity" && value.project_id === projectId && value.conversation_id === localStorage.getItem(`atlas-conversation:${projectId}`)) setActivity(value.stage); } catch { setActivity("Le flux d’activité a renvoyé un message illisible."); } };
    return () => { controller.abort(); socket.close(); };
  }, [projectId]);
  useEffect(() => { end.current?.scrollIntoView({ block: "nearest" }); }, [messages, busy]);
  async function choose(id: string) {
    if (busy) return;
    const selected = ++selection.current;
    setError(""); setMessages([]); setConversation(id); localStorage.setItem(`atlas-conversation:${projectId}`, id);
    if (!id) { setMessages([]); return; }
    try { const history = await api<Message[]>(`${base}/${id}`); if (selected === selection.current) setMessages(history); } catch (error) { if (selected === selection.current) setError(errorMessage(error)); }
  }
  async function ask(text = question) {
    if (!text.trim() || busy) return;
    const current = generation.current;
    setBusy(true); setError(""); setActivity("Recherche dans le projet…");
    try {
      let id = conversation;
      if (!id) { const created = await api<Conversation>(base, { method: "POST", body: JSON.stringify({ title: text.slice(0, 100) }) }); id = created.id; if (current !== generation.current) return; setConversation(id); localStorage.setItem(`atlas-conversation:${projectId}`, id); }
      const result = await api<AssistantResponse>(`${base}/${id}/messages`, { method: "POST", body: JSON.stringify({ question: text, context, configuration, embedding_configuration: neural && configuration ? { ...configuration, model: localStorage.getItem(`atlas-embedding-model:${configuration.provider}`) ?? "" } : undefined }) });
      if (current !== generation.current) return;
      setQuestion("");
      const [history, conversations] = await Promise.all([api<Message[]>(`${base}/${id}`), api<Conversation[]>(base)]);
      if (current !== generation.current) return;
      setMessages(history); setConversations(conversations);
      if (automatic && result.actions[0]) await onAction(result.actions[0]);
    } catch (error) { if (current === generation.current) setError(errorMessage(error)); }
    finally { if (current === generation.current) setBusy(false); }
  }
  async function rename() {
    try { await api(`${base}/${conversation}`, { method: "PATCH", body: JSON.stringify({ title }) }); setConversations(await api<Conversation[]>(base)); setRenaming(false); }
    catch (error) { setError(errorMessage(error)); }
  }
  async function remove() {
    try { await api(`${base}/${conversation}`, { method: "DELETE" }); await choose(""); setConversations(await api<Conversation[]>(base)); }
    catch (error) { setError(errorMessage(error)); }
  }
  const perform = (action: UiAction) => { void onAction(action).catch(error => setError(errorMessage(error))); };
  return <aside className="assistant-panel" aria-label="Assistant projet">
    <div className="assistant-resizer" role="separator" aria-label="Largeur de l’assistant" aria-orientation="vertical" aria-valuemin={300} aria-valuemax={Math.round(window.innerWidth * .65)} aria-valuenow={Math.round(width)} tabIndex={0} onKeyDown={e => { if (e.key === "ArrowLeft" || e.key === "ArrowRight") { e.preventDefault(); resize(window.innerWidth - width + (e.key === "ArrowLeft" ? -20 : 20)); } }} onPointerDown={e => { e.currentTarget.setPointerCapture(e.pointerId); }} onPointerMove={e => { if (e.currentTarget.hasPointerCapture(e.pointerId)) resize(e.clientX); }} onPointerUp={e => e.currentTarget.releasePointerCapture(e.pointerId)} />
    <div className="assistant-heading"><div><strong>Assistant Atlas</strong><small>{projectName} · {context.active_page}</small></div><button onClick={onClose} aria-label="Fermer l’assistant">×</button></div>
    <div className="assistant-conversations"><select aria-label="Conversation" value={conversation} onChange={e => void choose(e.target.value)} disabled={busy}><option value="">Nouvelle conversation</option>{conversations.map(c => <option key={c.id} value={c.id}>{c.title}</option>)}</select><button disabled={busy} onClick={() => void choose("")} aria-label="Nouvelle conversation">+</button>{conversation && <><button disabled={busy} onClick={() => { setTitle(conversations.find(c => c.id === conversation)?.title ?? ""); setRenaming(true); }}>Renommer</button><button disabled={busy} onClick={() => void remove()} aria-label="Supprimer la conversation">Supprimer</button></>}</div>
    {renaming && <form className="inline-form" onSubmit={e => { e.preventDefault(); void rename(); }}><input aria-label="Titre de conversation" value={title} onChange={e => setTitle(e.target.value)} maxLength={120} autoFocus /><button>Enregistrer</button><button type="button" onClick={() => setRenaming(false)}>Annuler</button></form>}
    <div className="assistant-messages" aria-live="polite">
      {!messages.length && <div className="assistant-empty"><h2>Explorez votre projet</h2><p>Posez une question. Atlas recherche les symboles, consulte les sources et ouvre les vues concernées.</p><button onClick={() => setQuestion("Montre-moi comment fonctionne l’authentification.")}>Comprendre l’authentification</button><button onClick={() => setQuestion("Quels tests manquent ?")}>Examiner les tests</button><small>{configuration ? `${configuration.provider} · ${configuration.model}` : "Mode local · recherche et analyses déterministes"}</small></div>}
      {messages.map(message => <article key={message.id} className={`chat-message ${message.role}`}><b>{message.role === "user" ? "Vous" : "Atlas"}</b><div className="assistant-answer"><Markdown skipHtml>{message.role === "user" ? message.content.text ?? "" : message.content.answer}</Markdown></div>{message.role === "assistant" && <>
        <small>{message.content.mode} · {message.content.context_tokens} tokens de source · conclusions à vérifier</small>
        <div className="entity-cards">{message.content.entities.slice(0, 6).map(node => <button key={node.id} onClick={() => perform({ type: "OPEN_NODE", node_id: node.id })}><strong>{node.name}</strong><small>{node.path} · {node.kind}</small></button>)}</div>
        <details><summary>Preuves · {message.content.citations.length}</summary>{message.content.citations.map(c => <button className="citation" key={c.node_id} onClick={() => perform({ type: "OPEN_SOURCE_RANGE", node_id: c.node_id, start_line: c.start_line, end_line: c.end_line, source_hash: c.source_hash })}><b>{c.symbol}</b><span>{c.path}:{c.start_line}–{c.end_line}</span><small>{c.tool}</small></button>)}</details>
        <details><summary>Activité · {message.content.tool_calls.length} opérations</summary>{message.content.tool_calls.map((tool, index) => <p key={index}><b>{tool.tool}</b> — {tool.summary}</p>)}</details>
        <div className="assistant-followups">{message.content.actions.map((action, index) => <button key={index} onClick={() => perform(action)}>{actionLabel(action)}</button>)}{message.content.suggested_followups.map(text => <button key={text} disabled={busy} onClick={() => void ask(text)}>{text}</button>)}</div>
      </>}</article>)}
      {busy && <p role="status">{activity || "Recherche dans le projet…"}</p>}<div ref={end} />
    </div>
    {error && <div className="intelligence-error" role="alert"><strong>La demande n’a pas abouti</strong><p>{error}</p><button disabled={busy} onClick={() => void ask()}>Réessayer</button></div>}
    <details className="assistant-capabilities"><summary>{availableTools.length} outils internes disponibles</summary><p>{availableTools.join(" · ")}</p></details>
    <form className="assistant-composer" onSubmit={e => { e.preventDefault(); void ask(); }}><label htmlFor="atlas-question">Votre question</label><textarea id="atlas-question" value={question} maxLength={8000} onChange={e => setQuestion(e.target.value)} placeholder="Retrouver du code, expliquer un flux, examiner les tests…" onKeyDown={e => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); void ask(); } }} /><label><input type="checkbox" checked={neural} disabled={!configuration} onChange={e => setNeural(e.target.checked)} /> Utiliser les embeddings neuronaux configurés dans Recherche</label><div><label><input type="checkbox" checked={automatic} onChange={e => setAutomatic(e.target.checked)} /> Ouvrir les vues automatiquement</label><button disabled={busy || !question.trim()}>{busy ? "Analyse…" : "Envoyer"}</button></div></form>
  </aside>;
}

function actionLabel(action: UiAction): string {
  const labels: Record<UiAction["type"], string> = {
    NAVIGATE_PAGE: "Ouvrir la page", OPEN_NODE: "Ouvrir le symbole", FOCUS_NODE: "Centrer le graphe", SELECT_NODE: "Sélectionner le symbole",
    OPEN_SOURCE_RANGE: "Ouvrir les lignes citées", HIGHLIGHT_SOURCE_RANGE: "Surligner les lignes", OPEN_FILE: "Ouvrir le fichier",
    OPEN_FEATURE: "Examiner la Feature", OPEN_FEATURE_SOURCE: "Voir la source de la Feature", OPEN_FEATURE_GRAPH: "Voir le graphe de la Feature",
    OPEN_FINDING: "Examiner le constat", OPEN_SECURITY: "Examiner la sécurité", OPEN_QUALITY: "Examiner la qualité", OPEN_FLOW: "Afficher le flux", TRACE_FLOW: "Tracer le flux",
    OPEN_IMPACT: "Analyser l’impact", OPEN_API: "Ouvrir l’explorateur API", OPEN_API_ENDPOINT: "Examiner l’endpoint", OPEN_LIBRARY_ENTRY: "Ouvrir dans la bibliothèque",
    OPEN_DOCUMENTATION: "Lire la documentation", FOCUS_GRAPH: "Centrer le graphe", FIT_GRAPH: "Ajuster le graphe", FILTER_GRAPH: "Filtrer le graphe", SHOW_RELATED_NODES: "Voir les relations",
    SHOW_DIFF: "Ouvrir la revue Git", SHOW_TEST_PLAN: "Examiner les tests", SHOW_ESTIMATE: "Ouvrir l’estimation",
  };
  return labels[action.type];
}
