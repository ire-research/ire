import { create } from "zustand";
import type { AssistantMessage, ExperimentStatus, ResourceStatus, Tab, ToolCallState } from "../types";

const MAIN_TAB_ID = "main";

interface ChatStore {
  tabs: Tab[];
  activeTabId: string;
  previousTabId: string | null;

  addTab: (tab: Tab) => void;
  createTab: (label?: string) => string;
  openPreviewTab: (label: string, wikiPath: string) => void;
  openExperimentTab: (uuid: string, name: string) => void;
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

  // Tool call management
  addTool: (tabId: string, msgId: string, tool: ToolCallState) => void;
  markToolDone: (tabId: string, toolId: string, outputPreview?: string | null, outputFull?: string | null) => void;
  /** Link the pending experiment card in tabId to its assigned UUID and PID. */
  linkExperimentUuid: (tabId: string, uuid: string, pid?: number) => void;
  /** Update experiment status across all tabs by UUID. */
  updateExperimentStatus: (uuid: string, status: ExperimentStatus, exitCode?: number) => void;
  /** Append a log line to the experiment card with the given UUID. */
  appendExperimentLog: (uuid: string, line: string) => void;
  /** Remove a tool card by tool_id from all messages across all tabs. */
  removeTool: (toolId: string) => void;
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

function updateToolInMessage(
  msg: AssistantMessage,
  predicate: (t: ToolCallState) => boolean,
  updater: (t: ToolCallState) => ToolCallState
): AssistantMessage {
  return {
    ...msg,
    tools: (msg.tools ?? []).map((t) => (predicate(t) ? updater(t) : t)),
  };
}

// CC prefixes MCP tool names with the server name (e.g. "ire__experiment.start").
// Mirror the normalization in MessageList.tsx so look-ups match what was stored.
function isExperimentToolName(name: string): boolean {
  const parts = name.split("__");
  const bare = parts[parts.length - 1].replace(/_/g, ".");
  return bare === "experiment.start";
}

/** Find the last assistant message in a tab that has a pending experiment card (no UUID yet). */
function findPendingExperimentMsgId(tabs: Tab[], tabId: string): string | null {
  const tab = tabs.find((t) => t.id === tabId);
  if (!tab) return null;
  for (let i = tab.messages.length - 1; i >= 0; i--) {
    const m = tab.messages[i];
    if (m.role === "assistant") {
      const am = m as AssistantMessage;
      const pending = am.tools?.find(
        (t) => isExperimentToolName(t.tool_name) && !t.experimentUuid
      );
      if (pending) return am.id;
    }
  }
  return null;
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

  openPreviewTab: (label, wikiPath) =>
    set((s) => {
      const existing = s.tabs.find((t) => t.kind === "preview" && t.wikiPath === wikiPath);
      if (existing) {
        return { previousTabId: s.activeTabId, activeTabId: existing.id };
      }
      const id = crypto.randomUUID();
      return {
        tabs: [...s.tabs, { id, label, messages: [], isStreaming: false, isPinned: false, kind: "preview", wikiPath }],
        previousTabId: s.activeTabId,
        activeTabId: id,
      };
    }),

  openExperimentTab: (uuid, name) =>
    set((s) => {
      const existing = s.tabs.find((t) => t.kind === "experiment" && t.experimentUuid === uuid);
      if (existing) {
        return { previousTabId: s.activeTabId, activeTabId: existing.id };
      }
      const id = crypto.randomUUID();
      return {
        tabs: [...s.tabs, { id, label: name, messages: [], isStreaming: false, isPinned: false, kind: "experiment", experimentUuid: uuid }],
        previousTabId: s.activeTabId,
        activeTabId: id,
      };
    }),

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

  addTool: (tabId, msgId, tool) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => ({
        ...m,
        tools: [...(m.tools ?? []), tool],
      })),
    })),

  markToolDone: (tabId, toolId, outputPreview?, outputFull?) =>
    set((s) => {
      const tab = s.tabs.find((t) => t.id === tabId);
      if (!tab) return s;
      for (const msg of tab.messages) {
        if (msg.role !== "assistant") continue;
        const am = msg as AssistantMessage;
        if (am.tools?.some((t) => t.tool_id === toolId)) {
          return {
            tabs: updateMessage(s.tabs, tabId, am.id, (m) =>
              updateToolInMessage(m, (t) => t.tool_id === toolId, (t) => ({
                ...t,
                isDone: true,
                output_preview: outputPreview ?? null,
                output_full: outputFull ?? null,
              }))
            ),
          };
        }
      }
      return s;
    }),

  linkExperimentUuid: (tabId, uuid, pid) =>
    set((s) => {
      const msgId = findPendingExperimentMsgId(s.tabs, tabId);
      if (!msgId) return s;
      return {
        tabs: updateMessage(s.tabs, tabId, msgId, (m) =>
          updateToolInMessage(
            m,
            (t) => isExperimentToolName(t.tool_name) && !t.experimentUuid,
            (t) => ({
              ...t,
              experimentUuid: uuid,
              experimentStatus: "running" as ExperimentStatus,
              ...(pid !== undefined ? { experimentPid: pid } : {}),
            })
          )
        ),
      };
    }),

  updateExperimentStatus: (uuid, status, exitCode) =>
    set((s) => {
      for (const tab of s.tabs) {
        for (const msg of tab.messages) {
          if (msg.role !== "assistant") continue;
          const am = msg as AssistantMessage;
          if (am.tools?.some((t) => t.experimentUuid === uuid)) {
            return {
              tabs: updateMessage(s.tabs, tab.id, am.id, (m) =>
                updateToolInMessage(
                  m,
                  (t) => t.experimentUuid === uuid,
                  (t) => ({ ...t, experimentStatus: status, ...(exitCode !== undefined ? { experimentExitCode: exitCode } : {}) })
                )
              ),
            };
          }
        }
      }
      return s;
    }),

  appendExperimentLog: (uuid, line) =>
    set((s) => {
      for (const tab of s.tabs) {
        for (const msg of tab.messages) {
          if (msg.role !== "assistant") continue;
          const am = msg as AssistantMessage;
          if (am.tools?.some((t) => t.experimentUuid === uuid)) {
            return {
              tabs: updateMessage(s.tabs, tab.id, am.id, (m) =>
                updateToolInMessage(
                  m,
                  (t) => t.experimentUuid === uuid,
                  (t) => {
                    const lines = t.logLines ?? [];
                    // Keep last 50 lines to avoid unbounded growth.
                    const next = [...lines, line].slice(-50);
                    return { ...t, logLines: next };
                  }
                )
              ),
            };
          }
        }
      }
      return s;
    }),

  removeTool: (toolId) =>
    set((s) => ({
      tabs: s.tabs.map((tab) => ({
        ...tab,
        messages: tab.messages.map((m) => {
          if (m.role !== "assistant") return m;
          const am = m as AssistantMessage;
          if (!am.tools?.some((t) => t.tool_id === toolId)) return m;
          return { ...am, tools: am.tools.filter((t) => t.tool_id !== toolId) };
        }),
      })),
    })),
}));

export { MAIN_TAB_ID };
