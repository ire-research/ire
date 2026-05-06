export type ChatMode = "brainstorm" | "experiment";

export type ExperimentStatus = "starting" | "running" | "completed" | "failed" | "cancelled";

export interface UserMessage {
  id: string;
  role: "user";
  text: string;
}

export interface ToolCallState {
  tool_id: string;
  tool_name: string;
  input_preview: string | null;
  output_preview: string | null;
  output_full: string | null;
  isDone: boolean;
  experimentUuid?: string;
  experimentStatus?: ExperimentStatus;
  experimentExitCode?: number;
  logLines?: string[];
}

export interface AssistantMessage {
  id: string;
  role: "assistant";
  text: string;
  thinking?: string;
  isStreaming: boolean;
  error?: string;
  tools?: ToolCallState[];
}

export type ChatMessage = UserMessage | AssistantMessage;

export type StreamEvent =
  | { kind: "Init"; session_id: string }
  | { kind: "TextDelta"; text: string }
  | { kind: "ThinkingDelta"; text: string }
  | { kind: "ToolStart"; tool_id: string; tool_name: string; input_preview: string | null }
  | { kind: "ToolDone"; tool_id: string; output_preview: string | null; output_full: string | null }
  | { kind: "Result"; text: string | null; session_id: string }
  | { kind: "Error"; message: string }
  | { kind: "Done" };

export interface WikiFile {
  content: string;
  frontmatter: Record<string, string> | null;
}

export type TabKind = "chat" | "resource" | "preview";
export type ResourceStatus = "summarizing" | "ready" | "confirmed";

export interface Tab {
  id: string;
  label: string;
  messages: ChatMessage[];
  isStreaming: boolean;
  isPinned: boolean;
  kind: TabKind;
  resourceId?: string;
  resourceStatus?: ResourceStatus;
  wikiPath?: string;
}

/** Payload for the "chat-stream" Tauri event. */
export interface TabStreamPayload {
  tab_id: string;
  event: StreamEvent;
}

/** Payload for the "tab-created" Tauri event (backend-initiated tabs). */
export interface TabCreatedPayload {
  tab_id: string;
  label: string;
  kind: TabKind;
  resource_id?: string;
}

/** A resource row returned by list_resources. */
export interface ResourceItem {
  resource_id: string;
  url: string;
  title: string | null;
  wiki_path: string | null;
}

/** An experiment row returned by experiment_list. */
export interface ExperimentRow {
  uuid: string;
  name: string;
  command: string;
  status: string;
  exit_code: number | null;
  started_at: string;
  ended_at: string | null;
  tab_id: string;
}

/** Payload for "experiment-status" Tauri event. */
export interface ExperimentStatusPayload {
  uuid: string;
  status: string;
  exit_code?: number;
}

/** Payload for "experiment-log-line" Tauri event. */
export interface ExperimentLogLinePayload {
  uuid: string;
  stream: "stdout" | "stderr";
  line: string;
}

/** Payload for "experiment-starting" Tauri event (links UUID to pending card). */
export interface ExperimentStartingPayload {
  tab_id: string;
  uuid: string;
}
