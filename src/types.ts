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
