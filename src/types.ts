export type ChatMode = "brainstorm" | "experiment";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
}
