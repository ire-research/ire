export type ChatMode = "brainstorm" | "experiment";

export interface UserMessage {
  id: string;
  role: "user";
  text: string;
}

export interface AssistantMessage {
  id: string;
  role: "assistant";
  text: string;
  thinking?: string;
  isStreaming: boolean;
  error?: string;
}

export type ChatMessage = UserMessage | AssistantMessage;

export type StreamEvent =
  | { kind: "Init"; session_id: string }
  | { kind: "TextDelta"; text: string }
  | { kind: "ThinkingDelta"; text: string }
  | { kind: "ToolStart"; tool_id: string; tool_name: string; input_preview: string | null }
  | { kind: "ToolInputDelta"; tool_id: string; partial_json: string }
  | { kind: "ToolDone"; tool_id: string; output_preview: string | null }
  | { kind: "Result"; text: string | null; session_id: string }
  | { kind: "Error"; message: string }
  | { kind: "Done" };

export interface WikiFile {
  content: string;
  frontmatter: Record<string, string> | null;
}

export type TabKind = "chat" | "resource";
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
