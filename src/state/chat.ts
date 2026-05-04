import { create } from "zustand";
import type { AssistantMessage, ResourceStatus, Tab } from "../types";

const MAIN_TAB_ID = "main";

interface ChatStore {
  tabs: Tab[];
  activeTabId: string;
  previousTabId: string | null;

  addTab: (tab: Tab) => void;
  createTab: (label?: string) => string;
  closeTab: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;

  addUserMessage: (tabId: string, text: string) => string;
  beginAssistantMessage: (tabId: string) => string;
  appendText: (tabId: string, msgId: string, chunk: string) => void;
  appendThinking: (tabId: string, msgId: string, chunk: string) => void;
  finishMessage: (tabId: string, msgId: string) => void;
  setMessageError: (tabId: string, msgId: string, error: string) => void;
  setStreaming: (tabId: string, v: boolean) => void;
  setResourceStatus: (tabId: string, status: ResourceStatus) => void;
  clearMessages: (tabId: string) => void;
}

let seq = 0;

function updateTab(tabs: Tab[], tabId: string, updater: (t: Tab) => Tab): Tab[] {
  return tabs.map((t) => (t.id === tabId ? updater(t) : t));
}

function updateMessage(
  tabs: Tab[],
  tabId: string,
  msgId: string,
  updater: (m: AssistantMessage) => AssistantMessage
): Tab[] {
  return updateTab(tabs, tabId, (t) => ({
    ...t,
    messages: t.messages.map((m) =>
      m.id === msgId && m.role === "assistant" ? updater(m as AssistantMessage) : m
    ),
  }));
}

export const MAIN_TAB: Tab = {
  id: MAIN_TAB_ID,
  label: "Main",
  messages: [],
  isStreaming: false,
  isPinned: true,
  kind: "chat",
};

export const useChat = create<ChatStore>((set) => ({
  tabs: [MAIN_TAB],
  activeTabId: MAIN_TAB_ID,
  previousTabId: null,

  addTab: (tab) =>
    set((s) => ({ tabs: [...s.tabs, tab] })),

  createTab: (label = "Chat") => {
    const id = crypto.randomUUID();
    set((s) => ({
      tabs: [...s.tabs, { id, label, messages: [], isStreaming: false, isPinned: false, kind: "chat" }],
      previousTabId: s.activeTabId,
      activeTabId: id,
    }));
    return id;
  },

  closeTab: (tabId) =>
    set((s) => {
      const tab = s.tabs.find((t) => t.id === tabId);
      if (!tab || tab.isPinned) return s;
      const newActiveTabId =
        s.activeTabId === tabId
          ? (s.previousTabId ?? MAIN_TAB_ID)
          : s.activeTabId;
      return {
        tabs: s.tabs.filter((t) => t.id !== tabId),
        activeTabId: newActiveTabId,
        previousTabId: null,
      };
    }),

  setActiveTab: (tabId) =>
    set((s) => ({ previousTabId: s.activeTabId, activeTabId: tabId })),

  addUserMessage: (tabId, text) => {
    const id = String(seq++);
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({
        ...t,
        messages: [...t.messages, { id, role: "user", text }],
      })),
    }));
    return id;
  },

  beginAssistantMessage: (tabId) => {
    const id = String(seq++);
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({
        ...t,
        messages: [
          ...t.messages,
          { id, role: "assistant", text: "", isStreaming: true } as AssistantMessage,
        ],
      })),
    }));
    return id;
  },

  appendText: (tabId, msgId, chunk) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => ({ ...m, text: m.text + chunk })),
    })),

  appendThinking: (tabId, msgId, chunk) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => ({
        ...m,
        thinking: (m.thinking ?? "") + chunk,
      })),
    })),

  finishMessage: (tabId, msgId) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => ({ ...m, isStreaming: false })),
    })),

  setMessageError: (tabId, msgId, error) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => ({
        ...m,
        error,
        isStreaming: false,
      })),
    })),

  setStreaming: (tabId, v) =>
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({ ...t, isStreaming: v })),
    })),

  setResourceStatus: (tabId, status) =>
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({ ...t, resourceStatus: status })),
    })),

  clearMessages: (tabId) =>
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({
        ...t,
        messages: [],
        isStreaming: false,
      })),
    })),
}));

export { MAIN_TAB_ID };
