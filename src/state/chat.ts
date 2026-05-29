import { create } from "zustand";
import type { AskAnswer, AskQuestion, AssistantContentBlock, AssistantMessage, ChatMessage, ChatOptions, ExperimentStatus, ResourceStatus, Tab, ToolCallState, ToolIo, ToolMeta, ToolStatus } from "../types";

const MAIN_TAB_ID = "main";

interface ChatStore {
  tabs: Tab[];
  activeTabId: string;
  previousTabId: string | null;

  addTab: (tab: Tab) => void;
  createTab: (label?: string) => string;
  renameTab: (tabId: string, label: string) => void;
  openPreviewTab: (label: string, wikiPath: string) => void;
  openExperimentTab: (uuid: string, name: string) => void;
  closeTab: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;
  /** Clear all tabs and messages — call on workspace close to prevent state from
   *  the previous workspace leaking into the next one. */
  reset: () => void;

  addUserMessage: (tabId: string, text: string) => string;
  beginAssistantMessage: (tabId: string) => string;
  appendText: (tabId: string, msgId: string, chunk: string) => void;
  appendThinking: (tabId: string, msgId: string, chunk: string) => void;
  finishMessage: (tabId: string, msgId: string) => void;
  setMessageError: (tabId: string, msgId: string, error: string) => void;
  setStreaming: (tabId: string, v: boolean) => void;
  setResourceStatus: (tabId: string, status: ResourceStatus) => void;
  clearMessages: (tabId: string) => void;

  // AskUserQuestion
  addAskQuestion: (tabId: string, msgId: string, toolId: string, questions: AskQuestion[]) => void;
  setAskAnswer: (toolId: string, index: number, answer: AskAnswer | undefined) => void;
  markAskSubmitted: (toolId: string) => void;

  // Tool call management
  addTool: (tabId: string, msgId: string, tool: ToolCallState) => void;
  markToolDone: (tabId: string, toolId: string, output: ToolIo | null, status: ToolStatus, meta?: ToolMeta | null) => void;
  /** Link the pending experiment card in tabId to its assigned UUID and PID. */
  linkExperimentUuid: (tabId: string, uuid: string, pid?: number) => void;
  /** Update experiment status across all tabs by UUID. */
  updateExperimentStatus: (uuid: string, status: ExperimentStatus, exitCode?: number) => void;
  /** Append a log line to the experiment card with the given UUID. */
  appendExperimentLog: (uuid: string, line: string) => void;
  /** Remove a tool card by tool_id from all messages across all tabs. */
  removeTool: (toolId: string) => void;
  /** Restore tabs persisted from a previous workspace session. Replaces all
   *  current tabs. Any tab with isStreaming=true is normalised to false. */
  restorePersistedTabs: (tabs: Tab[], activeTabId?: string) => void;
  setTabAgentOptions: (tabId: string, options: ChatOptions) => void;
  setTabHistoryMeta: (tabId: string, sessionUuid: string, startedAt: string) => void;
  /** Open a new chat tab pre-populated with historical messages (read from history). */
  createTabWithMessages: (label: string, messages: ChatMessage[], sessionUuid?: string, startedAt?: string, agentOptions?: ChatOptions) => void;
}

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
    blocks: msg.blocks.map((block) =>
      block.kind === "tool" && predicate(block.tool)
        ? { ...block, tool: updater(block.tool) }
        : block
    ),
  };
}

function appendTextBlock(msg: AssistantMessage, kind: "text" | "thinking", chunk: string): AssistantMessage {
  const last = msg.blocks[msg.blocks.length - 1];
  if (last?.kind === kind) {
    const updatedLast: AssistantContentBlock = { ...last, text: last.text + chunk };
    return { ...msg, blocks: [...msg.blocks.slice(0, -1), updatedLast] };
  }
  return { ...msg, blocks: [...msg.blocks, { id: crypto.randomUUID(), kind, text: chunk }] };
}

function messageHasTool(msg: AssistantMessage, predicate: (t: ToolCallState) => boolean): boolean {
  return msg.blocks.some((block) => block.kind === "tool" && predicate(block.tool));
}

