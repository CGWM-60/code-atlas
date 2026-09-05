import type { UiAction } from "../features/assistant/types";
export type ActionHandlers = {
 navigate: (page: string) => Promise<void>; node: (id: string, mode: "source" | "focus" | "impact", range?: [number, number], sourceHash?: string) => Promise<void>;
 file: (path: string) => Promise<void>; feature: (id: string, view: "overview" | "source" | "graph") => Promise<void>;
 finding: (id: string) => Promise<void>; flow: (id: string) => void; api: (id?: string) => Promise<void>;
 library: (id: string) => Promise<void>; fit: () => void; filter: (kinds: string[]) => void;
 diff: (base?: string | null, head?: string | null) => void;
 tests: (feature: string | null) => void; estimate: (task: string) => void;
};
export async function dispatchUiAction(action: UiAction, handlers: ActionHandlers): Promise<void> {
 switch (action.type) {
  case "NAVIGATE_PAGE": return handlers.navigate(action.page);
  case "OPEN_NODE": return handlers.node(action.node_id, "source");
  case "OPEN_SOURCE_RANGE": case "HIGHLIGHT_SOURCE_RANGE":
   if (action.start_line < 1 || action.end_line < action.start_line) throw new Error("Plage source invalide");
   return action.source_hash ? handlers.node(action.node_id, "source", [action.start_line, action.end_line], action.source_hash) : handlers.node(action.node_id, "source", [action.start_line, action.end_line]);
  case "SELECT_NODE": case "FOCUS_NODE": case "FOCUS_GRAPH": case "SHOW_RELATED_NODES": return handlers.node(action.node_id, "focus");
  case "OPEN_IMPACT": return handlers.node(action.node_id, "impact");
  case "OPEN_FILE": return handlers.file(action.path);
  case "OPEN_FEATURE": return handlers.feature(action.feature_id, "overview");
  case "OPEN_FEATURE_SOURCE": return handlers.feature(action.feature_id, "source");
  case "OPEN_FEATURE_GRAPH": return handlers.feature(action.feature_id, "graph");
  case "OPEN_FINDING": return handlers.finding(action.finding_id);
  case "OPEN_SECURITY": return handlers.navigate("security");
  case "OPEN_QUALITY": return handlers.navigate("quality");
  case "OPEN_DOCUMENTATION": return handlers.navigate("docs");
  case "OPEN_FLOW": case "TRACE_FLOW": return handlers.flow(action.node_id);
  case "OPEN_API": return handlers.api();
  case "OPEN_API_ENDPOINT": return handlers.api(action.endpoint_id);
  case "OPEN_LIBRARY_ENTRY": return handlers.library(action.entry_id);
  case "FIT_GRAPH": return handlers.fit();
  case "FILTER_GRAPH": return handlers.filter(action.kinds);
  case "SHOW_DIFF": return handlers.diff(action.base, action.head);
  case "SHOW_TEST_PLAN": return handlers.tests(action.feature_id);
  case "SHOW_ESTIMATE": return handlers.estimate(action.task);
  default: throw new Error("Action UI inconnue");
 }
}
