export type ChatMode = "brainstorm" | "experiment";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
}

export interface WikiFile {
  content: string;
  frontmatter: Record<string, string> | null;
}
