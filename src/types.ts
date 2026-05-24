export type EffortLevel = "low" | "medium" | "high" | "xhigh" | "max";

export interface ChatOptions {
  model: string;
  effort: EffortLevel;
}

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
  input_full: string | null;
  output_preview: string | null;
  output_full: string | null;
  isDone: boolean;
  experimentUuid?: string;
  experimentStatus?: ExperimentStatus;
  experimentExitCode?: number;
  experimentPid?: number;
  logLines?: string[];
}

export interface AskQuestionOption {
  label: string;
  description?: string;
}

export interface AskQuestion {
  header: string;
  question: string;
  multi_select: boolean;
  options: AskQuestionOption[];
}

/** A single answer per question. For multi-select it's an array of labels; for
 *  single-select it's a single label. "Other: <text>" denotes a custom value. */
export type AskAnswer = string | string[];

export interface AskBlockState {
  tool_id: string;
  questions: AskQuestion[];
  /** Map question index → answer. Undefined entries mean unanswered. */
  answers: (AskAnswer | undefined)[];
  /** True once the user has submitted; the card locks. */
  submitted: boolean;
}

export type AssistantContentBlock =
  | { id: string; kind: "text"; text: string }
  | { id: string; kind: "thinking"; text: string }
  | { id: string; kind: "tool"; tool: ToolCallState }
  | { id: string; kind: "ask"; ask: AskBlockState };

export interface AssistantMessage {
  id: string;
  role: "assistant";
  blocks: AssistantContentBlock[];
  isStreaming: boolean;
  error?: string;
}

export type ChatMessage = UserMessage | AssistantMessage;

export type StreamEvent =
  | { kind: "Init"; session_id: string }
  | { kind: "TextDelta"; text: string }
  | { kind: "ThinkingDelta"; text: string }
  | { kind: "ToolStart"; tool_id: string; tool_name: string; input_preview: string | null; input_full: string | null }
  | { kind: "ToolDone"; tool_id: string; output_preview: string | null; output_full: string | null }
  | { kind: "AskUserQuestion"; tool_id: string; questions: AskQuestion[] }
  | { kind: "Result"; text: string | null; session_id: string }
  | { kind: "Error"; message: string }
  | { kind: "Done" };

export interface WikiFile {
  content: string;
  frontmatter: Record<string, string> | null;
}

export type TabKind = "chat" | "resource" | "preview" | "experiment";
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
  experimentUuid?: string;
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
  source_type: "url" | "local_file" | "batch";
  source_label: string;
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
  pid?: number;
}

export interface IdeaItem {
  id: string;
  text: string;
  trashed: boolean;
  order: number;
}

export interface PulseContent {
  research_question: string;
  this_week: string;
}

export interface SystemStatus {
  workspace_path: string;
  git_branch: string;
  git_insertions: number;
  git_deletions: number;
  cpu_model: string;
  cpu_usage_pct: number;
  gpu_model: string | null;
  gpu_usage_pct: number | null;
  gpu_vram_gb: number | null;
  ram_total_gb: number;
  hostname: string;
  username: string;
  cc_connected: boolean;
}
