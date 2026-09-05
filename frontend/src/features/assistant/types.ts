import type { AtlasNode } from "../../graphView";
export type UiAction =
 | { type: "NAVIGATE_PAGE"; page: string }
 | { type: "OPEN_NODE" | "FOCUS_NODE" | "SELECT_NODE" | "OPEN_FLOW" | "TRACE_FLOW" | "OPEN_IMPACT" | "FOCUS_GRAPH" | "SHOW_RELATED_NODES"; node_id: string }
 | { type: "OPEN_SOURCE_RANGE" | "HIGHLIGHT_SOURCE_RANGE"; node_id: string; start_line: number; end_line: number; source_hash?: string }
 | { type: "OPEN_FILE"; path: string }
 | { type: "OPEN_FEATURE" | "OPEN_FEATURE_SOURCE" | "OPEN_FEATURE_GRAPH"; feature_id: string }
 | { type: "OPEN_FINDING"; finding_id: string }
 | { type: "OPEN_API_ENDPOINT"; endpoint_id: string }
 | { type: "OPEN_LIBRARY_ENTRY"; entry_id: string }
 | { type: "FILTER_GRAPH"; kinds: string[] }
 | { type: "SHOW_TEST_PLAN"; feature_id: string | null }
 | { type: "SHOW_ESTIMATE"; task: string }
 | { type: "OPEN_SECURITY" | "OPEN_QUALITY" | "OPEN_API" | "OPEN_DOCUMENTATION" | "FIT_GRAPH" | "SHOW_DIFF" };
export type Citation = { node_id: string; path: string; symbol: string; start_line: number; end_line: number; source_hash: string; source: string; tool: string };
export type AssistantResponse = { answer: string; citations: Citation[]; entities: AtlasNode[]; actions: UiAction[]; suggested_followups: string[]; tool_calls: { tool: string; status: string; summary: string }[]; specialists: string[]; uncertain: boolean; mode: string; context_tokens: number };
export type Conversation = { id: string; project_id: string; title: string; updated_at: number };
export type Message = { id: number; role: "user" | "assistant"; content: AssistantResponse & { text?: string } };
export type UiContext = { active_page: string; selected_node?: string; selected_feature?: string; selected_file?: string; selected_finding?: string; selected_range?: [number, number]; current_flow?: string; current_diff?: string };
