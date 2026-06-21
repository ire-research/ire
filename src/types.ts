export type EffortLevel = "low" | "medium" | "high" | "xhigh" | "max";
export type Provider = "claude" | "codex";

export interface ChatOptions {
  model: string;
  provider: Provider;
  effort: EffortLevel | null;
}

export type ExperimentStatus = "starting" | "running" | "completed" | "failed" | "cancelled";
export type ToolProvider = "claude" | "codex";
export type ToolKind =
  | "command"
  | "file_read"
  | "file_write"
  | "file_edit"
  | "file_search"
  | "web_fetch"
  | "ire_read"
  | "ire_edit"
  | "resource_add"
  | "memory_write"
  | "experiment_start"
  | "experiment_status"
  | "experiment_tail_logs"
  | "other";
export type ToolStatus = "running" | "completed" | "failed";
export type ToolFormat = "text" | "json";

export interface UserMessage {
  id: string;
  role: "user";
  text: string;
}

export interface ToolIo {
  preview?: string | null;
  full?: string | null;
  format: ToolFormat;
}

export interface ToolMeta {
  [key: string]: unknown;
  path?: string;
  paths?: string[];
  command?: string;
  name?: string;
  experiment_uuid?: string;
  experiment_status?: ExperimentStatus;
  experiment_exit_code?: number;
  experiment_pid?: number;
}

export interface ToolCallState {
  tool_id: string;
  provider: ToolProvider;
  kind: ToolKind;
  raw_name: string;
  title: string;
  input: ToolIo;
  output?: ToolIo | null;
  status: ToolStatus;
  meta: ToolMeta;
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
  runtime?: number;
}

export type ChatMessage = UserMessage | AssistantMessage;

export type StreamEvent =
  | { kind: "Init"; session_id: string }
  | { kind: "TextDelta"; text: string }
  | { kind: "ThinkingDelta"; text: string }
  | { kind: "ToolStart"; tool: ToolCallState }
  | { kind: "ToolDone"; tool_id: string; output: ToolIo | null; status: ToolStatus; meta: ToolMeta }
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
  agentOptions?: ChatOptions;
  historySessionUuid?: string;
  historyStartedAt?: string;
  resourceId?: string;
  resourceStatus?: ResourceStatus;
  draftContent?: string;
  irePath?: string;
  experimentUuid?: string;
}

/** Payload for the "chat-stream" Tauri event. */
export interface TabStreamPayload {
  tab_id: string;
  stream_id?: string;
  event_id?: number;
  event: StreamEvent;
}

/** Payload for the "tab-created" Tauri event (backend-initiated tabs). */
export interface TabCreatedPayload {
  tab_id: string;
  label: string;
  kind: TabKind;
  resource_id?: string;
  resource_status?: ResourceStatus;
  agent_options?: ChatOptions;
}

/** A file-based resource discovered under `.ire/resources/`. Identity is the
 *  `path` (`resources/<slug>.md`); title and sources come from frontmatter. */
export interface ResourceItem {
  path: string;
  title: string;
  sources: string[];
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
  text: string;
}

export interface FocusContent {
  research_question: string;
  this_week: string;
}

/** Why a workspace-event fired. `hydrate` = initial burst on workspace open;
 *  `mutation` = a live change (CC write, command, etc.). Side-effect listeners
 *  (panel-flash animations, toasts) typically filter to `mutation` only. */
export type WorkspaceEventSource = "hydrate" | "mutation";

/** Payload for the "workspace-event" Tauri event. */
export type WorkspaceEvent =
  | { kind: "focus-changed"; source: WorkspaceEventSource; research_question: string; this_week: string }
  | { kind: "notes-changed"; source: WorkspaceEventSource; content: string }
  | { kind: "ideas-changed"; source: WorkspaceEventSource; ideas: IdeaItem[] }
  | { kind: "resource-changed"; source: WorkspaceEventSource; resource: ResourceItem }
  | { kind: "resource-deleted"; source: WorkspaceEventSource; path: string }
  | { kind: "experiment-changed"; source: WorkspaceEventSource; experiment: ExperimentRow }
  | { kind: "experiment-deleted"; source: WorkspaceEventSource; uuid: string };

export interface ChatSessionSummary {
  session_uuid: string;
  tab_label: string;
  provider: string;
  model: string;
  started_at: string;
  ended_at: string;
  message_count: number;
  first_user_msg: string | null;
}

export interface SystemInfo {
  cpu_model: string;
  ram_total_gb: number;
  gpu_model: string | null;
  gpu_vram_gb: number | null;
  hostname: string;
  username: string;
  cc_connected: boolean;
  codex_connected: boolean;
}

export interface SystemMetrics {
  git_branch: string;
  git_insertions: number;
  git_deletions: number;
  cpu_usage_pct: number;
  gpu_usage_pct: number | null;
}
