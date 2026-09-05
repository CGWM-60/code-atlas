import { useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { ChevronLeft, ChevronRight, X } from "lucide-react";
import type { AtlasEdge, AtlasNode } from "./graphView";

export type SourceSpan = {
  path: string;
  start_line: number;
  start_column: number;
  end_line: number;
  end_column: number;
  snippet?: string;
  label?: string;
  role?: string;
};
export type EvidenceFinding = {
  id: string;
  severity: string;
  title: string;
  description: string;
  primary_span?: SourceSpan;
  evidence_spans?: SourceSpan[];
};
export type SourceDetails = {
  node: AtlasNode;
  incoming: AtlasEdge[];
  outgoing: AtlasEdge[];
  source?: string;
};

export function EvidenceCodeInspector({
  details,
  finding,
  onClose,
}: {
  details: SourceDetails;
  finding?: EvidenceFinding;
  onClose: () => void;
}) {
  const [index, setIndex] = useState(0);
  const spans = finding?.evidence_spans?.length
    ? finding.evidence_spans
    : finding?.primary_span
      ? [finding.primary_span]
      : [];
  const selected = spans[index];
  const offset = (details.node.start_line ?? 1) - 1;
  const relative = (line: number) => Math.max(1, line - offset);
  const onMount: OnMount = (editor, monaco) => {
    if (!selected) return;
    const start = relative(selected.start_line);
    const end = relative(selected.end_line);
    editor.createDecorationsCollection([
      {
        range: new monaco.Range(
          start,
          selected.start_column,
          end,
          Math.max(selected.end_column, selected.start_column + 1),
        ),
        options: {
          isWholeLine: selected.start_column === 1 && selected.end_column <= 1,
          className: "finding-highlight " + (finding?.severity ?? "medium"),
          glyphMarginClassName:
            "finding-glyph " + (finding?.severity ?? "medium"),
          glyphMarginHoverMessage: {
            value:
              selected.label ?? finding?.description ?? "Preuve du Finding",
          },
          hoverMessage: {
            value:
              "**" +
              (selected.label ?? finding?.title ?? "Preuve") +
              "**\n\n" +
              (finding?.description ?? ""),
          },
        },
      },
    ]);
    editor.revealLineInCenter(start);
  };
  return (
    <div className="modal-backdrop">
      <section className="modal code-inspector" role="dialog" aria-modal="true">
        <div className="modal-head">
          <div>
            <span>
              {finding
                ? "Preuve exacte du Finding"
                : "Inspection du code source"}
            </span>
            <h2>{finding?.title ?? details.node.name}</h2>
            <code>
              {selected?.path ?? details.node.path ?? "Nœud virtuel"} · L
              {selected?.start_line ?? details.node.start_line ?? "—"}–
              {selected?.end_line ?? details.node.end_line ?? "—"}
            </code>
          </div>
          <button aria-label="Fermer" onClick={onClose}>
            <X size={18} />
          </button>
        </div>
        {finding && (
          <div className={"evidence-banner " + finding.severity}>
            <b>{finding.severity.toUpperCase()}</b>
            <p>{finding.description}</p>
            {spans.length > 0 && (
              <nav>
                <button
                  disabled={index === 0}
                  onClick={() => setIndex((value) => value - 1)}
                >
                  <ChevronLeft size={13} />
                  Preuve précédente
                </button>
                <span>
                  {index + 1}/{spans.length} · {selected?.role ?? "preuve"} ·{" "}
                  {selected?.label ?? ""}
                </span>
                <button
                  disabled={index >= spans.length - 1}
                  onClick={() => setIndex((value) => value + 1)}
                >
                  Preuve suivante
                  <ChevronRight size={13} />
                </button>
              </nav>
            )}
          </div>
        )}
        <div className="code-inspector-layout">
          <div className="source-editor">
            <Editor
              key={(finding?.id ?? "source") + ":" + index}
              height="100%"
              theme="vs-dark"
              language={(details.node.language ?? "text").toLowerCase()}
              value={
                details.source ??
                "// Aucun code source disponible pour ce nœud."
              }
              onMount={onMount}
              options={{
                readOnly: true,
                minimap: { enabled: false },
                fontSize: 13,
                scrollBeyondLastLine: false,
                glyphMargin: true,
                lineNumbers: (value) => String(offset + Number(value)),
              }}
            />
          </div>
          <aside>
            <h3>Contexte du graphe</h3>
            <b>
              {details.incoming.length}
              <small> relations entrantes</small>
            </b>
            <b>
              {details.outgoing.length}
              <small> relations sortantes</small>
            </b>
            {selected?.snippet && <blockquote>{selected.snippet}</blockquote>}
            <p>
              La décoration colore uniquement la preuve persistée, jamais toute
              la fonction par défaut.
            </p>
          </aside>
        </div>
      </section>
    </div>
  );
}
