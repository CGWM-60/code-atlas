import { KnowledgeDocument } from "./components/KnowledgeDocument";
import { ImpactView, type ImpactResult } from "./components/ImpactView";
import { DependenciesPage } from "./pages/DependenciesPage";
import type { Details, DirectoryListing, AiProvider, AiSettings, Page, ProjectSummary, ProgressState, ScoreFactor, RealUsage, AiFunctionParameter, AiFunctionDocumentation, LibraryEntry, LibraryCandidate, MyCodeItem, Finding, ProjectDocumentation, FeatureMembership, Feature, FeatureSpec, FeatureSnippet, FeatureLibraryEntry, ProjectDashboard, GeneratedFile, McpStatus } from "./types/contracts";
export type { Details, DirectoryListing, AiProvider, AiSettings, Page, ProjectSummary, ProgressState, ScoreFactor, RealUsage, AiFunctionParameter, AiFunctionDocumentation, LibraryEntry, LibraryCandidate, MyCodeItem, Finding, ProjectDocumentation, FeatureMembership, Feature, FeatureSpec, FeatureSnippet, FeatureLibraryEntry, ProjectDashboard, GeneratedFile, McpStatus } from "./types/contracts";
import { McpPage, ProjectsPage, MyCodePage, FindingsPage, DocsPage } from "./pages/KnowledgePages";
export { MyCodePage, FindingsPage, DocsPage } from "./pages/KnowledgePages";
import { HybridSearchPage } from "./pages/HybridSearchPage";
import { AssistantPanel } from "./features/assistant/AssistantPanel";
import { CommandPalette } from "./features/assistant/CommandPalette";
import type { UiAction } from "./features/assistant/types";
import { dispatchUiAction } from "./state/uiActions";
import { api, projectApi } from "./services/api";
import { TestsPage, EstimatePage, GitPage } from "./pages/IntelligencePages";
import { useEffect, useMemo, useState } from "react";
import Editor from "@monaco-editor/react";
import {
  Background,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  type Edge,
  type Node,
  type NodeProps,
  type ReactFlowInstance,
} from "@xyflow/react";
import {
  Activity,
  ArrowUp,
  BookOpen,
  Box,
  ChevronRight,
  Crosshair,
  Eye,
  EyeOff,
  Folder,
  FolderOpen,
  GitBranch,
  Layers3,
  LoaderCircle,
  Maximize2,
  Network,
  Plus,
  RefreshCw,
  RotateCcw,
  Save,
  Search,
  Settings,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import {
  MAX_VISIBLE_NODES,
  VIEW_LEVELS,
  flowEdge,
  hierarchyChildren,
  kindsForLevel,
  layoutNodes,
  visibleSubgraph,
  type AtlasEdge,
  type AtlasNode,
  type FlowNodeData,
  type Graph,
  type ViewLevel,
} from "./graphView";
import { ApiPage } from "./ApiPage";
import { FlowPage } from "./FlowPage";
import {
  FeatureSourceExplorer,
  type FeatureSourceSnippet,
} from "./FeatureSourceExplorer";
import {
  EvidenceCodeInspector,
  type EvidenceFinding,
  type SourceDetails,
  type SourceSpan,
} from "./EvidenceCodeInspector";

const AI_STORAGE_KEY = "code-atlas.ai-settings.v1";
const SYMBOL_KINDS = new Set([
  "Function",
  "Method",
  "Constructor",
  "TypeAlias",
  "Constant",
]);
const defaultModels: Record<AiProvider, string> = {
  openai: "gpt-5",
  mistral: "mistral-large-latest",
  openrouter: "openrouter/auto",
};
const providerNames: Record<AiProvider, string> = {
  openai: "OpenAI",
  mistral: "Mistral",
  openrouter: "OpenRouter",
};
const color: Record<string, string> = {
  Project: "#f4b860",
  ArchitectureZone: "#f4b860",
  ArchitectureGroup: "#d5a956",
  EntryPoint: "#ffd166",
  Workspace: "#f4b860",
  Package: "#9b87f5",
  Crate: "#8d7cf1",
  File: "#4d9de0",
  Module: "#5ec4b6",
  Function: "#61d095",
  Method: "#78dba9",
  Constructor: "#ffd166",
  Class: "#ff7b72",
  Controller: "#fb7185",
  Handler: "#ff8f70",
  Command: "#fbbf24",
  Cli: "#fbbf24",
  Worker: "#f97316",
  Job: "#fb923c",
  Listener: "#e879f9",
  Infrastructure: "#94a3b8",
  Shared: "#a8a29e",
  Entity: "#c4b5fd",
  Model: "#c4b5fd",
  EventSubscriber: "#f472b6",
  EventListener: "#e879f9",
  MessageHandler: "#fb7185",
  Template: "#2dd4bf",
  ExternalService: "#38bdf8",
  Struct: "#ff8e72",
  Interface: "#df78ef",
  Trait: "#df78ef",
  Component: "#3dd6d0",
  Page: "#2dd4bf",
  Route: "#ff9f43",
  ApiEndpoint: "#ff6b6b",
  Provider: "#c084fc",
  Service: "#7dd3fc",
  Repository: "#a3e635",
  Database: "#f59e0b",
  DatabaseTable: "#f59e0b",
  Event: "#f472b6",
  Queue: "#fb7185",
  WebSocket: "#22d3ee",
};
function languageForPath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase();
  return (
    (
      {
        rs: "rust",
        ts: "typescript",
        tsx: "typescript",
        js: "javascript",
        php: "php",
        py: "python",
        go: "go",
        dart: "dart",
      } as Record<string, string>
    )[extension ?? ""] ?? "text"
  );
}

function storedAiSettings(): AiSettings {
  const fallback: AiSettings = {
    provider: "openai",
    apiKey: "",
    model: defaultModels.openai,
  };
  try {
    const value = JSON.parse(
      localStorage.getItem(AI_STORAGE_KEY) ?? "null",
    ) as Partial<AiSettings> | null;
    if (
      !value ||
      !["openai", "mistral", "openrouter"].includes(value.provider ?? "")
    )
      return fallback;
    const provider = value.provider as AiProvider;
    return {
      provider,
      apiKey: value.apiKey ?? "",
      model: value.model || defaultModels[provider],
    };
  } catch {
    return fallback;
  }
}

function AtlasCard({ data, selected }: NodeProps<Node<FlowNodeData>>) {
  const c = color[data.kind] ?? "#8b949e";
  return (
    <div
      className={`atlas-node ${selected ? "selected" : ""} ${data.kind === "ArchitectureZone" ? "zone-node" : ""}`}
      style={{ "--node-color": c } as React.CSSProperties}
    >
      <Handle type="target" position={Position.Left} />
      <span className="node-kind">{data.kind}</span>
      <strong>{data.name}</strong>
      {data.path && <small>{data.path}</small>}
      <Handle type="source" position={Position.Right} />
    </div>
  );
}
const nodeTypes = { atlas: AtlasCard };

