import { create } from "zustand";
import type { AssistantMessage, ChatMessage } from "../types";

interface ChatStore {
  messages: ChatMessage[];
  isStreaming: boolean;
  addUserMessage: (text: string) => string;
  beginAssistantMessage: () => string;
  appendText: (id: string, chunk: string) => void;
  appendThinking: (id: string, chunk: string) => void;
  finishMessage: (id: string) => void;
  setMessageError: (id: string, error: string) => void;
  setStreaming: (v: boolean) => void;
  clearMessages: () => void;
}

let seq = 0;

export const useChat = create<ChatStore>((set) => ({
  messages: [],
  isStreaming: false,

  addUserMessage: (text) => {
    const id = String(seq++);
    set((s) => ({ messages: [...s.messages, { id, role: "user", text }] }));
    return id;
  },

  beginAssistantMessage: () => {
    const id = String(seq++);
    set((s) => ({
      messages: [
        ...s.messages,
        { id, role: "assistant", text: "", isStreaming: true },
      ],
    }));
    return id;
  },

  appendText: (id, chunk) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === id && m.role === "assistant"
          ? { ...m, text: m.text + chunk }
          : m
      ),
    })),

  appendThinking: (id, chunk) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === id && m.role === "assistant"
          ? { ...m, thinking: ((m as AssistantMessage).thinking ?? "") + chunk }
          : m
      ),
    })),

  finishMessage: (id) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === id ? { ...m, isStreaming: false } : m
      ),
    })),

  setMessageError: (id, error) =>
    set((s) => ({
      messages: s.messages.map((m) =>
        m.id === id && m.role === "assistant"
          ? { ...m, error, isStreaming: false }
          : m
      ),
    })),

  setStreaming: (v) => set({ isStreaming: v }),

  clearMessages: () => set({ messages: [], isStreaming: false }),
}));
