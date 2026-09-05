import { useEffect, useMemo, useState } from "react";
import {
  Background,
  Controls,
  Handle,
  Position,
  ReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import { ArrowLeft, ArrowRight, GitBranch, Search } from "lucide-react";
import {
  flowEdge,
  layoutNodes,
  type AtlasEdge,
  type AtlasNode,
  type FlowNodeData,
} from "./graphView";

type SearchHit = { node: AtlasNode };
type FlowStep = {
  index: number;
  node: AtlasNode;
  relation_from_previous?: string;
  cumulative_cost: number;
};
type FlowTrace = {
  found: boolean;
  direction: string;
  start_node_id: string;
  end_node_id?: string;
  nodes: AtlasNode[];
  edges: AtlasEdge[];
  steps: FlowStep[];
  total_cost: number;
  message: string;
};
type Direction = "downstream" | "upstream" | "between";
function FlowCard({ data }: NodeProps<Node<FlowNodeData>>) {
  return (
    <div className="atlas-node flow-node">
      <Handle type="target" position={Position.Left} />
      <span className="node-kind">{data.kind}</span>
      <strong>{data.name}</strong>
      {data.path && <small>{data.path}</small>}
      <Handle type="source" position={Position.Right} />
    </div>
  );
}
const flowNodeTypes = { atlas: FlowCard };

export function FlowPage({
  projectId,
  seed,
  onInspect,
}: {
  projectId: string;
  seed?: string;
  onInspect: (id: string) => void;
}) {
  const [start, setStart] = useState(seed ?? "");
  const [end, setEnd] = useState("");
  const [startQuery, setStartQuery] = useState("");
  const [endQuery, setEndQuery] = useState("");
  const [startHits, setStartHits] = useState<AtlasNode[]>([]);
  const [endHits, setEndHits] = useState<AtlasNode[]>([]);
  const [direction, setDirection] = useState<Direction>("downstream");
  const [depth, setDepth] = useState(8);
  const [result, setResult] = useState<FlowTrace | null>(null);
  const [history, setHistory] = useState<FlowTrace[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [status, setStatus] = useState("");
  const relations = [
    "CALLS",
    "HANDLED_BY",
    "ROUTES_TO",
    "USES",
    "READS",
    "WRITES",
    "RENDERS",
    "EMITS",
    "LISTENS",
  ];
  const [selectedRelations, setSelectedRelations] = useState(
    new Set(relations),
  );
  useEffect(() => {
    if (seed) setStart(seed);
  }, [seed]);
  useEffect(() => {
    if (start) {
      setStartHits([]);
      return;
    }
    const timer = window.setTimeout(
      () => void search(startQuery, setStartHits),
      180,
    );
    return () => window.clearTimeout(timer);
  }, [start, startQuery, projectId]);
  useEffect(() => {
    if (end) {
      setEndHits([]);
      return;
    }
    const timer = window.setTimeout(
      () => void search(endQuery, setEndHits),
      180,
    );
    return () => window.clearTimeout(timer);
  }, [end, endQuery, projectId]);
  async function search(query: string, setter: (nodes: AtlasNode[]) => void) {
    if (!query.trim()) {
      setter([]);
      return;
    }
    const response = await fetch(
      "/api/projects/" +
        projectId +
        "/search?q=" +
        encodeURIComponent(query) +
        "&limit=12",
    );
    if (response.ok)
      setter(((await response.json()) as SearchHit[]).map((hit) => hit.node));
  }
  async function trace() {
    if (!start) {
      setStatus("Sélectionne un nœud de départ.");
      return;
    }
    if (direction === "between" && !end) {
      setStatus("Sélectionne un nœud d’arrivée.");
      return;
    }
    setStatus("Calcul du chemin pondéré…");
    const response = await fetch("/api/projects/" + projectId + "/flow/trace", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        start_node_id: start,
        end_node_id: direction === "between" ? end : undefined,
        direction,
        max_depth: depth,
        relations: [...selectedRelations],
      }),
    });
    const value = (await response.json()) as FlowTrace;
    if (response.ok) {
      setResult(value);
      setStatus(value.message);
      setHistory((current) => [...current.slice(0, historyIndex + 1), value]);
      setHistoryIndex((index) => index + 1);
    } else
      setStatus(
        (value as unknown as { error?: string }).error ?? "Échec du tracé",
      );
  }
  function navigateHistory(index: number) {
    const value = history[index];
    if (!value) return;
    setHistoryIndex(index);
    setResult(value);
    setStatus(value.message);
  }
  const nodes = useMemo<Node<FlowNodeData>[]>(
    () => (result ? layoutNodes(result.nodes, result.edges, null) : []),
    [result],
  );
  const edges = useMemo<Edge[]>(
    () =>
      result ? result.edges.map((edge) => flowEdge(edge, null, null)) : [],
    [result],
  );
  return (
    <section className="page-view flow-page">
      <div className="page-heading">
        <div>
          <span>Project Map · mode chemin</span>
          <h1>Flow Explorer</h1>
          <p>
            Trace un flux descendant, remonte ses appelants ou calcule le chemin
            sémantique le plus court entre deux éléments.
          </p>
        </div>
        <div className="flow-history">
          <button
            disabled={historyIndex <= 0}
            onClick={() => navigateHistory(historyIndex - 1)}
          >
            <ArrowLeft size={14} />
            Retour
          </button>
          <button
            disabled={historyIndex < 0 || historyIndex >= history.length - 1}
            onClick={() => navigateHistory(historyIndex + 1)}
          >
            Suivant
            <ArrowRight size={14} />
          </button>
        </div>
      </div>
      <div className="flow-builder">
        <NodePicker
          label="Départ"
          query={startQuery}
          onQuery={(value) => {
            setStartQuery(value);
            setStart("");
          }}
          hits={startHits}
          selected={start}
          onSelect={(node) => {
            setStart(node.id);
            setStartQuery(node.name);
            setStartHits([]);
          }}
        />
        <label>
          Direction
          <select
            value={direction}
            onChange={(event) => setDirection(event.target.value as Direction)}
          >
            <option value="downstream">Descendant</option>
            <option value="upstream">Ascendant</option>
            <option value="between">Entre A et B</option>
          </select>
        </label>
        {direction === "between" && (
          <NodePicker
            label="Arrivée"
            query={endQuery}
            onQuery={(value) => {
              setEndQuery(value);
              setEnd("");
            }}
            hits={endHits}
            selected={end}
            onSelect={(node) => {
              setEnd(node.id);
              setEndQuery(node.name);
              setEndHits([]);
            }}
          />
        )}
        <label>
          Profondeur
          <input
            type="number"
            min={1}
            max={20}
            value={depth}
            onChange={(event) => setDepth(Number(event.target.value))}
          />
        </label>
        <button className="primary" onClick={() => void trace()}>
          <GitBranch size={14} />
          Tracer
        </button>
      </div>
      <div className="flow-relations">
        {relations.map((relation) => (
          <label key={relation}>
            <input
              type="checkbox"
              checked={selectedRelations.has(relation)}
              onChange={() =>
                setSelectedRelations((current) => {
                  const next = new Set(current);
                  next.has(relation)
                    ? next.delete(relation)
                    : next.add(relation);
                  return next;
                })
              }
            />
            {relation}
          </label>
        ))}
      </div>
      {status && <p className="feature-action-status">{status}</p>}
      <div className="flow-result">
        {result?.found ? (
          <>
            <aside>
              <h2>Étapes · coût {result.total_cost}</h2>
              {result.steps.map((step) => (
                <button
                  key={step.node.id}
                  onClick={() => onInspect(step.node.id)}
                >
                  <b>
                    {step.index}. {step.node.name}
                  </b>
                  <span>
                    {step.relation_from_previous ?? "START"} · {step.node.kind}
                  </span>
                  <code>{step.node.path ?? "virtuel"}</code>
                </button>
              ))}
            </aside>
            <div className="flow-canvas">
              <ReactFlow
                nodes={nodes}
                edges={edges}
                nodeTypes={flowNodeTypes}
                fitView
                onNodeClick={(_, node) => onInspect(node.id)}
              >
                <Background color="#252a34" gap={28} />
                <Controls />
              </ReactFlow>
            </div>
          </>
        ) : (
          <div className="empty-card">
            <GitBranch size={34} />
            <h2>Aucun chemin affiché</h2>
            <p>
              Choisis un point de départ puis lance le tracé. Le résultat reste
              limité aux étapes significatives.
            </p>
          </div>
        )}
      </div>
    </section>
  );
}

function NodePicker({
  label,
  query,
  onQuery,
  hits,
  selected,
  onSelect,
}: {
  label: string;
  query: string;
  onQuery: (value: string) => void;
  hits: AtlasNode[];
  selected: string;
  onSelect: (node: AtlasNode) => void;
}) {
  return (
    <label className="flow-node-picker">
      {label}
      <div>
        <Search size={13} />
        <input
          value={query}
          onChange={(event) => onQuery(event.target.value)}
          placeholder="Route, page, service, table…"
        />
      </div>
      {hits.length > 0 && (
        <menu>
          {hits.map((node) => (
            <button type="button" key={node.id} onClick={() => onSelect(node)}>
              <b>{node.name}</b>
              <span>
                {node.kind} · {node.path ?? "virtuel"}
              </span>
            </button>
          ))}
        </menu>
      )}
      {selected && <small>{selected}</small>}
    </label>
  );
}