export default function App() {
  const [page, setPage] = useState<Page>("projects");
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [projectId, setProjectId] = useState("");
  const [path, setPath] = useState("");
  const [fullGraph, setFullGraph] = useState<Graph | null>(null);
  const [architectureMap, setArchitectureMap] = useState<Graph | null>(null);
  const [selected, setSelected] = useState<Details | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [hoveredEdgeId, setHoveredEdgeId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [enabledKinds, setEnabledKinds] = useState(new Set<string>());
  const [level, setLevel] = useState<ViewLevel>("Architecture");
  const [expanded, setExpanded] = useState(new Set<string>());
  const [focusMode, setFocusMode] = useState(false);
  const [layoutVersion, setLayoutVersion] = useState(0);
  const [flow, setFlow] = useState<ReactFlowInstance<
    Node<FlowNodeData>,
    Edge
  > | null>(null);
  const [status, setStatus] = useState("Ready");
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [progress, setProgress] = useState<ProgressState | null>(null);
  const [tab, setTab] = useState<"relations" | "source" | "impact" | "ai">(
    "relations",
  );
  const [impact, setImpact] = useState<ImpactResult | null>(null);
  const [question, setQuestion] = useState(
    "Explain this element in the project architecture.",
  );
  const [answer, setAnswer] = useState("");
  const [directoryPickerOpen, setDirectoryPickerOpen] = useState(false);
  const [aiSettingsOpen, setAiSettingsOpen] = useState(false);
  const [aiSettings, setAiSettings] = useState<AiSettings>(storedAiSettings);
  const [library, setLibrary] = useState<LibraryEntry[]>([]);
  const [libraryQuery, setLibraryQuery] = useState("");
  const [candidates, setCandidates] = useState<LibraryCandidate[]>([]);
  const [selectedCandidate, setSelectedCandidate] =
    useState<LibraryCandidate | null>(null);
  const [selectedLibrary, setSelectedLibrary] = useState<LibraryEntry | null>(
    null,
  );
  const [documentationGenerating, setDocumentationGenerating] = useState<
    Set<string>
  >(() => new Set());
  const [documentationErrors, setDocumentationErrors] = useState<
    Record<string, string>
  >({});
  const [myCode, setMyCode] = useState<MyCodeItem[]>([]);
  const [myCodeKind, setMyCodeKind] = useState("functions");
  const [qualityFindings, setQualityFindings] = useState<Finding[]>([]);
  const [securityFindings, setSecurityFindings] = useState<Finding[]>([]);
  const [projectDocs, setProjectDocs] = useState<ProjectDocumentation | null>(
    null,
  );
  const [features, setFeatures] = useState<Feature[]>([]);
  const [selectedFeature, setSelectedFeature] = useState<Feature | null>(null);
  const [featureSpec, setFeatureSpec] = useState<FeatureSpec | null>(null);
  const [featureCode, setFeatureCode] = useState<FeatureSnippet[]>([]);
  const [featureGraph, setFeatureGraph] = useState<Graph | null>(null);
  const [featureFindings, setFeatureFindings] = useState<Finding[]>([]);
  const [remoteSearch, setRemoteSearch] = useState<AtlasNode[]>([]);
  const [dashboard, setDashboard] = useState<ProjectDashboard | null>(null);
  const [codeInspector, setCodeInspector] = useState<{
    details: SourceDetails;
    finding?: EvidenceFinding;
  } | null>(null);
  const [gitSeed, setGitSeed] = useState<{ base?: string | null; head?: string | null }>({});
  const [currentDiff, setCurrentDiff] = useState("");
  const [flowSeed, setFlowSeed] = useState("");
  const [assistantOpen, setAssistantOpen] = useState(true);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [testSeed, setTestSeed] = useState("");
  const [estimateSeed, setEstimateSeed] = useState("");
  const [apiSeed, setApiSeed] = useState("");
  const [featureView, setFeatureView] = useState<"overview" | "code" | "map">("overview");
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") { event.preventDefault(); setPaletteOpen(value => !value); }
      if (event.key === "Escape") { setCodeInspector(null); setSelectedFeature(null); setPaletteOpen(false); }
    };
    window.addEventListener("keydown", listener); return () => window.removeEventListener("keydown", listener);
  }, []);
  async function navigatePage(destination: string) {
    const pages = ["dependencies", "projects", "map", "flow", "search", "features", "my-code", "library", "api", "docs", "quality", "security", "tests", "estimate", "git", "mcp"];
    if (!pages.includes(destination)) throw new Error("Page inconnue");
    setCodeInspector(null); setSelectedFeature(null); setPage(destination as Page);
    if (destination === "security" || destination === "quality") await refreshFindings(destination);
    if (destination === "docs") await refreshDocs();
    if (destination === "features") await refreshFeatures();
    if (destination === "my-code") await refreshMyCode();
    if (destination === "library") await refreshLibrary();
  }
  async function handleUiAction(action: UiAction) {
    return dispatchUiAction(action, {
      navigate: navigatePage,
      node: async (id, mode, range, sourceHash) => {
        if (range && sourceHash) await api(`${projectApi(projectId)}/evidence/verify`, { method: "POST", body: JSON.stringify({ node_id:id,source_hash:sourceHash,start_line:range[0],end_line:range[1] }) });
        const details = await api<SourceDetails>(`${projectApi(projectId)}/nodes/${encodeURIComponent(id)}`);
        setSelectedId(id); setSelected(details); setSelectedFeature(null);
        if (mode === "source") {
          setCodeInspector({ details, finding: range ? { id: `assistant:${id}`, title: details.node.name, description: "Source citée par l’assistant", severity: "info", primary_span: { path: details.node.path ?? "", start_line: range[0], end_line: range[1], start_column: 1, end_column: 1 } } : undefined });
        } else {
          setCodeInspector(null); setPage("map"); await loadVisibleView("Symbols", id); setFocusMode(true);
          if (mode === "impact") { setImpact(await api(`${projectApi(projectId)}/impact/${encodeURIComponent(id)}?depth=4`)); setTab("impact"); }
          centerNode(id);
        }
      },
      file: async path => { const details = await api<SourceDetails>(`${projectApi(projectId)}/source?path=${encodeURIComponent(path)}`); setCodeInspector({ details }); },
      feature: async (id, view) => { const data = await api<{ feature: Feature }>(`${projectApi(projectId)}/features/${encodeURIComponent(id)}`); setCodeInspector(null); setFeatureView(view === "source" ? "code" : view === "graph" ? "map" : "overview"); setPage("features"); await openFeature(data.feature); },
      finding: async id => { const all = [...qualityFindings, ...securityFindings]; let finding = all.find(f => f.id === id); if (!finding) { const values = await Promise.all([api<{ items: Finding[] }>(`${projectApi(projectId)}/security`), api<{ items: Finding[] }>(`${projectApi(projectId)}/quality`)]); finding = values.flatMap(v => v.items).find(f => f.id === id); } if (!finding) throw new Error("Constat introuvable"); await inspectFinding(finding); },
      flow: id => { setCodeInspector(null); setSelectedFeature(null); setFlowSeed(id); setPage("flow"); },
      api: async id => { setCodeInspector(null); setSelectedFeature(null); setApiSeed(id ?? ""); setPage("api"); },
      library: async id => { const entry = await api<LibraryEntry>(`/api/library/${encodeURIComponent(id)}`); setCodeInspector(null); setPage("library"); await openLibraryEntry(entry); },
      fit: () => { setPage("map"); void flow?.fitView({ padding: .2 }); },
      filter: kinds => { setEnabledKinds(new Set(kinds)); setPage("map"); },
      diff: (base, head) => { setGitSeed({ base, head }); setCodeInspector(null); setSelectedFeature(null); setPage("git"); },
      tests: feature => { setCodeInspector(null); setSelectedFeature(null); setTestSeed(feature ?? ""); setPage("tests"); },
      estimate: task => { setCodeInspector(null); setSelectedFeature(null); setEstimateSeed(task); setPage("estimate"); },
    });
  }

  async function refreshProjects() {
    const response = await fetch("/api/projects");
    if (response.ok) setProjects(await response.json());
  }
  async function refreshLibrary(q = "") {
    const response = await fetch(
      `/api/library${q ? `?q=${encodeURIComponent(q)}` : ""}`,
    );
    if (response.ok) setLibrary(await response.json());
  }
  async function refreshMyCode(kind = myCodeKind) {
    if (!projectId) return;
    const response = await fetch(
      `/api/projects/${projectId}/my-code?kind=${encodeURIComponent(kind)}&limit=200`,
    );
    if (response.ok) {
      const value = await response.json();
      setMyCode(value.items);
    }
  }
  async function refreshFindings(category: "quality" | "security") {
    if (!projectId) return;
    setStatus(`Running deterministic ${category} detectors…`);
    const response = await fetch(`/api/projects/${projectId}/${category}`);
    if (response.ok) {
      const value = await response.json();
      category === "quality"
        ? setQualityFindings(value.items)
        : setSecurityFindings(value.items);
      setStatus(`${value.items.length} ${category} finding(s)`);
    } else setStatus(`${category} scan failed`);
  }
  async function setFindingStatus(finding: Finding, status: Finding["status"]) {
    const response = await fetch(
      `/api/findings/${encodeURIComponent(finding.id)}`,
      {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ status }),
      },
    );
    if (response.ok) {
      const updated = await response.json();
      const update = (items: Finding[]) =>
        items.map((item) => (item.id === updated.id ? updated : item));
      finding.category === "security"
        ? setSecurityFindings(update)
        : setQualityFindings(update);
    }
  }
  async function refreshDocs(generate = false) {
    if (!projectId) return;
    setStatus(
      generate
        ? "Generating hierarchical documentation…"
        : "Loading documentation…",
    );
    const configuration = aiSettings.apiKey.trim()
      ? {
          provider: aiSettings.provider,
          api_key: aiSettings.apiKey,
          model: aiSettings.model,
        }
      : undefined;
    const response = await fetch(
      `/api/projects/${projectId}/docs${generate ? "/generate" : ""}`,
      {
        method: generate ? "POST" : "GET",
        headers: generate ? { "content-type": "application/json" } : undefined,
        body: generate ? JSON.stringify({ configuration }) : undefined,
      },
    );
    if (response.ok) {
      const value = await response.json();
      setProjectDocs(value);
      setStatus(
        value.cache_hit ? "Documentation cache hit" : "Documentation generated",
      );
    } else {
      const value = await response.json();
      setStatus(value.error ?? "Documentation failed");
    }
  }
  async function refreshFeatures(detect = false) {
    if (!projectId) return;
    setStatus(detect ? "Detecting semantic Features…" : "Loading features…");
    const configuration = aiSettings.apiKey.trim()
      ? {
          provider: aiSettings.provider,
          api_key: aiSettings.apiKey,
          model: aiSettings.model,
        }
      : undefined;
    const response = await fetch(
      `/api/projects/${projectId}/features${detect ? "/detect" : ""}`,
      {
        method: detect ? "POST" : "GET",
        headers: detect ? { "content-type": "application/json" } : undefined,
        body: detect ? JSON.stringify({ configuration }) : undefined,
      },
    );
    if (response.ok) {
      const value = await response.json();
      setFeatures(value.items);
      setStatus(
        `${value.items.length} Feature(s) · ${value.semantic_analysis ?? "persisted"}`,
      );
    } else {
      const value = await response.json();
      setStatus(value.error ?? "Feature detection failed");
    }
  }
  async function explainFinding(finding: Finding) {
    const configuration = aiSettings.apiKey.trim()
      ? {
          provider: aiSettings.provider,
          api_key: aiSettings.apiKey,
          model: aiSettings.model,
        }
      : undefined;
    setStatus("Explaining finding with grounded context…");
    const response = await fetch(
      `/api/findings/${encodeURIComponent(finding.id)}/explain`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ configuration }),
      },
    );
    const value = await response.json();
    if (response.ok) {
      const updated = value.finding as Finding;
      const update = (items: Finding[]) =>
        items.map((item) => (item.id === updated.id ? updated : item));
      finding.category === "security"
        ? setSecurityFindings(update)
        : setQualityFindings(update);
      setStatus(`${value.provider} explanation saved`);
    } else setStatus(value.error ?? "AI explanation failed");
  }
  async function runSecurityReview() {
    const configuration = aiSettings.apiKey.trim()
      ? {
          provider: aiSettings.provider,
          api_key: aiSettings.apiKey,
          model: aiSettings.model,
        }
      : undefined;
    setStatus("Running grounded AI security review…");
    const response = await fetch(`/api/projects/${projectId}/security/review`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ configuration }),
    });
    const value = await response.json();
    setStatus(
      response.ok
        ? value.review.executive_summary
        : (value.error ?? "Security review failed"),
    );
  }
  async function openFeature(feature: Feature) {
    const base = `/api/projects/${projectId}/features/${encodeURIComponent(feature.id)}`;
    const [detail, code, graph, spec] = await Promise.all([
      fetch(base),
      fetch(`${base}/code`),
      fetch(`${base}/graph`),
      fetch(`${base}/spec`),
    ]);
    if (detail.ok) {
      const value = await detail.json();
      setSelectedFeature(value.feature);
      setFeatureFindings(value.findings ?? []);
    }
    if (code.ok) setFeatureCode((await code.json()).items);
    if (graph.ok) setFeatureGraph(await graph.json());
    if (spec.ok) setFeatureSpec(await spec.json());
  }
  async function inspectCode(nodeId: string) {
    if (!projectId) return;
    setStatus("Chargement du code source…");
    const response = await fetch(
      `/api/projects/${projectId}/nodes/${encodeURIComponent(nodeId)}`,
    );
    if (response.ok) {
      setCodeInspector({ details: await response.json() });
      setStatus("Code source chargé");
    } else setStatus("Code source indisponible");
  }
  async function inspectFinding(finding: Finding) {
    const nodeId = finding.node_ids[0];
    if (!projectId || !nodeId) return;
    setStatus("Chargement de la preuve exacte…");
    const response = await fetch(
      `/api/projects/${projectId}/nodes/${encodeURIComponent(nodeId)}`,
    );
    if (response.ok) {
      setCodeInspector({ details: await response.json(), finding });
      setStatus("Preuve source chargée");
    } else setStatus("Preuve source indisponible");
  }
  async function updateFeature(
    feature: Feature,
    update: Partial<Pick<Feature, "name" | "description" | "status">>,
  ) {
    const response = await fetch(
      `/api/projects/${projectId}/features/${encodeURIComponent(feature.id)}`,
      {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(update),
      },
    );
    if (response.ok) {
      const value = await response.json();
      setFeatures((current) =>
        current.map((item) => (item.id === value.id ? value : item)),
      );
      setSelectedFeature((current) =>
        current?.id === value.id ? value : current,
      );
    }
  }
  async function saveFeatureSpec(feature: Feature, spec: FeatureSpec) {
    const response = await fetch(
      `/api/projects/${projectId}/features/${encodeURIComponent(feature.id)}/spec`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ spec }),
      },
    );
    if (response.ok) setFeatureSpec(await response.json());
  }
  async function loadProject(id: string) {
    const mapResponse = await fetch(`/api/projects/${id}/map`);
    if (!mapResponse.ok) return;
    const mapValue = await mapResponse.json();
    const map: Graph | null = mapValue
      ? { ...mapValue, unresolved_calls: [] }
      : null;
    setProjectId(id);
    setFullGraph(map);
    setArchitectureMap(map);
    setEnabledKinds(new Set((map?.nodes ?? []).map((node) => node.kind)));
    setLevel("Architecture");
    setExpanded(new Set());
    setFocusMode(false);
    setSelected(null);
    setSelectedId(null);
    setSelectedFeature(null); setCodeInspector(null); setFeatures([]); setTestSeed(""); setEstimateSeed(""); setApiSeed("");
    setPage("map");
    setLayoutVersion((value) => value + 1);
    const candidateResponse = await fetch(
      `/api/projects/${id}/library-candidates`,
    );
    if (candidateResponse.ok)
      setCandidates((await candidateResponse.json()).items ?? []);
    const dashboardResponse = await fetch(`/api/projects/${id}/dashboard`);
    if (dashboardResponse.ok) setDashboard(await dashboardResponse.json());
  }
  async function loadVisibleView(nextLevel: ViewLevel, focus?: string) {
    if (!projectId) return;
    setLevel(nextLevel);
    setFocusMode(Boolean(focus));
    setExpanded(new Set());
    if (nextLevel === "Architecture") {
      if (architectureMap) setFullGraph(architectureMap);
      setLayoutVersion((value) => value + 1);
      return;
    }
    const params = new URLSearchParams({ level: nextLevel, limit: "650" });
    if (focus) params.set("focus", focus);
    const response = await fetch(`/api/projects/${projectId}/view?${params}`);
    if (response.ok) {
      const graph: Graph = await response.json();
      setFullGraph(graph);
      setEnabledKinds(new Set(graph.nodes.map((node) => node.kind)));
      setLayoutVersion((value) => value + 1);
    }
  }

  useEffect(() => {
    void refreshProjects();
    void refreshLibrary();
  }, []);
  useEffect(() => {
    if (!projectId || !query.trim()) {
      setRemoteSearch([]);
      return;
    }
    const timer = window.setTimeout(async () => {
      const response = await fetch(
        `/api/projects/${projectId}/search?q=${encodeURIComponent(query)}&limit=12`,
      );
      if (response.ok)
        setRemoteSearch(
          (await response.json()).map((hit: { node: AtlasNode }) => hit.node),
        );
    }, 180);
    return () => window.clearTimeout(timer);
  }, [projectId, query]);
  useEffect(() => {
    if (!projectId || level === "Architecture" || focusMode) return;
    const controller = new AbortController();
    void (async () => {
      const response = await fetch(
        `/api/projects/${projectId}/view?level=${encodeURIComponent(level)}&limit=650`,
        { signal: controller.signal },
      );
      if (response.ok) {
        const graph: Graph = await response.json();
        setFullGraph(graph);
        setEnabledKinds(new Set(graph.nodes.map((node) => node.kind)));
        setLayoutVersion((value) => value + 1);
      }
    })();
    return () => controller.abort();
  }, [projectId, level, focusMode]);
  useEffect(() => {
    const scheme = location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(`${scheme}://${location.host}/api/events`);
    ws.onmessage = (event) => {
      const value = JSON.parse(event.data);
      if (value.type === "analysis_started") {
        setIsAnalyzing(true);
        setProgress({
          project_id: value.project_id,
          stage: "scanning",
          processed: 0,
          total: value.total_files,
          nodes: 0,
          edges: 0,
          elapsed_ms: 0,
        });
        setStatus("Analysis started");
      } else if (value.type === "analysis_progress") {
        setIsAnalyzing(true);
        setProgress(value);
        setStatus(value.stage.replaceAll("_", " "));
      } else if (value.type === "analysis_completed") {
        setIsAnalyzing(false);
        setProgress({
          ...value,
          stage: "completed",
          processed: value.files,
          total: value.files,
          elapsed_ms: value.duration_ms,
        });
        setStatus(`${value.nodes} nodes · ${value.edges} edges`);
        void loadProject(value.project_id);
        void refreshProjects();
      } else if (value.type === "analysis_error") {
        setIsAnalyzing(false);
        setStatus(value.message);
        setProgress(null);
      } else if (value.type === "file_changed") {
        setStatus(`${value.paths.length} file change(s) detected`);
      }
    };
    return () => ws.close();
  }, []);
  useEffect(() => {
    if (!flow || !fullGraph) return;
    const timer = window.setTimeout(
      () => void flow.fitView({ padding: 0.16, duration: 450, maxZoom: 1 }),
      80,
    );
    return () => window.clearTimeout(timer);
  }, [flow, fullGraph, level, layoutVersion]);

  async function analyze(target = path) {
    if (!target.trim()) {
      setDirectoryPickerOpen(true);
      return;
    }
    setPath(target);
    setIsAnalyzing(true);
    setProgress({
      stage: "starting",
      processed: 0,
      total: 0,
      nodes: 0,
      edges: 0,
      elapsed_ms: 0,
    });
    setStatus("Starting analysis…");
    setSelected(null);
    const response = await fetch("/api/projects/analyze", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path: target, watch: true }),
    });
    const value = await response.json();
    if (!response.ok) {
      setIsAnalyzing(false);
      setStatus(value.error ?? "Analysis failed");
      return;
    }
    await loadProject(value.id);
    setIsAnalyzing(false);
    void refreshProjects();
  }
  async function deleteProject(id: string) {
    if (
      !confirm(
        "Delete this project from Code Atlas? Source files stay untouched.",
      )
    )
      return;
    const response = await fetch(`/api/projects/${id}`, { method: "DELETE" });
    if (response.ok) {
      if (projectId === id) {
        setProjectId("");
        setFullGraph(null);
        setArchitectureMap(null);
        setPage("projects");
      }
      void refreshProjects();
    }
  }
  async function chooseById(id: string) {
    if (!projectId) return;
    const response = await fetch(
      `/api/projects/${projectId}/nodes/${encodeURIComponent(id)}`,
    );
    if (response.ok) {
      setSelected(await response.json());
      setTab("relations");
      setImpact(null);
      setAnswer("");
    }
  }
  function centerNode(id: string) {
    window.setTimeout(
      () =>
        void flow?.fitView({
          nodes: [{ id }],
          padding: 0.8,
          duration: 450,
          maxZoom: 1.3,
        }),
      70,
    );
  }
  async function navigateTo(id: string) {
    if (!fullGraph?.nodes.some((node) => node.id === id))
      await loadVisibleView("Symbols", id);
    setSelectedId(id);
    setFocusMode(true);
    await chooseById(id);
    centerNode(id);
  }
  function expandNode(id: string) {
    setFocusMode(false);
    setExpanded((current) => {
      const next = new Set(current);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
    setLayoutVersion((value) => value + 1);
  }
  async function runImpact() {
    if (!selected) return;
    const response = await fetch(
      `/api/projects/${projectId}/impact/${encodeURIComponent(selected.node.id)}?depth=4`,
    );
    setImpact(await response.json());
    setTab("impact");
  }
  async function ask() {
    if (!selected) return;
    setAnswer(`Thinking with ${providerNames[aiSettings.provider]}…`);
    const configuration = aiSettings.apiKey.trim()
      ? {
          provider: aiSettings.provider,
          api_key: aiSettings.apiKey,
          model: aiSettings.model,
        }
      : undefined;
    const response = await fetch(`/api/projects/${projectId}/ai/query`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        question,
        node_id: selected.node.id,
        configuration,
      }),
    });
    const value = await response.json();
    setAnswer(
      value.answer ?? value.error ?? "The AI provider returned no answer.",
    );
    setTab("ai");
  }
  function saveAiSettings(value: AiSettings) {
    setAiSettings(value);
    localStorage.setItem(AI_STORAGE_KEY, JSON.stringify(value));
    setAiSettingsOpen(false);
    setStatus(`${providerNames[value.provider]} configuration saved locally`);
  }
  async function addSelectedToLibrary() {
    if (!selected) return;
    const response = await fetch("/api/library", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        project_id: projectId,
        node_id: selected.node.id,
      }),
    });
    const value = await response.json();
    setStatus(
      response.ok
        ? `${value.display_name} added to Library`
        : (value.error ?? "Cannot add to Library"),
    );
    if (response.ok) void refreshLibrary();
  }
  async function addCandidate(candidate: LibraryCandidate) {
    const response = await fetch("/api/library", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        project_id: projectId,
        node_id: candidate.node.id,
      }),
    });
    const value = await response.json();
    setStatus(
      response.ok
        ? `${candidate.node.name} added to Library`
        : (value.error ?? "Cannot add to Library"),
    );
    if (response.ok) {
      void refreshLibrary();
      setCandidates((current) =>
        current.filter((item) => item.node.id !== candidate.node.id),
      );
      setSelectedCandidate(null);
    }
  }
  async function addMyCodeToLibrary(nodeId: string) {
    const response = await fetch("/api/library", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ project_id: projectId, node_id: nodeId }),
    });
    const value = await response.json();
    setStatus(
      response.ok
        ? `${value.display_name} added to Library`
        : (value.error ?? "Cannot add to Library"),
    );
    if (response.ok) {
      void refreshLibrary();
      void refreshMyCode();
    }
  }
  async function openMyCodeAction(
    nodeId: string,
    destination: "relations" | "ai" | "impact",
  ) {
    setPage("map");
    await navigateTo(nodeId);
    if (destination === "impact") {
      const response = await fetch(
        `/api/projects/${projectId}/impact/${encodeURIComponent(nodeId)}?depth=4`,
      );
      if (response.ok) setImpact(await response.json());
    }
    setTab(destination);
  }
  async function openLibraryEntry(entry: LibraryEntry) {
    setSelectedLibrary(entry);
    const response = await fetch(
      `/api/library/${encodeURIComponent(entry.id)}`,
    );
    if (response.ok) setSelectedLibrary(await response.json());
  }
  async function removeLibrary(id: string) {
    const response = await fetch(`/api/library/${encodeURIComponent(id)}`, {
      method: "DELETE",
    });
    if (response.ok) {
      if (selectedLibrary?.id === id) setSelectedLibrary(null);
      void refreshLibrary(libraryQuery);
    }
  }
  async function generateDocumentation(id: string) {
    const configuration = aiSettings.apiKey.trim()
      ? {
          provider: aiSettings.provider,
          api_key: aiSettings.apiKey,
          model: aiSettings.model,
        }
      : undefined;
    setDocumentationGenerating((current) => new Set(current).add(id));
    setDocumentationErrors((current) => {
      const next = { ...current };
      delete next[id];
      return next;
    });
    setStatus("Generating structured documentation…");
    try {
      const response = await fetch(
        `/api/library/${encodeURIComponent(id)}/generate-documentation`,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ configuration }),
        },
      );
      const value = await response.json();
      if (response.ok) {
        const entry = value as LibraryEntry;
        setLibrary((current) =>
          current.map((item) => (item.id === id ? entry : item)),
        );
        setSelectedLibrary((current) => (current?.id === id ? entry : current));
        setStatus("Documentation saved");
      } else {
        const message = value.error ?? "Documentation failed";
        setDocumentationErrors((current) => ({ ...current, [id]: message }));
        setStatus(message);
      }
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Documentation request failed";
      setDocumentationErrors((current) => ({ ...current, [id]: message }));
      setStatus(message);
    } finally {
      setDocumentationGenerating((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      });
    }
  }

  const graphForView =
    level === "Architecture" && architectureMap ? architectureMap : fullGraph;
  const visibleGraph = useMemo(
    () =>
      graphForView
        ? visibleSubgraph(
            graphForView,
            level,
            enabledKinds,
            expanded,
            selectedId,
            focusMode,
          )
        : null,
    [graphForView, level, enabledKinds, expanded, selectedId, focusMode],
  );
  const relatedIds = useMemo(() => {
    const ids = new Set<string>();
    if (selectedId && graphForView) {
      ids.add(selectedId);
      graphForView.edges.forEach((edge) => {
        if (edge.source_id === selectedId) ids.add(edge.target_id);
        if (edge.target_id === selectedId) ids.add(edge.source_id);
      });
    }
    return ids;
  }, [selectedId, graphForView]);
  const flowNodes: Node<FlowNodeData>[] = useMemo(() => {
    void layoutVersion;
    if (!visibleGraph) return [];
    return layoutNodes(visibleGraph.nodes, visibleGraph.edges, selectedId).map(
      (node) => ({
        ...node,
        style: {
          opacity:
            selectedId && !focusMode && !relatedIds.has(node.id) ? 0.18 : 1,
        },
      }),
    );
  }, [visibleGraph, selectedId, focusMode, relatedIds, layoutVersion]);
  const flowEdges: Edge[] = useMemo(
    () =>
      visibleGraph
        ? visibleGraph.edges.map((edge) =>
            flowEdge(edge, selectedId, hoveredEdgeId ?? selectedEdgeId),
          )
        : [],
    [visibleGraph, selectedId, hoveredEdgeId, selectedEdgeId],
  );
  const kinds = useMemo(() => {
    const counts = new Map<string, number>();
    fullGraph?.nodes.forEach((node) =>
      counts.set(node.kind, (counts.get(node.kind) ?? 0) + 1),
    );
    return [...counts].sort((a, b) => b[1] - a[1]);
  }, [fullGraph]);
  const searchResults = remoteSearch;
  const selectedLibraryEligible =
    selected &&
    ["Function", "Method", "Constructor"].includes(selected.node.kind) &&
    ["Project", "WorkspacePackage", "project", "workspace_package"].includes(
      selected.node.source_scope ?? "Project",
    );

  return (
    <main className={"shell" + (assistantOpen && projectId ? " with-assistant" : "")}>
      <header>
        <div className="brand">
          <div className="logo">
            <GitBranch size={18} />
          </div>
          <div>
            <strong>Code Atlas</strong>
            <span>Intelligence projet</span>
          </div>
        </div>
        <nav className="main-nav" aria-label="Navigation principale">
          <button
            className={page === "projects" ? "active" : ""}
            onClick={() => setPage("projects")}
          >
            Projets
          </button>
          <button
            className={page === "map" ? "active" : ""}
            disabled={!fullGraph}
            onClick={() => setPage("map")}
          >
            Carte
          </button>
          <button
            className={page === "flow" ? "active" : ""}
            disabled={!projectId}
            onClick={() => setPage("flow")}
          >
            Flux
          </button>
          <button
            className={page === "features" ? "active" : ""}
            disabled={!projectId}
            onClick={() => {
              setPage("features");
              void refreshFeatures();
            }}
          >
            Features
          </button>
          <button
            className={page === "search" ? "active" : ""}
            disabled={!fullGraph}
            onClick={() => setPage("search")}
          >
            Recherche
          </button>
          <button
            className={page === "my-code" ? "active" : ""}
            disabled={!projectId}
            onClick={() => {
              setPage("my-code");
              void refreshMyCode();
            }}
          >
            Mon code
          </button>
          <button
            className={page === "library" ? "active" : ""}
            onClick={() => {
              setPage("library");
              void refreshLibrary();
            }}
          >
            Bibliothèque
          </button>
          <button
            className={page === "api" ? "active" : ""}
            disabled={!projectId}
            onClick={() => setPage("api")}
          >
            API
          </button>
          <button
            className={page === "docs" ? "active" : ""}
            disabled={!projectId}
            onClick={() => {
              setPage("docs");
              void refreshDocs();
            }}
          >
            Documentation
          </button>
          <button
            className={page === "quality" ? "active" : ""}
            disabled={!projectId}
            onClick={() => {
              setPage("quality");
              void refreshFindings("quality");
            }}
          >
            Qualité
          </button>
          <button
            className={page === "security" ? "active" : ""}
            disabled={!projectId}
            onClick={() => {
              setPage("security");
              void refreshFindings("security");
            }}
          >
            Sécurité
          </button>
          <button
            className={page === "mcp" ? "active" : ""}
            onClick={() => setPage("mcp")}
          >
            MCP
          </button>
          {([['tests', 'Tests'], ['estimate', 'Estimation'], ['git', 'Git'], ['dependencies', 'Dépendances']] as const).map(([destination, label]) => <button key={destination} disabled={!projectId} className={page === destination ? "active" : ""} onClick={() => setPage(destination)}>{label}</button>)}
        </nav>
        <button onClick={() => setPaletteOpen(true)} aria-label="Palette de commandes">⌘ K</button>
        <button disabled={!projectId} onClick={() => setAssistantOpen(value => !value)} aria-expanded={assistantOpen}>Assistant</button>
        <div className="project-input">
          <button
            className="browse-button"
            aria-label="Parcourir les dossiers"
            onClick={() => setDirectoryPickerOpen(true)}
          >
            <FolderOpen size={16} />
          </button>
          <input
            value={path}
            onChange={(event) => setPath(event.target.value)}
            placeholder="Choisir un dossier de projet"
            onKeyDown={(event) => event.key === "Enter" && void analyze()}
          />
          <button disabled={isAnalyzing} onClick={() => void analyze()}>
            {isAnalyzing ? (
              <LoaderCircle className="spinner" size={15} />
            ) : (
              <Activity size={15} />
            )}{" "}
            {isAnalyzing ? "Analyse…" : "Analyser"}
          </button>
        </div>
        <button
          className="settings-button"
          onClick={() => setAiSettingsOpen(true)}
        >
          <Settings size={16} />
          <span>{providerNames[aiSettings.provider]}</span>
        </button>
        <div className="status">
          <i />
          {status}
        </div>
      </header>
      {isAnalyzing && progress && <AnalysisProgressBar progress={progress} />}
      {projectId && <div className="workspace-breadcrumb"><b>{projects.find(p => p.id === projectId)?.name ?? projectId}</b><span>{projects.find(p => p.id === projectId)?.git_branch ?? "Local"}</span><span>{page}</span>{selectedId && <code>{selected?.node.name}</code>}</div>}
      {page === "dependencies" && projectId && <DependenciesPage key={"deps:" + projectId} projectId={projectId} onFile={path => void handleUiAction({ type: "OPEN_FILE", path }).catch(error => setStatus(String(error)))} />}
      {page === "tests" && projectId && <TestsPage key={"TestsPage:" + projectId} projectId={projectId} features={features} seed={testSeed} onInspect={id => void inspectCode(id)} />}
      {page === "estimate" && projectId && <EstimatePage key={"EstimatePage:" + projectId} features={features} onFeature={id => { const feature = features.find(f => f.id === id); if (feature) { setPage("features"); void openFeature(feature); } }} projectId={projectId} seed={estimateSeed} onInspect={id => void inspectCode(id)} />}
      {page === "git" && projectId && <GitPage seed={gitSeed} onComparison={setCurrentDiff} key={`GitPage:${projectId}:${gitSeed.base}:${gitSeed.head}`} features={features} onFeature={id => { const feature = features.find(f => f.id === id); if (feature) { setPage("features"); void openFeature(feature); } }} projectId={projectId} onInspect={id => void inspectCode(id)} />}
      {assistantOpen && projectId && <AssistantPanel key={"AssistantPanel:" + projectId} projectId={projectId} projectName={projects.find(p => p.id === projectId)?.name ?? "Projet"} context={{ active_page: page, selected_node: selectedId ?? undefined, selected_feature: selectedFeature?.id, selected_file: codeInspector?.details.node.path, selected_finding: codeInspector?.finding?.id, selected_range: codeInspector?.finding?.primary_span ? [codeInspector.finding.primary_span.start_line, codeInspector.finding.primary_span.end_line ?? codeInspector.finding.primary_span.start_line] : undefined, current_flow: page === "flow" ? flowSeed : undefined, current_diff: page === "git" ? currentDiff : undefined }} configuration={aiSettings.apiKey ? { provider: aiSettings.provider, api_key: aiSettings.apiKey, model: aiSettings.model } : undefined} onAction={handleUiAction} onClose={() => setAssistantOpen(false)} />}
      {paletteOpen && <CommandPalette projectId={projectId} projects={projects} features={features} onProject={id => void loadProject(id)} onAction={handleUiAction} onAssistant={() => setAssistantOpen(true)} onClose={() => setPaletteOpen(false)} />}
      {page === "projects" && (
        <ProjectsPage
          projects={projects}
          onOpen={(id) => void loadProject(id)}
          onAnalyze={(root) => void analyze(root)}
          onDelete={(id) => void deleteProject(id)}
          onAdd={() => setDirectoryPickerOpen(true)}
        />
      )}
      {page === "library" && (
        <LibraryPage
          entries={library}
          candidates={projectId ? candidates : []}
          query={libraryQuery}
          generating={documentationGenerating}
          errors={documentationErrors}
          onQuery={(value) => {
            setLibraryQuery(value);
            void refreshLibrary(value);
          }}
          onInspect={setSelectedCandidate}
          onOpen={(entry) => void openLibraryEntry(entry)}
          onDelete={(id) => void removeLibrary(id)}
          onDocument={(id) => void generateDocumentation(id)}
        />
      )}
      {page === "my-code" && (
        <MyCodePage
          items={myCode}
          kind={myCodeKind}
          onKind={(value) => {
            setMyCodeKind(value);
            void refreshMyCode(value);
          }}
          onInspect={(id) => void inspectCode(id)}
          onExplain={(id) => void openMyCodeAction(id, "ai")}
          onImpact={(id) => void openMyCodeAction(id, "impact")}
          onAdd={(id) => void addMyCodeToLibrary(id)}
        />
      )}
      {page === "flow" && projectId && (
        <FlowPage
          projectId={projectId}
          seed={flowSeed}
          onInspect={(id) => void inspectCode(id)}
        />
      )}
      {page === "api" && projectId && (
        <ApiPage
          seed={apiSeed}
          projectId={projectId}
          onInspect={(id) => void inspectCode(id)}
          onTrace={(id) => {
            setFlowSeed(id);
            setPage("flow");
          }}
        />
      )}
      {page === "quality" && (
        <FindingsPage
          title="Qualité"
          description="Constats déterministes de qualité, d’architecture et de performance, avec preuve et niveau de confiance."
          items={qualityFindings}
          onInspect={(finding) => void inspectFinding(finding)}
          onStatus={(finding, status) => void setFindingStatus(finding, status)}
          onExplain={(finding) => void explainFinding(finding)}
        />
      )}
      {page === "security" && (
        <FindingsPage
          title="Sécurité"
          description="Candidats de sécurité statiques conservateurs : les motifs faibles et sans contexte d’entrée externe sont écartés."
          items={securityFindings}
          onInspect={(finding) => void inspectFinding(finding)}
          onStatus={(finding, status) => void setFindingStatus(finding, status)}
          onExplain={(finding) => void explainFinding(finding)}
          onReview={() => void runSecurityReview()}
        />
      )}
      {page === "mcp" && <McpPage />}
      {page === "docs" && (
        <DocsPage
          documentation={projectDocs}
          projectId={projectId}
          onRefresh={() => void refreshDocs(true)}
        />
      )}
      {page === "features" && (
        <FeaturesPage
          items={features}
          onDetect={() => void refreshFeatures(true)}
          onInspect={(feature) => void openFeature(feature)}
          onAccept={(feature) =>
            void updateFeature(feature, { status: "accepted" })
          }
          onIgnore={(feature) =>
            void updateFeature(feature, { status: "ignored" })
          }
        />
      )}
      {page === "search" && projectId && <HybridSearchPage configuration={aiSettings.apiKey ? { provider: aiSettings.provider, api_key: aiSettings.apiKey } : undefined} key={"HybridSearchPage:" + projectId} projectId={projectId} onInspect={id => void inspectCode(id)} onFocus={id => { setPage("map"); void navigateTo(id); }} />}
      {page === "map" && (
        <section className="workspace">
          <aside className="explorer">
            <h2>
              <Layers3 size={15} />
              Project Map
            </h2>
            <div className="search">
              <Search size={14} />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && searchResults[0])
                    void navigateTo(searchResults[0].id);
                }}
                placeholder="Find and focus…"
              />
            </div>
            {query && (
              <div className="search-results">
                {searchResults.map((node) => (
                  <button
                    key={node.id}
                    onClick={() => void navigateTo(node.id)}
                  >
                    <b>{node.name}</b>
                    <span>
                      {node.kind} · {node.path ?? "virtual"}
                    </span>
                  </button>
                ))}
              </div>
            )}
            <div className="project-root">
              <Box size={14} />
              <span>{fullGraph?.root ?? "No project loaded"}</span>
            </div>
            <div className="quick-filters">
              <button
                onClick={() =>
                  setEnabledKinds(new Set(kinds.map(([kind]) => kind)))
                }
              >
                Show all
              </button>
              <button
                onClick={() =>
                  setEnabledKinds(
                    (current) =>
                      new Set(
                        [...current].filter((kind) => !SYMBOL_KINDS.has(kind)),
                      ),
                  )
                }
              >
                Hide symbols
              </button>
              <button
                onClick={() => {
                  setLevel("Architecture");
                  setFocusMode(false);
                }}
              >
                Architecture only
              </button>
            </div>
            <div className="architecture-links">
              <button onClick={() => setLevel("Architecture")}>
                Architecture
              </button>
              <button
                onClick={() =>
                  setEnabledKinds(
                    (current) => new Set([...current, "EntryPoint"]),
                  )
                }
              >
                Entry Points
              </button>
              <button
                onClick={() =>
                  setEnabledKinds(
                    (current) => new Set([...current, "Route", "ApiEndpoint"]),
                  )
                }
              >
                Routes & APIs
              </button>
              <button
                onClick={() =>
                  setEnabledKinds(
                    (current) => new Set([...current, "Service", "Repository"]),
                  )
                }
              >
                Services
              </button>
              <button
                onClick={() =>
                  setEnabledKinds(
                    (current) =>
                      new Set([...current, "Database", "DatabaseTable"]),
                  )
                }
              >
                Database
              </button>
            </div>
            <div className="kinds">
              {kinds.map(([kind, count]) => (
                <button
                  className={enabledKinds.has(kind) ? "active" : ""}
                  onClick={() =>
                    setEnabledKinds((current) => {
                      const next = new Set(current);
                      next.has(kind) ? next.delete(kind) : next.add(kind);
                      return next;
                    })
                  }
                  key={kind}
                >
                  <span
                    className="dot"
                    style={{ background: color[kind] ?? "#8b949e" }}
                  />
                  <span>{kind}</span>
                  <em>{count}</em>
                </button>
              ))}
            </div>
            {fullGraph && (
              <div className="summary">
                <b>{fullGraph.nodes.length.toLocaleString()}</b> total nodes ·{" "}
                <b>{visibleGraph?.nodes.length ?? 0}</b> visible
                <br />
                <b>{fullGraph.edges.length.toLocaleString()}</b> relations ·{" "}
                <b>{fullGraph.unresolved_calls.length.toLocaleString()}</b>{" "}
                unresolved
              </div>
            )}
          </aside>
          <div className="canvas">
            {fullGraph && visibleGraph ? (
              <>
                <div className="graph-toolbar">
                  <div className="view-switch">
                    <span>View</span>
                    {VIEW_LEVELS.map((value) => (
                      <button
                        className={level === value ? "active" : ""}
                        onClick={() => {
                          setLevel(value);
                          setFocusMode(false);
                          setExpanded(new Set());
                          setLayoutVersion((version) => version + 1);
                        }}
                        key={value}
                      >
                        {value}
                      </button>
                    ))}
                  </div>
                  <div className="map-actions">
                    <button
                      disabled={!selectedId}
                      className={focusMode ? "active" : ""}
                      onClick={() => setFocusMode(true)}
                    >
                      <Crosshair size={13} />
                      Focus selected
                    </button>
                    <button onClick={() => setFocusMode(false)}>
                      <Network size={13} />
                      Show all level
                    </button>
                    <button
                      onClick={() =>
                        void flow?.fitView({ padding: 0.16, duration: 400 })
                      }
                    >
                      <Maximize2 size={13} />
                      Fit View
                    </button>
                    <button
                      disabled={!selectedId}
                      onClick={() => selectedId && centerNode(selectedId)}
                    >
                      <Crosshair size={13} />
                      Center selected
                    </button>
                    <button
                      onClick={() => setLayoutVersion((value) => value + 1)}
                    >
                      <RotateCcw size={13} />
                      Auto layout
                    </button>
                  </div>
                  <div className="visible-count">
                    {visibleGraph.nodes.length} nodes ·{" "}
                    {visibleGraph.edges.length} edges
                    {visibleGraph.truncated &&
                      ` · capped at ${MAX_VISIBLE_NODES}`}
                  </div>
                </div>
                <ReactFlow
                  nodes={flowNodes}
                  edges={flowEdges}
                  nodeTypes={nodeTypes}
                  onInit={(instance) => setFlow(instance)}
                  onNodeClick={(_, node) => void navigateTo(node.id)}
                  onNodeDoubleClick={(_, node) => expandNode(node.id)}
                  onEdgeMouseEnter={(_, edge) => setHoveredEdgeId(edge.id)}
                  onEdgeMouseLeave={() => setHoveredEdgeId(null)}
                  onEdgeClick={(_, edge) => setSelectedEdgeId(edge.id)}
                  onPaneClick={() => setSelectedEdgeId(null)}
                  minZoom={0.08}
                  maxZoom={2}
                >
                  <Background color="#252a34" gap={28} />
                  <Controls />
                  {flowNodes.length > 1 && flowNodes.length <= 350 && (
                    <MiniMap
                      pannable
                      zoomable
                      nodeColor={(node) =>
                        color[(node.data as FlowNodeData).kind] ?? "#8b949e"
                      }
                      maskColor="rgba(4,6,10,.68)"
                    />
                  )}
                </ReactFlow>
              </>
            ) : (
              <div className="empty">
                <Network size={48} />
                <h1>Open a project map</h1>
                <p>Select a saved project or analyze another codebase.</p>
              </div>
            )}
          </div>
          {selected && (
            <aside className="inspector">
              <div className="inspector-head">
                <div>
                  <span>{selected.node.kind}</span>
                  <h2>{selected.node.name}</h2>
                </div>
                <button
                  onClick={() => {
                    setSelected(null);
                    setSelectedId(null);
                    setFocusMode(false);
                  }}
                >
                  <X size={17} />
                </button>
              </div>
              <div className="meta">
                <code>{selected.node.path ?? "virtual node"}</code>
                <span>
                  {selected.node.language ?? "Universal"} · L
                  {selected.node.start_line ?? "—"}–
                  {selected.node.end_line ?? "—"}
                </span>
                {hierarchyChildren(fullGraph!, selected.node.id).length > 0 && (
                  <button
                    className="expand-action"
                    onClick={() => expandNode(selected.node.id)}
                  >
                    {expanded.has(selected.node.id)
                      ? "Collapse"
                      : "Expand contents"}
                  </button>
                )}
                {selectedLibraryEligible && (
                  <button
                    className="library-action"
                    onClick={() => void addSelectedToLibrary()}
                  >
                    <BookOpen size={13} />
                    Add to Library
                  </button>
                )}
              </div>
              <nav>
                <button
                  className={tab === "relations" ? "active" : ""}
                  onClick={() => setTab("relations")}
                >
                  Relations
                </button>
                <button
                  className={tab === "source" ? "active" : ""}
                  onClick={() => setTab("source")}
                >
                  Source
                </button>
                <button onClick={() => void runImpact()}>Impact</button>
                <button
                  className={tab === "ai" ? "active" : ""}
                  onClick={() => setTab("ai")}
                >
                  AI
                </button>
              </nav>
              <div className="panel">
                {tab === "relations" && (
                  <Relations
                    details={selected}
                    graph={fullGraph!}
                    onNavigate={(id) => void navigateTo(id)}
                  />
                )}{" "}
                {tab === "source" && (
                  <Editor
                    height="100%"
                    theme="vs-dark"
                    language={(selected.node.language ?? "text").toLowerCase()}
                    value={
                      selected.source ??
                      "// Source is unavailable for this virtual node."
                    }
                    options={{
                      readOnly: true,
                      minimap: { enabled: false },
                      fontSize: 12,
                      scrollBeyondLastLine: false,
                    }}
                  />
                )}
                {tab === "impact" && (
                  <ImpactView result={impact} onNavigate={id => void navigateTo(id)} />
                )}
                {tab === "ai" && (
                  <div className="ai">
                    <div className="ai-provider">
                      <span>{providerNames[aiSettings.provider]}</span>
                      <code>{aiSettings.model}</code>
                      <button onClick={() => setAiSettingsOpen(true)}>
                        <Settings size={13} />
                        Configure
                      </button>
                    </div>
                    <textarea
                      value={question}
                      onChange={(event) => setQuestion(event.target.value)}
                    />
                    <button onClick={() => void ask()}>
                      <Sparkles size={14} />
                      Ask about node
                    </button>
                    <div className="answer">{answer}</div>
                  </div>
                )}
              </div>
            </aside>
          )}
        </section>
      )}
      {page === "map" && architectureMap && (
        <div className="quality-metrics">
          <b>{architectureMap.connected_components ?? "—"}</b> components ·{" "}
          <b>{architectureMap.isolated_nodes ?? "—"}</b> isolated
        </div>
      )}
      {page === "map" && dashboard && (
        <ProjectDashboardStrip value={dashboard} />
      )}
      {selectedFeature && (
        <FeatureDetail
          initialTab={featureView}
          feature={selectedFeature}
          spec={featureSpec}
          snippets={featureCode}
          graph={featureGraph}
          projectGraph={fullGraph}
          findings={featureFindings}
          onClose={() => setSelectedFeature(null)}
          onSave={(feature, spec) => void saveFeatureSpec(feature, spec)}
          onRename={(name) => void updateFeature(selectedFeature, { name })}
          onUpdated={(value) => {
            setSelectedFeature(value);
            setFeatures((current) =>
              current.map((item) => (item.id === value.id ? value : item)),
            );
          }}
        />
      )}{" "}
      {selectedCandidate && (
        <CandidateInspector
          candidate={selectedCandidate}
          project={fullGraph?.root ?? ""}
          onCancel={() => setSelectedCandidate(null)}
          onAdd={(candidate) => void addCandidate(candidate)}
        />
      )}{" "}
      {selectedLibrary && (
        <LibraryFunctionDetail
          entry={selectedLibrary}
          generating={documentationGenerating.has(selectedLibrary.id)}
          error={documentationErrors[selectedLibrary.id]}
          onClose={() => setSelectedLibrary(null)}
          onDocument={() => void generateDocumentation(selectedLibrary.id)}
        />
      )}{" "}
      {codeInspector && (
        <EvidenceCodeInspector
          projectId={projectId}
          details={codeInspector.details}
          finding={codeInspector.finding}
          onClose={() => setCodeInspector(null)}
        />
      )}{" "}
      {directoryPickerOpen && (
        <DirectoryPicker
          initialPath={path}
          onCancel={() => setDirectoryPickerOpen(false)}
          onSelect={(selectedPath) => {
            setPath(selectedPath);
            setDirectoryPickerOpen(false);
            void analyze(selectedPath);
          }}
        />
      )}
      {aiSettingsOpen && (
        <AiSettingsDialog
          value={aiSettings}
          onCancel={() => setAiSettingsOpen(false)}
          onSave={saveAiSettings}
        />
      )}
    </main>
  );
}