/** Find the last assistant message in a tab that has a pending experiment card (no UUID yet). */
function findPendingExperimentMsgId(tabs: Tab[], tabId: string): string | null {
  const tab = tabs.find((t) => t.id === tabId);
  if (!tab) return null;
  for (let i = tab.messages.length - 1; i >= 0; i--) {
    const m = tab.messages[i];
    if (m.role === "assistant") {
      const am = m as AssistantMessage;
      const pending = messageHasTool(am, (t) => t.kind === "experiment_start" && !t.meta.experiment_uuid);
      if (pending) return am.id;
    }
  }
  return null;
}

export const MAIN_TAB: Tab = {
  id: MAIN_TAB_ID,
  label: "Untitled",
  messages: [],
  isStreaming: false,
  isPinned: false,
  kind: "chat",
};

// Tracks when each assistant message started streaming, keyed by message id.
// Used to compute runtime when finishMessage is called. Not persisted.
const messageStartTimes = new Map<string, number>();

export const useChat = create<ChatStore>((set) => ({
  tabs: [MAIN_TAB],
  activeTabId: MAIN_TAB_ID,
  previousTabId: null,

  addTab: (tab) =>
    set((s) => ({ tabs: [...s.tabs, tab] })),

  renameTab: (tabId, label) =>
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({ ...t, label })),
    })),

  createTab: (label = "Untitled") => {
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
      if (!tab) return s;
      const remaining = s.tabs.filter((t) => t.id !== tabId);
      const newActiveTabId =
        s.activeTabId === tabId
          ? (remaining.find((t) => t.id === s.previousTabId)?.id ?? remaining[0]?.id ?? "")
          : s.activeTabId;
      return {
        tabs: remaining,
        activeTabId: newActiveTabId,
        previousTabId: null,
      };
    }),

  setActiveTab: (tabId) =>
    set((s) => ({ previousTabId: s.activeTabId, activeTabId: tabId })),

  reset: () =>
    set({
      tabs: [{ ...MAIN_TAB, messages: [] }],
      activeTabId: MAIN_TAB_ID,
      previousTabId: null,
    }),

  addUserMessage: (tabId, text) => {
    const id = crypto.randomUUID();
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({
        ...t,
        messages: [...t.messages, { id, role: "user", text }],
      })),
    }));
    return id;
  },

  beginAssistantMessage: (tabId) => {
    const id = crypto.randomUUID();
    messageStartTimes.set(id, Date.now());
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({
        ...t,
        messages: [
          ...t.messages,
          { id, role: "assistant", blocks: [], isStreaming: true } as AssistantMessage,
        ],
      })),
    }));
    return id;
  },

  appendText: (tabId, msgId, chunk) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => appendTextBlock(m, "text", chunk)),
    })),

  appendThinking: (tabId, msgId, chunk) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => appendTextBlock(m, "thinking", chunk)),
    })),

  finishMessage: (tabId, msgId) =>
    set((s) => {
      const startTime = messageStartTimes.get(msgId);
      messageStartTimes.delete(msgId);
      const runtime = startTime !== undefined ? (Date.now() - startTime) / 1000 : undefined;
      return {
        tabs: updateMessage(s.tabs, tabId, msgId, (m) => ({
          ...m,
          isStreaming: false,
          ...(runtime !== undefined ? { runtime } : {}),
        })),
      };
    }),

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
        blocks: [...m.blocks, { id: crypto.randomUUID(), kind: "tool", tool }],
      })),
    })),

  addAskQuestion: (tabId, msgId, toolId, questions) =>
    set((s) => ({
      tabs: updateMessage(s.tabs, tabId, msgId, (m) => ({
        ...m,
        blocks: [
          ...m.blocks,
          {
            id: crypto.randomUUID(),
            kind: "ask",
            ask: {
              tool_id: toolId,
              questions,
              answers: questions.map(() => undefined),
              submitted: false,
            },
          },
        ],
      })),
    })),

  setAskAnswer: (toolId, index, answer) =>
    set((s) => ({
      tabs: s.tabs.map((tab) => ({
        ...tab,
        messages: tab.messages.map((m) => {
          if (m.role !== "assistant") return m;
          const am = m as AssistantMessage;
          if (!am.blocks.some((b) => b.kind === "ask" && b.ask.tool_id === toolId)) return m;
          return {
            ...am,
            blocks: am.blocks.map((block) => {
              if (block.kind !== "ask" || block.ask.tool_id !== toolId) return block;
              const nextAnswers = block.ask.answers.slice();
              nextAnswers[index] = answer;
              return { ...block, ask: { ...block.ask, answers: nextAnswers } };
            }),
          };
        }),
      })),
    })),

  markAskSubmitted: (toolId) =>
    set((s) => ({
      tabs: s.tabs.map((tab) => ({
        ...tab,
        messages: tab.messages.map((m) => {
          if (m.role !== "assistant") return m;
          const am = m as AssistantMessage;
          if (!am.blocks.some((b) => b.kind === "ask" && b.ask.tool_id === toolId)) return m;
          return {
            ...am,
            blocks: am.blocks.map((block) =>
              block.kind === "ask" && block.ask.tool_id === toolId
                ? { ...block, ask: { ...block.ask, submitted: true } }
                : block
            ),
          };
        }),
      })),
    })),

  markToolDone: (tabId, toolId, output, status, meta?) =>
    set((s) => {
      const tab = s.tabs.find((t) => t.id === tabId);
      if (!tab) return s;
      for (const msg of tab.messages) {
        if (msg.role !== "assistant") continue;
        const am = msg as AssistantMessage;
        if (messageHasTool(am, (t) => t.tool_id === toolId)) {
          return {
            tabs: updateMessage(s.tabs, tabId, am.id, (m) =>
              updateToolInMessage(m, (t) => t.tool_id === toolId, (t) => ({
                ...t,
                output,
                status,
                meta: { ...t.meta, ...(meta ?? {}) },
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
            (t) => t.kind === "experiment_start" && !t.meta.experiment_uuid,
            (t) => ({
              ...t,
              meta: {
                ...t.meta,
                experiment_uuid: uuid,
                experiment_status: "running" as ExperimentStatus,
                ...(pid !== undefined ? { experiment_pid: pid } : {}),
              },
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
          if (messageHasTool(am, (t) => t.meta.experiment_uuid === uuid)) {
            return {
              tabs: updateMessage(s.tabs, tab.id, am.id, (m) =>
                updateToolInMessage(
                  m,
                  (t) => t.meta.experiment_uuid === uuid,
                  (t) => ({
                    ...t,
                    meta: {
                      ...t.meta,
                      experiment_status: status,
                      ...(exitCode !== undefined ? { experiment_exit_code: exitCode } : {}),
                    },
                  })
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
          if (messageHasTool(am, (t) => t.meta.experiment_uuid === uuid)) {
            return {
              tabs: updateMessage(s.tabs, tab.id, am.id, (m) =>
                updateToolInMessage(
                  m,
                  (t) => t.meta.experiment_uuid === uuid,
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
          if (!messageHasTool(am, (t) => t.tool_id === toolId)) return m;
          return {
            ...am,
            blocks: am.blocks.filter((block) => block.kind !== "tool" || block.tool.tool_id !== toolId),
          };
        }),
      })),
    })),

  restorePersistedTabs: (tabs, activeTabId) =>
    set(() => {
      const normalised = tabs.map((t) => ({
        ...t,
        isStreaming: false,
        messages: t.messages.map((m) =>
          m.role === "assistant" ? { ...m, isStreaming: false } : m
        ),
      }));
      const restoredActiveTabId =
        normalised.find((t) => t.id === activeTabId)?.id ?? normalised[0]?.id ?? MAIN_TAB_ID;
      return {
        tabs: normalised,
        activeTabId: restoredActiveTabId,
        previousTabId: null,
      };
    }),

  setTabAgentOptions: (tabId, options) =>
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({
        ...t,
        agentOptions: options,
      })),
    })),

  setTabHistoryMeta: (tabId, sessionUuid, startedAt) =>
    set((s) => ({
      tabs: updateTab(s.tabs, tabId, (t) => ({
        ...t,
        historySessionUuid: sessionUuid,
        historyStartedAt: startedAt,
      })),
    })),

  createTabWithMessages: (label, messages, sessionUuid, startedAt, agentOptions) => {
    const id = crypto.randomUUID();
    const clean: ChatMessage[] = messages.map((m) =>
      m.role === "assistant" ? { ...m, isStreaming: false } : m
    );
    set((s) => ({
      tabs: [
        ...s.tabs,
        {
          id,
          label,
          messages: clean,
          isStreaming: false,
          isPinned: false,
          kind: "chat",
          historySessionUuid: sessionUuid,
          historyStartedAt: startedAt,
          agentOptions,
        },
      ],
      previousTabId: s.activeTabId,
      activeTabId: id,
    }));
  },
}));