function AnalysisProgressBar({ progress }: { progress: ProgressState }) {
  const percent = progress.total
    ? Math.min(100, Math.round((progress.processed / progress.total) * 100))
    : 0;
  return (
    <div className="analysis-progress">
      <div className="progress-title">
        <strong>{progress.stage.replaceAll("_", " ")}</strong>
        <span>{percent}%</span>
      </div>
      <div className="progress-track">
        <i style={{ width: `${percent}%` }} />
      </div>
      <div className="progress-details">
        <span>
          {progress.processed.toLocaleString()} /{" "}
          {progress.total.toLocaleString()} files
        </span>
        <span>
          {progress.nodes.toLocaleString()} nodes ·{" "}
          {progress.edges.toLocaleString()} edges
        </span>
        <span>{(progress.elapsed_ms / 1000).toFixed(1)}s</span>
        <code>{progress.current_file ?? ""}</code>
      </div>
    </div>
  );
}
function ProjectDashboardStrip({ value }: { value: ProjectDashboard }) {
  return (
    <aside className="dashboard-strip">
      <b>
        Architecture{" "}
        <span>
          {value.architecture.zones} zones · {value.architecture.entry_points}{" "}
          entries
        </span>
      </b>
      <b>
        Features{" "}
        <span>
          {value.features.accepted}/{value.features.detected} accepted
        </span>
      </b>
      <b>
        My Code{" "}
        <span>
          {value.my_code.functions} funcs · {value.my_code.classes} classes
        </span>
      </b>
      <b>
        Quality{" "}
        <span>
          {value.quality.critical} critical · {value.quality.high} high
        </span>
      </b>
      <b>
        Security{" "}
        <span>
          {value.security.high} high · {value.security.medium} medium
        </span>
      </b>
      <b>
        Docs <span>{value.documentation.current ? "current" : "stale"}</span>
      </b>
      <b>
        Library{" "}
        <span>
          {value.library.functions} functions · {value.library.features}{" "}
          features
        </span>
      </b>
    </aside>
  );
}
export function FeaturesPage({
  items,
  onDetect,
  onInspect,
  onAccept,
  onIgnore,
}: {
  items: Feature[];
  onDetect: () => void;
  onInspect: (feature: Feature) => void;
  onAccept: (feature: Feature) => void;
  onIgnore: (feature: Feature) => void;
}) {
  return (
    <section className="page-view">
      <div className="page-heading">
        <div>
          <span>Intelligence fonctionnelle</span>
          <h1>Features</h1>
          <p>
            Flux métier compacts dérivés des points d’entrée et des relations
            réellement persistées.
          </p>
        </div>
        <button className="primary page-cta" onClick={onDetect}>
          <Sparkles size={14} />
          Détecter les Features
        </button>
      </div>
      <div className="feature-grid">
        {items.map((feature) => (
          <article
            key={feature.id}
            className={feature.status === "ignored" ? "muted" : ""}
          >
            <span>
              {feature.status} · {Math.round(feature.confidence * 100)}% de
              confiance · {Math.round((feature.completeness ?? 0) * 100)}%
              complète
            </span>
            <h2>{feature.name}</h2>
            <p>{feature.description}</p>
            <div className="feature-counts">
              <b>
                {feature.entry_point_node_ids.length}
                <small>Points d’entrée</small>
              </b>
              <b>
                {feature.node_ids.length}
                <small>Nœuds</small>
              </b>
              <b>
                {feature.routes.length}
                <small>Routes</small>
              </b>
              <b>
                {feature.tests.length}
                <small>Tests</small>
              </b>
            </div>
            <div className="card-actions">
              <button onClick={() => onInspect(feature)}>Inspecter</button>
              {feature.status !== "accepted" && (
                <button onClick={() => onAccept(feature)}>Accepter</button>
              )}
              <button onClick={() => onIgnore(feature)}>Ignorer</button>
            </div>
          </article>
        ))}
        {items.length === 0 && (
          <div className="empty-card">
            <Network size={34} />
            <h2>Aucune Feature détectée</h2>
            <button onClick={onDetect}>Lancer la détection déterministe</button>
          </div>
        )}
      </div>
    </section>
  );
}
export function FeatureDetail({
  feature,
  spec,
  snippets,
  graph,
  projectGraph,
  findings,
  onClose,
  onSave,
  onRename,
  onUpdated,
  initialTab = "overview",
}: {
  initialTab?: "overview" | "code" | "map";
  feature: Feature;
  spec: FeatureSpec | null;
  snippets: FeatureSnippet[];
  graph: Graph | null;
  projectGraph?: Graph | null;
  findings: Finding[];
  onClose: () => void;
  onSave: (feature: Feature, spec: FeatureSpec) => void;
  onRename: (name: string) => void;
  onUpdated?: (feature: Feature) => void;
}) {
  const [tab, setTab] = useState<
    | "overview"
    | "members"
    | "map"
    | "code"
    | "api"
    | "spec"
    | "tests"
    | "quality"
    | "security"
    | "ai"
    | "port"
  >(initialTab);
  useEffect(() => setTab(initialTab), [initialTab, feature.id]);
  const [memberNodeId, setMemberNodeId] = useState("");
  const [memberQuery, setMemberQuery] = useState("");
  const [memberHits, setMemberHits] = useState<AtlasNode[]>([]);
  const [draft, setDraft] = useState(spec ? JSON.stringify(spec, null, 2) : "");
  const [specError, setSpecError] = useState("");
  const [actionStatus, setActionStatus] = useState("");
  const [targetProfile, setTargetProfile] = useState("rust_axum");
  const [targetProjectId, setTargetProjectId] = useState("");
  const [targetProjects, setTargetProjects] = useState<ProjectSummary[]>([]);
  const [portPlan, setPortPlan] = useState<Record<string, unknown> | null>(
    null,
  );
  const [portPreview, setPortPreview] = useState<Record<
    string,
    unknown
  > | null>(null);
  const [featureDocumentation, setFeatureDocumentation] = useState<Record<
    string,
    unknown
  > | null>(null);
  const [selectedGeneratedPath, setSelectedGeneratedPath] = useState("");
  useEffect(() => {
    setDraft(spec ? JSON.stringify(spec, null, 2) : "");
  }, [spec]);
  useEffect(() => {
    void (async () => {
      const response = await fetch("/api/projects");
      if (response.ok) setTargetProjects(await response.json());
    })();
  }, []);
  useEffect(() => {
    if (memberNodeId || memberQuery.trim().length < 2) {
      setMemberHits([]);
      return;
    }
    const timer = window.setTimeout(() => {
      void (async () => {
        const response = await fetch(
          `/api/projects/${feature.project_id}/search?q=${encodeURIComponent(memberQuery)}&limit=15`,
        );
        if (response.ok) {
          const value = (await response.json()) as { node: AtlasNode }[];
          setMemberHits(
            value
              .map((item) => item.node)
              .filter((node) => !feature.node_ids.includes(node.id)),
          );
        }
      })();
    }, 180);
    return () => window.clearTimeout(timer);
  }, [feature.node_ids, feature.project_id, memberNodeId, memberQuery]);
  const flowNodes = useMemo(
    () => (graph ? layoutNodes(graph.nodes, graph.edges, null) : []),
    [graph],
  );
  const flowEdges = useMemo(
    () => (graph ? graph.edges.map((edge) => flowEdge(edge, null, null)) : []),
    [graph],
  );
  const rename = () => {
    const value = window.prompt("Feature name", feature.name)?.trim();
    if (value) onRename(value);
  };
  const save = () => {
    try {
      const value = JSON.parse(draft) as FeatureSpec;
      setSpecError("");
      onSave(feature, value);
    } catch (error) {
      setSpecError(error instanceof Error ? error.message : "Invalid JSON");
    }
  };
  async function generateSpec() {
    const settings = storedAiSettings();
    const configuration = settings.apiKey.trim()
      ? {
          provider: settings.provider,
          api_key: settings.apiKey,
          model: settings.model,
        }
      : undefined;
    setActionStatus("Generating structured FeatureSpec…");
    const response = await fetch(
      `/api/projects/${feature.project_id}/features/${encodeURIComponent(feature.id)}/spec`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ configuration }),
      },
    );
    const value = await response.json();
    if (response.ok) {
      setDraft(JSON.stringify(value, null, 2));
      setActionStatus("FeatureSpec generated and cached");
    } else setActionStatus(value.error ?? "FeatureSpec generation failed");
  }
  async function loadFeatureDocumentation() {
    const response = await fetch(
      `/api/projects/${feature.project_id}/features/${encodeURIComponent(feature.id)}/docs`,
      { method: "POST" },
    );
    const value = await response.json();
    if (response.ok) {
      setFeatureDocumentation(value);
      setActionStatus(
        "Feature documentation generated from the current FeatureSpec",
      );
    } else setActionStatus(value.error ?? "Feature documentation failed");
  }
  async function addToLibrary() {
    const response = await fetch(
      `/api/features/${encodeURIComponent(feature.id)}/library`,
      { method: "POST" },
    );
    const value = response.status === 204 ? {} : await response.json();
    setActionStatus(
      response.ok ? "Saved to Feature Library" : (value.error ?? "Save failed"),
    );
  }
  async function exportFeature() {
    const response = await fetch(
      `/api/features/${encodeURIComponent(feature.id)}/export`,
    );
    if (!response.ok) {
      setActionStatus("Feature export failed");
      return;
    }
    const bundle = await response.json();
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" }),
    );
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${feature.name.replaceAll(" ", "-").toLowerCase()}-feature-bundle.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  }
  async function merge() {
    const other = window
      .prompt("ID of the Feature to merge with this one")
      ?.trim();
    if (!other) return;
    const response = await fetch(
      `/api/projects/${feature.project_id}/features/merge`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ feature_ids: [feature.id, other] }),
      },
    );
    const value = await response.json();
    setActionStatus(
      response.ok ? `Merged as ${value.name}` : (value.error ?? "Merge failed"),
    );
  }
  async function split() {
    const raw = window.prompt(
      "Split parts: Name:node_id,node_id; Other:node_id,node_id",
    );
    if (!raw) return;
    const parts = raw.split(";").map((part) => {
      const [name, ids = ""] = part.split(":");
      return {
        name: name.trim(),
        node_ids: ids
          .split(",")
          .map((id) => id.trim())
          .filter(Boolean),
      };
    });
    const response = await fetch(
      `/api/projects/${feature.project_id}/features/${encodeURIComponent(feature.id)}/split`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ parts }),
      },
    );
    const value = await response.json();
    setActionStatus(
      response.ok
        ? `Created ${value.items.length} split Features`
        : (value.error ?? "Split failed"),
    );
  }
  async function updateMembership(
    action: "add" | "remove" | "optional" | "external" | "move",
    nodeId: string,
    extra: Record<string, unknown> = {},
  ) {
    setActionStatus("Mise à jour de l’appartenance…");
    const response = await fetch(
      `/api/projects/${feature.project_id}/features/${encodeURIComponent(feature.id)}/members`,
      {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ action, node_id: nodeId, ...extra }),
      },
    );
    const value = await response.json();
    if (response.ok) {
      onUpdated?.(value as Feature);
      setMemberNodeId("");
      setMemberQuery("");
      setActionStatus("Appartenance enregistrée et complétude recalculée");
    } else {
      setActionStatus(value.error ?? "Modification impossible");
    }
  }
  const portRequest = () => {
    const settings = storedAiSettings();
    return {
      target_profile: targetProfile,
      target_project_id: targetProjectId || undefined,
      constraints: [],
      configuration: settings.apiKey.trim()
        ? {
            provider: settings.provider,
            api_key: settings.apiKey,
            model: settings.model,
          }
        : undefined,
    };
  };
  async function preparePort() {
    setActionStatus("Preparing target-aware plan…");
    const response = await fetch(
      `/api/features/${encodeURIComponent(feature.id)}/port/plan`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(portRequest()),
      },
    );
    const value = await response.json();
    if (response.ok) {
      setPortPlan(value);
      setActionStatus("Porting plan ready");
    } else setActionStatus(value.error ?? "Plan failed");
  }
  async function generatePort() {
    const hasAi = Boolean(storedAiSettings().apiKey.trim());
    setActionStatus(
      "Génération de l’implémentation et des tests d’acceptation…",
    );
    const response = await fetch(
      `/api/features/${encodeURIComponent(feature.id)}/port/generate`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(portRequest()),
      },
    );
    const value = await response.json();
    if (response.ok) {
      setPortPreview(value);
      setPortPlan(value.plan);
      setSelectedGeneratedPath((value.files as GeneratedFile[])[0]?.path ?? "");
      setActionStatus(
        hasAi
          ? "Code IA structuré et validé — aucun fichier modifié"
          : "Gabarit déterministe généré — configure une IA pour traduire le comportement complet",
      );
    } else setActionStatus(value.error ?? "Échec de la génération");
  }
  async function applyPort() {
    if (
      !portPreview ||
      !targetProjectId ||
      !window.confirm(
        "Apply only the previewed new files to the selected target project? Existing files will not be overwritten.",
      )
    )
      return;
    const plan = portPreview.plan as { target_hash?: string };
    const response = await fetch(
      `/api/features/${encodeURIComponent(feature.id)}/port/apply`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          target_project_id: targetProjectId,
          expected_target_hash: plan.target_hash,
          preview: portPreview,
          confirm: true,
        }),
      },
    );
    const value = await response.json();
    setActionStatus(
      response.ok
        ? `Applied and reanalyzed: ${value.created.length} file(s)`
        : (value.error ?? "Apply failed"),
    );
  }
  function exportPort() {
    if (!portPreview) return;
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(portPreview, null, 2)], {
        type: "application/json",
      }),
    );
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${feature.name.replaceAll(" ", "-").toLowerCase()}-port.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  }
  const generatedFiles =
    (portPreview?.files as GeneratedFile[] | undefined) ?? [];
  const selectedGenerated =
    generatedFiles.find((file) => file.path === selectedGeneratedPath) ??
    generatedFiles[0];
  const labels = {
    overview: "Vue d’ensemble",
    members: "Membres",
    map: "Carte",
    code: "Code source",
    api: "API",
    spec: "Spécification",
    tests: "Tests",
    quality: "Qualité",
    security: "Sécurité",
    ai: "IA",
    port: "Portage",
  };
  return (
    <div className="modal-backdrop">
      <section className="modal feature-detail" role="dialog" aria-modal="true">
        <div className="modal-head">
          <div>
            <span>Détail de la Feature · {feature.status}</span>
            <h2>{feature.name}</h2>
            <code>
              {Math.round(feature.confidence * 100)}% de confiance ·{" "}
              {feature.node_ids.length} nœuds
            </code>
          </div>
          <button aria-label="Fermer" onClick={onClose}>
            <X size={18} />
          </button>
        </div>
        <nav className="feature-tabs">
          {(
            [
              "overview",
              "members",
              "map",
              "code",
              "api",
              "spec",
              "tests",
              "quality",
              "security",
              "ai",
              "port",
            ] as const
          ).map((value) => (
            <button
              className={tab === value ? "active" : ""}
              key={value}
              onClick={() => setTab(value)}
            >
              {labels[value]}
            </button>
          ))}
        </nav>
        <div className="feature-content">
          {actionStatus && (
            <p className="feature-action-status">{actionStatus}</p>
          )}
          {tab === "overview" && (
            <>
              <p>{feature.description}</p>
              <div className="feature-completeness">
                <b>{Math.round((feature.completeness ?? 0) * 100)}% complète</b>
                <progress max="1" value={feature.completeness ?? 0} />
                {(feature.missing_signals ?? []).length > 0 && (
                  <ul>
                    {(feature.missing_signals ?? []).map((signal) => (
                      <li key={signal}>{signal}</li>
                    ))}
                  </ul>
                )}
              </div>
              <div className="card-actions">
                <button onClick={rename}>Renommer</button>
                <button onClick={() => void merge()}>Fusionner</button>
                <button onClick={() => void split()}>Diviser</button>
                <button onClick={() => void exportFeature()}>
                  Exporter le bundle
                </button>
                {feature.status === "accepted" && (
                  <button onClick={() => void addToLibrary()}>
                    <BookOpen size={13} />
                    Enregistrer dans la bibliothèque
                  </button>
                )}
              </div>
              <dl>
                <div>
                  <dt>Points d’entrée</dt>
                  <dd>{feature.entry_point_node_ids.join(", ") || "Aucun"}</dd>
                </div>
                <div>
                  <dt>Routes</dt>
                  <dd>{feature.routes.join(", ") || "Aucune"}</dd>
                </div>
                <div>
                  <dt>Pages</dt>
                  <dd>{feature.pages.join(", ") || "Aucune"}</dd>
                </div>
                <div>
                  <dt>Services</dt>
                  <dd>{feature.services.join(", ") || "Aucun"}</dd>
                </div>
                <div>
                  <dt>Dépôts</dt>
                  <dd>{feature.repositories.join(", ") || "Aucun"}</dd>
                </div>
                <div>
                  <dt>Données</dt>
                  <dd>
                    {[...feature.models, ...feature.entities].join(", ") ||
                      "Aucune"}
                  </dd>
                </div>
              </dl>
            </>
          )}
          {tab === "members" && (
            <div className="feature-members">
              <div className="feature-member-add">
                <div className="feature-member-search">
                  <input
                    value={memberQuery}
                    onChange={(event) => {
                      setMemberQuery(event.target.value);
                      setMemberNodeId("");
                    }}
                    placeholder="Rechercher un symbole, fichier, service…"
                  />
                  {memberHits.length > 0 && (
                    <menu>
                      {memberHits.map((node) => (
                        <button
                          key={node.id}
                          onClick={() => {
                            setMemberNodeId(node.id);
                            setMemberQuery(
                              `${node.name} · ${node.path ?? node.kind}`,
                            );
                            setMemberHits([]);
                          }}
                        >
                          <b>{node.name}</b>
                          <span>
                            {node.kind} · {node.path ?? "virtuel"}
                          </span>
                        </button>
                      ))}
                    </menu>
                  )}
                </div>
                <button
                  disabled={!memberNodeId}
                  onClick={() => void updateMembership("add", memberNodeId)}
                >
                  Ajouter
                </button>
              </div>
              {(feature.memberships ?? []).map((membership) => {
                const node = [
                  ...(projectGraph?.nodes ?? []),
                  ...(graph?.nodes ?? []),
                ].find((candidate) => candidate.id === membership.node_id);
                return (
                  <article key={membership.node_id}>
                    <div>
                      <b>{node?.name ?? membership.node_id}</b>
                      <span>
                        {node?.kind ?? "Nœud"} ·{" "}
                        {node?.path ?? "chemin inconnu"}
                      </span>
                      <p>{membership.reason}</p>
                      <small>
                        source {membership.source} · confiance{" "}
                        {Math.round(membership.confidence * 100)}%
                        {membership.optional ? " · optionnel" : ""}
                        {membership.external_dependency ? " · externe" : ""}
                      </small>
                    </div>
                    <div className="card-actions">
                      <button
                        onClick={() =>
                          void updateMembership(
                            "optional",
                            membership.node_id,
                            {
                              optional: !membership.optional,
                            },
                          )
                        }
                      >
                        {membership.optional ? "Rendre requis" : "Optionnel"}
                      </button>
                      <button
                        onClick={() => {
                          const target = window
                            .prompt("ID de la Feature cible")
                            ?.trim();
                          if (target)
                            void updateMembership("move", membership.node_id, {
                              target_feature_id: target,
                            });
                        }}
                      >
                        Déplacer
                      </button>
                      <button
                        onClick={() =>
                          void updateMembership(
                            "external",
                            membership.node_id,
                            {
                              external_dependency: true,
                            },
                          )
                        }
                      >
                        Marquer externe
                      </button>
                      <button
                        className="danger"
                        onClick={() =>
                          void updateMembership("remove", membership.node_id)
                        }
                      >
                        Retirer
                      </button>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
          {tab === "map" && (
            <div className="feature-map">
              {graph && (
                <ReactFlow
                  nodes={flowNodes}
                  edges={flowEdges}
                  nodeTypes={nodeTypes}
                  fitView
                >
                  <Background color="#252a34" gap={24} />
                  <Controls />
                </ReactFlow>
              )}
            </div>
          )}
          {tab === "code" && (
            <FeatureSourceExplorer
              projectId={feature.project_id}
              featureId={feature.id}
              initial={snippets}
            />
          )}
          {tab === "api" && (
            <div className="feature-api-links">
              <DocumentationItems
                title="Endpoints liés à cette Feature"
                items={feature.routes}
              />
              <p>
                L’API Explorer relie chaque endpoint à son handler, son code,
                ses constats, ses tests et cette Feature.
              </p>
            </div>
          )}
          {tab === "spec" && (
            <div className="feature-spec">
              <textarea
                value={draft}
                onChange={(event) => setDraft(event.target.value)}
              />
              {specError && <p className="error">{specError}</p>}
              <button className="primary" onClick={save}>
                Enregistrer la FeatureSpec
              </button>
            </div>
          )}
          {tab === "tests" && (
            <DocumentationItems
              title="Preuves comportementales"
              items={spec?.acceptance_criteria ?? feature.tests}
            />
          )}{" "}
          {tab === "quality" && (
            <div className="feature-findings">
              {findings
                .filter((finding) => finding.category !== "security")
                .map((finding) => (
                  <article key={finding.id}>
                    <b>
                      {finding.severity} · {finding.title}
                    </b>
                    <p>{finding.description}</p>
                  </article>
                ))}
            </div>
          )}
          {tab === "security" && (
            <div className="feature-findings">
              {findings
                .filter((finding) => finding.category === "security")
                .map((finding) => (
                  <article key={finding.id}>
                    <b>
                      {finding.severity} · {finding.title}
                    </b>
                    <p>{finding.description}</p>
                  </article>
                ))}
            </div>
          )}
          {tab === "ai" && (
            <div className="feature-ai">
              <h3>Enrichissement IA fondé sur le code</h3>
              <p>
                Génère une FeatureSpec stricte depuis le sous-graphe, le code,
                les tests et les constats. Le résultat est mis en cache tant que
                la Feature ne change pas.
              </p>
              <button className="primary" onClick={() => void generateSpec()}>
                <Sparkles size={14} />
                Générer la FeatureSpec
              </button>
              <button onClick={() => void loadFeatureDocumentation()}>
                Générer la documentation
              </button>
              {featureDocumentation && (
                <KnowledgeDocument value={featureDocumentation} />
              )}
            </div>
          )}
          {tab === "port" && (
            <div className="feature-port">
              <h3>Génération multilangage</h3>
              <p>
                Le générateur produit des fichiers d’implémentation et de test à
                partir de la FeatureSpec. L’aperçu ci-dessous montre le code
                exact avant toute écriture.
              </p>
              <label>
                Stack cible
                <select
                  value={targetProfile}
                  onChange={(event) => setTargetProfile(event.target.value)}
                >
                  {[
                    ["rust_axum", "Rust + Axum"],
                    ["rust_actix", "Rust + Actix"],
                    ["type_script_next", "TypeScript + Next.js"],
                    ["type_script_express", "TypeScript + Express"],
                    ["type_script_nest", "TypeScript + NestJS"],
                    ["php_symfony", "PHP + Symfony"],
                    ["php_laravel", "PHP + Laravel"],
                    ["python_fast_api", "Python + FastAPI"],
                    ["python_django", "Python + Django"],
                    ["go_gin", "Go + Gin"],
                    ["go_echo", "Go + Echo"],
                    ["dart_flutter", "Dart + Flutter"],
                  ].map(([value, label]) => (
                    <option value={value} key={value}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Projet cible existant (facultatif)
                <select
                  value={targetProjectId}
                  onChange={(event) => setTargetProjectId(event.target.value)}
                >
                  <option value="">Export uniquement</option>
                  {targetProjects.map((project) => (
                    <option key={project.id} value={project.id}>
                      {project.name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="card-actions">
                <button onClick={() => void preparePort()}>
                  Préparer le plan
                </button>
                <button className="primary" onClick={() => void generatePort()}>
                  Générer code + tests
                </button>
                {portPreview && <button onClick={exportPort}>Exporter</button>}
                {portPreview && targetProjectId && (
                  <button onClick={() => void applyPort()}>
                    Appliquer sans écraser
                  </button>
                )}
              </div>
              {portPlan && (
                <details>
                  <summary>Plan détaillé</summary>
                  <KnowledgeDocument value={portPlan} />
                </details>
              )}
              {generatedFiles.length > 0 && (
                <div className="generated-code">
                  <aside>
                    {generatedFiles.map((file) => (
                      <button
                        className={
                          selectedGenerated?.path === file.path ? "active" : ""
                        }
                        key={file.path}
                        onClick={() => setSelectedGeneratedPath(file.path)}
                      >
                        <b>{file.is_test ? "TEST" : "CODE"}</b>
                        <span>{file.path}</span>
                      </button>
                    ))}
                  </aside>
                  <div>
                    <Editor
                      height="100%"
                      theme="vs-dark"
                      language={languageForPath(selectedGenerated?.path ?? "")}
                      value={selectedGenerated?.content ?? ""}
                      options={{
                        readOnly: true,
                        minimap: { enabled: false },
                        fontSize: 12,
                      }}
                    />
                  </div>
                </div>
              )}
              {portPreview && (
                <details>
                  <summary>Diff unifié</summary>
                  <pre>{String(portPreview.unified_diff ?? "")}</pre>
                </details>
              )}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
export function LibraryPage({
  entries,
  candidates,
  query,
  generating,
  errors,
  onQuery,
  onInspect,
  onOpen,
  onDelete,
  onDocument,
}: {
  entries: LibraryEntry[];
  candidates: LibraryCandidate[];
  query: string;
  generating: Set<string>;
  errors: Record<string, string>;
  onQuery: (value: string) => void;
  onInspect: (candidate: LibraryCandidate) => void;
  onOpen: (entry: LibraryEntry) => void;
  onDelete: (id: string) => void;
  onDocument: (id: string) => void;
}) {
  const [tab, setTab] = useState<"functions" | "features">("functions");
  const [featureEntries, setFeatureEntries] = useState<FeatureLibraryEntry[]>(
    [],
  );
  useEffect(() => {
    void (async () => {
      const response = await fetch("/api/library/features");
      if (response.ok) setFeatureEntries((await response.json()).items ?? []);
    })();
  }, []);
  async function removeFeature(id: string) {
    const response = await fetch(
      `/api/library/features/${encodeURIComponent(id)}`,
      { method: "DELETE" },
    );
    if (response.ok)
      setFeatureEntries((current) =>
        current.filter((entry) => entry.id !== id),
      );
  }
  return (
    <section className="page-view">
      <div className="page-heading">
        <div>
          <span>First-party knowledge</span>
          <h1>Library</h1>
          <p>
            Reusable functions and accepted business Features with immutable
            provenance.
          </p>
        </div>
      </div>
      <div className="library-tabs">
        <button
          className={tab === "functions" ? "active" : ""}
          onClick={() => setTab("functions")}
        >
          Functions
        </button>
        <button
          className={tab === "features" ? "active" : ""}
          onClick={() => setTab("features")}
        >
          Features
        </button>
      </div>
      {tab === "functions" ? (
        <>
          <div className="global-search">
            <Search size={18} />
            <input
              value={query}
              onChange={(event) => onQuery(event.target.value)}
              placeholder="JWT, pagination, email validation…"
            />
          </div>
          {candidates.length > 0 && (
            <>
              <h2 className="section-title">Suggested candidates</h2>
              <div className="candidate-row">
                {candidates.slice(0, 6).map((candidate) => (
                  <article key={candidate.node.id}>
                    <b>{candidate.node.name}</b>
                    <span>
                      {candidate.reuse_score}/100 reuse ·{" "}
                      {candidate.knowledge_value}/100 knowledge
                    </span>
                    <small>{candidate.reasons.join(" · ")}</small>
                    <button onClick={() => onInspect(candidate)}>
                      Inspect
                    </button>
                  </article>
                ))}
              </div>
            </>
          )}
          <h2 className="section-title">Saved functions</h2>
          <div className="library-grid">
            {entries.map((entry) => (
              <SavedFunctionCard
                key={entry.id}
                entry={entry}
                generating={generating.has(entry.id)}
                error={errors[entry.id]}
                onOpen={() => onOpen(entry)}
                onDelete={() => onDelete(entry.id)}
                onDocument={() => onDocument(entry.id)}
              />
            ))}
            {entries.length === 0 && (
              <div className="empty-card">
                <BookOpen size={35} />
                <h2>Your Function Library is empty</h2>
                <p>Inspect a suggested function before adding it.</p>
              </div>
            )}
          </div>
        </>
      ) : (
        <>
          <h2 className="section-title">Saved Features</h2>
          <div className="library-grid feature-library-grid">
            {featureEntries.map((entry) => (
              <article className="library-card" key={entry.id}>
                <span>{entry.languages.join(" · ") || "Multi-stack"}</span>
                <h2>{entry.name}</h2>
                <p>{entry.documentation}</p>
                <code>
                  {entry.source_project_id} · {entry.architecture_summary}
                </code>
                <small>
                  hash {entry.source_hash.slice(0, 12)} ·{" "}
                  {entry.acceptance_criteria.length} acceptance criteria
                </small>
                <div className="card-actions">
                  <details>
                    <summary>FeatureSpec</summary>
                    <KnowledgeDocument value={entry.spec} />
                  </details>
                  <button
                    className="danger"
                    aria-label={`Delete ${entry.name}`}
                    onClick={() => void removeFeature(entry.id)}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </article>
            ))}
            {featureEntries.length === 0 && (
              <div className="empty-card">
                <Network size={35} />
                <h2>Your Feature Library is empty</h2>
                <p>Accept a Feature, then save it from Feature Detail.</p>
              </div>
            )}
          </div>
        </>
      )}
    </section>
  );
}

export function activateSavedFunctionCard(
  event: { key: string; preventDefault: () => void },
  onOpen: () => void,
) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    onOpen();
  }
}
export function runSavedFunctionAction(
  event: { stopPropagation: () => void },
  action: () => void,
) {
  event.stopPropagation();
  action();
}
export function documentationActionLabel(generating: boolean, error?: string) {
  return generating ? "Generating…" : error ? "Retry" : "Document";
}
export function SavedFunctionCard({
  entry,
  generating,
  error,
  onOpen,
  onDelete,
  onDocument,
}: {
  entry: LibraryEntry;
  generating: boolean;
  error?: string;
  onOpen: () => void;
  onDelete: () => void;
  onDocument: () => void;
}) {
  return (
    <article
      className="library-card"
      role="button"
      tabIndex={0}
      aria-label={`Open ${entry.display_name}`}
      onClick={onOpen}
      onKeyDown={(event) => activateSavedFunctionCard(event, onOpen)}
    >
      <div className="score">{entry.reuse_score}</div>
      <span>
        {entry.language} · {entry.category} · knowledge {entry.knowledge_value}
      </span>
      <h2>{entry.display_name}</h2>
      <p>
        {entry.description ||
          entry.documentation?.summary ||
          "No description yet."}
      </p>
      <code>
        {entry.source_path}:{entry.start_line ?? "—"}
      </code>
      <small>
        {entry.source_scope} · hash {entry.source_hash.slice(0, 10)}
      </small>
      {entry.real_usages.slice(0, 2).map((usage) => (
        <blockquote key={`${usage.path}:${usage.line}`}>
          {usage.snippet ?? usage.caller}
          <cite>
            {usage.path}:{usage.line ?? "—"}
          </cite>
        </blockquote>
      ))}
      {error && (
        <div className="documentation-error" role="alert">
          <b>Documentation failed</b>
          <span>{error}</span>
        </div>
      )}
      <div className="card-actions">
        <button
          disabled={generating}
          onClick={(event) => runSavedFunctionAction(event, onDocument)}
        >
          {generating ? (
            <LoaderCircle className="spinner" size={13} />
          ) : (
            <Sparkles size={13} />
          )}{" "}
          {documentationActionLabel(generating, error)}
        </button>
        <button
          className="danger"
          aria-label={`Delete ${entry.display_name}`}
          onClick={(event) => runSavedFunctionAction(event, onDelete)}
        >
          <Trash2 size={13} />
        </button>
      </div>
    </article>
  );
}

function DocumentationItems({
  title,
  items,
}: {
  title: string;
  items: string[];
}) {
  return (
    <section>
      <h3>{title}</h3>
      {items.length ? (
        <ul>
          {items.map((item, index) => (
            <li key={`${item}-${index}`}>{item}</li>
          ))}
        </ul>
      ) : (
        <p>None documented.</p>
      )}
    </section>
  );
}

export function LibraryFunctionDetail({
  entry,
  generating,
  error,
  onClose,
  onDocument,
}: {
  entry: LibraryEntry;
  generating: boolean;
  error?: string;
  onClose: () => void;
  onDocument: () => void;
}) {
  useEffect(() => {
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [onClose]);
  const documentation = entry.documentation;
  const facts = [
    ["Project", entry.source_project_id],
    ["Path", entry.source_path],
    ["Lines", `${entry.start_line ?? "—"}–${entry.end_line ?? "—"}`],
    ["Language", entry.language],
    ["Source hash", entry.source_hash],
    ["Created at", new Date(entry.created_at * 1000).toLocaleString()],
    ["Updated at", new Date(entry.updated_at * 1000).toLocaleString()],
    ["Git repo", entry.source_repository ?? "Unavailable"],
    ["Branch", entry.source_branch ?? "Unavailable"],
    ["Commit", entry.source_version ?? "Unavailable"],
  ];
  return (
    <div className="modal-backdrop">
      <section
        className="modal library-detail"
        role="dialog"
        aria-modal="true"
        aria-labelledby="library-detail-title"
      >
        <div className="modal-head">
          <div>
            <span>Library Function Detail</span>
            <h2 id="library-detail-title">{entry.display_name}</h2>
            <code>
              {entry.language} · {entry.source_path} · L
              {entry.start_line ?? "—"}–{entry.end_line ?? "—"}
            </code>
          </div>
          <button aria-label="Close function detail" onClick={onClose}>
            <X size={18} />
          </button>
        </div>
        <div className="library-detail-content">
          <div className="library-detail-scores">
            <b>
              Reuse Score <span>{entry.reuse_score}/100</span>
            </b>
            <b>
              Knowledge Value <span>{entry.knowledge_value}/100</span>
            </b>
            <b>
              Category <span>{entry.category}</span>
            </b>
            <b>
              Tags <span>{entry.tags.join(", ") || "None"}</span>
            </b>
          </div>
          <section>
            <h3>Source code</h3>
            <div className="candidate-source">
              <Editor
                height="340px"
                theme="vs-dark"
                language={entry.language.toLowerCase()}
                value={entry.source_code || "// Saved source is unavailable."}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  fontSize: 12,
                  scrollBeyondLastLine: false,
                }}
              />
            </div>
          </section>
          <section>
            <h3>Provenance</h3>
            <dl>
              {facts.map(([label, value]) => (
                <div key={label}>
                  <dt>{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
          </section>
          <section>
            <h3>Graph context</h3>
            <dl>
              <div>
                <dt>Used by</dt>
                <dd>
                  {entry.real_usages.map((usage) => usage.caller).join(", ") ||
                    "None detected"}
                </dd>
              </div>
              <div>
                <dt>Calls</dt>
                <dd>{entry.calls.join(", ") || "None detected"}</dd>
              </div>
              <div>
                <dt>Dependencies</dt>
                <dd>{entry.dependencies.join(", ") || "None detected"}</dd>
              </div>
              <div>
                <dt>Tests</dt>
                <dd>
                  {entry.tests
                    .map((test) => `${test.path}:${test.line ?? "—"}`)
                    .join(", ") || "None detected"}
                </dd>
              </div>
            </dl>
          </section>
          <section className="library-documentation">
            <div className="documentation-head">
              <div>
                <h3>AI documentation</h3>
                {entry.documentation_provider && (
                  <small>
                    {entry.documentation_provider} · {entry.documentation_model}
                    {entry.documentation_generated_at
                      ? ` · ${new Date(entry.documentation_generated_at * 1000).toLocaleString()}`
                      : ""}
                  </small>
                )}
              </div>
              <button
                className="primary"
                disabled={generating}
                onClick={onDocument}
              >
                {generating ? (
                  <LoaderCircle className="spinner" size={14} />
                ) : (
                  <Sparkles size={14} />
                )}{" "}
                {generating
                  ? "Generating…"
                  : error
                    ? "Retry"
                    : "Generate documentation"}
              </button>
            </div>
            {error && (
              <div className="documentation-error" role="alert">
                <b>Documentation generation failed.</b>
                <span>Your saved function and source are unchanged.</span>
                <details>
                  <summary>Technical details</summary>
                  <code>{error}</code>
                </details>
              </div>
            )}
            {documentation ? (
              <div className="documentation-grid">
                <section>
                  <h3>Purpose</h3>
                  <p>{documentation.purpose}</p>
                </section>
                <section>
                  <h3>Description</h3>
                  <p>{documentation.summary}</p>
                </section>
                <DocumentationItems
                  title="Use cases"
                  items={documentation.use_cases}
                />
                <section>
                  <h3>Parameters</h3>
                  {documentation.parameters.length ? (
                    <ul>
                      {documentation.parameters.map((parameter) => (
                        <li key={parameter.name}>
                          <code>
                            {parameter.name}
                            {parameter.type ? `: ${parameter.type}` : ""}
                          </code>{" "}
                          — {parameter.description}
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p>None documented.</p>
                  )}
                </section>
                <section>
                  <h3>Returns</h3>
                  <p>
                    {documentation.returns.type && (
                      <code>{documentation.returns.type} — </code>
                    )}
                    {documentation.returns.description}
                  </p>
                </section>
                <DocumentationItems
                  title="Errors"
                  items={documentation.errors}
                />
                <DocumentationItems
                  title="Side effects"
                  items={documentation.side_effects}
                />
                <DocumentationItems
                  title="Limitations"
                  items={documentation.limitations}
                />
                <DocumentationItems
                  title="Security notes"
                  items={documentation.security_notes}
                />
              </div>
            ) : (
              <p className="no-documentation">No AI documentation yet.</p>
            )}
          </section>
        </div>
      </section>
    </div>
  );
}
function CandidateInspector({
  candidate,
  project,
  onCancel,
  onAdd,
}: {
  candidate: LibraryCandidate;
  project: string;
  onCancel: () => void;
  onAdd: (candidate: LibraryCandidate) => void;
}) {
  const facts = [
    ["Project", project || "Unknown"],
    ["Path", candidate.node.path ?? "Unknown"],
    ["Language", candidate.node.language ?? "Unknown"],
    ["Signature", candidate.signature || candidate.node.name],
    ["Returns", candidate.return_type ?? "Not declared"],
    ["Size", `${candidate.loc} LOC · complexity ${candidate.complexity}`],
    ["Dependencies", candidate.dependencies.join(", ") || "None detected"],
    ["Calls", candidate.calls.join(", ") || "None detected"],
    ["Side effects", candidate.side_effects.join(", ") || "None detected"],
    ["Tests", candidate.tests.length.toString()],
    ["Incoming", candidate.real_usages.length.toString()],
  ];
  return (
    <div className="modal-backdrop">
      <section
        className="modal candidate-inspector"
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-head">
          <div>
            <span>Candidate inspection</span>
            <h2>{candidate.node.name}</h2>
          </div>
          <button onClick={onCancel}>
            <X size={18} />
          </button>
        </div>
        <div className="candidate-content">
          <div className="candidate-scores">
            <ScoreDetails
              title="Reuse"
              score={candidate.reuse_score}
              factors={candidate.reuse_breakdown}
            />
            <ScoreDetails
              title="Knowledge"
              score={candidate.knowledge_value}
              factors={candidate.knowledge_breakdown}
            />
          </div>
          <dl>
            {facts.map(([label, value]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
          {candidate.real_usages.length > 0 && (
            <div className="candidate-examples">
              <h3>Real examples</h3>
              {candidate.real_usages.slice(0, 5).map((usage) => (
                <blockquote key={`${usage.path}:${usage.line}`}>
                  {usage.snippet ?? usage.caller}
                  <cite>
                    {usage.path}:{usage.line ?? "—"}
                  </cite>
                </blockquote>
              ))}
            </div>
          )}
          <div className="candidate-source">
            <Editor
              height="340px"
              theme="vs-dark"
              language={(candidate.node.language ?? "text").toLowerCase()}
              value={candidate.source}
              options={{
                readOnly: true,
                minimap: { enabled: false },
                fontSize: 12,
                scrollBeyondLastLine: false,
              }}
            />
          </div>
        </div>
        <div className="modal-actions">
          <button className="secondary" onClick={onCancel}>
            Cancel
          </button>
          <button className="primary" onClick={() => onAdd(candidate)}>
            <BookOpen size={14} />
            Add to Library
          </button>
        </div>
      </section>
    </div>
  );
}
function ScoreDetails({
  title,
  score,
  factors,
}: {
  title: string;
  score: number;
  factors: ScoreFactor[];
}) {
  return (
    <section>
      <div className="score-head">
        <b>{title}</b>
        <strong>{score}/100</strong>
      </div>
      {factors.map((factor, index) => (
        <div key={`${factor.label}-${index}`}>
          <span>{factor.label}</span>
          <em className={factor.points < 0 ? "negative" : ""}>
            {factor.points > 0 ? "+" : ""}
            {factor.points}
          </em>
        </div>
      ))}
    </section>
  );
}
function DirectoryPicker({
  initialPath,
  onCancel,
  onSelect,
}: {
  initialPath: string;
  onCancel: () => void;
  onSelect: (path: string) => void;
}) {
  const [listing, setListing] = useState<DirectoryListing | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  async function browse(target?: string, fallback = true) {
    setLoading(true);
    setError("");
    const params = new URLSearchParams();
    if (target) params.set("path", target);
    const response = await fetch(
      `/api/fs/directories${params.size ? `?${params}` : ""}`,
    );
    const value = await response.json();
    if (!response.ok) {
      if (target && fallback) {
        await browse(undefined, false);
        return;
      }
      setError(value.error ?? "Unable to open this folder.");
      setLoading(false);
      return;
    }
    setListing(value);
    setLoading(false);
  }
  useEffect(() => {
    void browse(initialPath || undefined);
  }, []);
  return (
    <div className="modal-backdrop">
      <section
        className="modal directory-modal"
        role="dialog"
        aria-modal="true"
      >
        <div className="modal-head">
          <div>
            <span>Project source</span>
            <h2>Choose a folder</h2>
          </div>
          <button onClick={onCancel}>
            <X size={18} />
          </button>
        </div>
        <div className="directory-path">
          <FolderOpen size={15} />
          <code>{listing?.current ?? "Loading…"}</code>
        </div>
        <div className="directory-list">
          {listing?.parent && (
            <button
              className="directory-row parent"
              onClick={() => void browse(listing.parent)}
            >
              <ArrowUp size={16} />
              <span>Parent folder</span>
            </button>
          )}
          {loading && <div className="modal-message">Loading folders…</div>}
          {error && <div className="modal-message error">{error}</div>}
          {!loading &&
            listing?.directories.map((directory) => (
              <button
                className="directory-row"
                key={directory.path}
                onClick={() => void browse(directory.path)}
              >
                <Folder size={16} />
                <span>{directory.name}</span>
                <ChevronRight size={15} />
              </button>
            ))}
        </div>
        <div className="modal-actions">
          <button className="secondary" onClick={onCancel}>
            Cancel
          </button>
          <button
            className="primary"
            disabled={!listing}
            onClick={() => listing && onSelect(listing.current)}
          >
            <FolderOpen size={15} />
            Analyze this folder
          </button>
        </div>
      </section>
    </div>
  );
}
function AiSettingsDialog({
  value,
  onCancel,
  onSave,
}: {
  value: AiSettings;
  onCancel: () => void;
  onSave: (value: AiSettings) => void;
}) {
  const [draft, setDraft] = useState(value);
  const [showKey, setShowKey] = useState(false);
  return (
    <div className="modal-backdrop">
      <section className="modal settings-modal" role="dialog" aria-modal="true">
        <div className="modal-head">
          <div>
            <span>Local configuration</span>
            <h2>AI provider</h2>
          </div>
          <button onClick={onCancel}>
            <X size={18} />
          </button>
        </div>
        <form
          id="ai-settings-form"
          className="settings-form"
          onSubmit={(event) => {
            event.preventDefault();
            if (draft.apiKey.trim() && draft.model.trim()) onSave(draft);
          }}
        >
          <label>
            Provider
            <select
              value={draft.provider}
              onChange={(event) => {
                const provider = event.target.value as AiProvider;
                setDraft((current) => ({
                  ...current,
                  provider,
                  model: defaultModels[provider],
                }));
              }}
            >
              <option value="openai">OpenAI</option>
              <option value="mistral">Mistral</option>
              <option value="openrouter">OpenRouter</option>
            </select>
          </label>
          <label>
            Model
            <input
              value={draft.model}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  model: event.target.value,
                }))
              }
            />
          </label>
          <label>
            API key
            <div className="secret-input">
              <input
                type={showKey ? "text" : "password"}
                value={draft.apiKey}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    apiKey: event.target.value,
                  }))
                }
                autoComplete="off"
              />
              <button
                type="button"
                onClick={() => setShowKey((value) => !value)}
              >
                {showKey ? <EyeOff size={15} /> : <Eye size={15} />}
              </button>
            </div>
          </label>
          <p>Saved only in this browser. Never persisted in SQLite.</p>
        </form>
        <div className="modal-actions">
          <button className="secondary" onClick={onCancel}>
            Cancel
          </button>
          <button
            className="primary"
            type="submit"
            form="ai-settings-form"
            disabled={!draft.apiKey.trim() || !draft.model.trim()}
          >
            <Save size={15} />
            Save locally
          </button>
        </div>
      </section>
    </div>
  );
}
function Relations({
  details,
  graph,
  onNavigate,
}: {
  details: Details;
  graph: Graph;
  onNavigate: (id: string) => void;
}) {
  const byId = new Map(graph.nodes.map((node) => [node.id, node]));
  return (
    <div className="relations">
      <h3>
        Incoming <span>{details.incoming.length}</span>
      </h3>
      {details.incoming.map((edge) => (
        <Relation
          key={edge.id}
          edge={edge}
          node={byId.get(edge.source_id)}
          onNavigate={onNavigate}
        />
      ))}
      <h3>
        Outgoing <span>{details.outgoing.length}</span>
      </h3>
      {details.outgoing.map((edge) => (
        <Relation
          key={edge.id}
          edge={edge}
          node={byId.get(edge.target_id)}
          onNavigate={onNavigate}
        />
      ))}
    </div>
  );
}
function Relation({
  edge,
  node,
  onNavigate,
}: {
  edge: AtlasEdge;
  node?: AtlasNode;
  onNavigate: (id: string) => void;
}) {
  return (
    <button className="relation" onClick={() => node && onNavigate(node.id)}>
      <span style={{ color: color[node?.kind ?? ""] }}>
        {node?.kind ?? "Node"}
      </span>
      <b>{node?.name ?? "Unknown"}</b>
      <em>{edge.relation}</em>
      <ChevronRight size={13} />
    </button>
  );
}
