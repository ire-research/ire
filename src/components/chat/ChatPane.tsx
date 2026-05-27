import { useEffect, useRef, useState } from "react";
import { useChat } from "../../state/chat";
import { useChatOptions } from "../../state/chatOptions";
import { useWorkspace } from "../../state/workspace";
import { toastError } from "../../state/toasts";
import {
  ipc,
  onChatStream,
  onExperimentLogLine,
  onExperimentStarting,
  onExperimentStatus,
  onTabCreated,
} from "../../ipc";
import { MessageList } from "./MessageList";
import { Composer } from "./Composer";
import { TabBar } from "./TabBar";
import { HistoryPanel } from "./HistoryPanel";
import { ResourcePreviewPane } from "./ResourcePreviewPane";
import { ExperimentTabView } from "./ExperimentTabView";
import { Icon } from "../Icon";
import type { AskAnswer, AskBlockState, ChatMessage, Tab } from "../../types";

const seenStreamEventIds = new Map<string, number>();

const HERO_MESSAGES = [
  "Advancing science...",
  "Answering big questions...",
  "Accelerating discovery...",
  "Exploring the unknown...",
  "Pushing knowledge forward...",
  "Investigating new ideas...",
  "Connecting the dots...",
  "Uncovering new knowledge...",
  "Discovering what matters...",
  "Research without limits...",
  "Think deeper...",
  "Explore further...",
  "Discover faster...",
];

function randomHeroMessage() {
  return HERO_MESSAGES[Math.floor(Math.random() * HERO_MESSAGES.length)];
}

function shouldProcessStreamEvent(tabId: string, streamId?: string, eventId?: number): boolean {
  if (!streamId || eventId === undefined) return true;
  const key = `${tabId}:${streamId}`;
  const previous = seenStreamEventIds.get(key);
  if (previous !== undefined && eventId <= previous) return false;
  seenStreamEventIds.set(key, eventId);
  return true;
}

export function ChatPane() {
  const { model, provider, effort } = useChatOptions();

  const {
    tabs,
    activeTabId,
    addTab,
    createTab,
    renameTab,
    closeTab,
    setActiveTab,
    addUserMessage,
    beginAssistantMessage,
    appendText,
    appendThinking,
    finishMessage,
    setMessageError,
    setStreaming,
    setResourceStatus,
    addTool,
    markToolDone,
    addAskQuestion,
    linkExperimentUuid,
    updateExperimentStatus,
    appendExperimentLog,
    setTabHistoryMeta,
    createTabWithMessages,
  } = useChat();

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0];

  const [previewContent, setPreviewContent] = useState("");
  const [historyOpen, setHistoryOpen] = useState(false);
  const [heroMessage, setHeroMessage] = useState(randomHeroMessage);

  useEffect(() => {
    if (activeTab?.kind !== "preview" || !activeTab.wikiPath) return;
    ipc.readWikiFile(activeTab.wikiPath).then((f) => setPreviewContent(f.content));
  }, [activeTab?.id]);

  // Maps tab_id → in-flight assistant message id.
  const assistantIdByTab = useRef<Map<string, string>>(new Map());
  // Per-tab stable session UUID and start time — generated on first send, stored
  // on the tab, and reused for upserts so restart/close does not duplicate rows.
  const sessionUuidByTab = useRef<Map<string, string>>(new Map());
  const sessionStartedAtByTab = useRef<Map<string, string>>(new Map());

  // Global stream listener — routes events to the correct tab.
  //
  // Tauri's listen() is async; it returns a Promise<UnlistenFn>. React Strict
  // Mode runs cleanup synchronously before that Promise resolves, so the naive
  // pattern (store the fn, call it in cleanup) leaves the first listener alive
  // and registers a second one — every event fires twice. The `cancelled` flag
  // lets the .then() handler unlisten immediately when cleanup already ran.
  useEffect(() => {
    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    function reg(p: Promise<() => void>) {
      p.then((u) => { if (cancelled) u(); else unlisteners.push(u); });
    }

    reg(onChatStream(({ tab_id, stream_id, event_id, event }) => {
      if (!shouldProcessStreamEvent(tab_id, stream_id, event_id)) return;

      const msgId = assistantIdByTab.current.get(tab_id);

      switch (event.kind) {
        case "Init":
          // For backend-initiated tabs (e.g. resource), CC starts without a user send.
          if (!assistantIdByTab.current.has(tab_id)) {
            const aid = beginAssistantMessage(tab_id);
            assistantIdByTab.current.set(tab_id, aid);
            setStreaming(tab_id, true);
            // Mark resource tab as actively summarizing.
            const tab = useChat.getState().tabs.find((t) => t.id === tab_id);
            if (tab?.kind === "resource") {
              setResourceStatus(tab_id, "summarizing");
            }
          }
          break;

        case "TextDelta":
          if (msgId) appendText(tab_id, msgId, event.text);
          break;

        case "ThinkingDelta":
          if (msgId) appendThinking(tab_id, msgId, event.text);
          break;

        case "ToolStart":
          if (msgId) {
            addTool(tab_id, msgId, event.tool);
          }
          break;

        case "ToolDone":
          markToolDone(tab_id, event.tool_id, event.output, event.status, event.meta);
          break;

        case "AskUserQuestion":
          if (msgId) addAskQuestion(tab_id, msgId, event.tool_id, event.questions);
          break;

        case "Result":
          if (msgId && event.text) appendText(tab_id, msgId, event.text);
          break;

        case "Error":
          if (msgId) setMessageError(tab_id, msgId, event.message);
          setStreaming(tab_id, false);
          assistantIdByTab.current.delete(tab_id);
          void ipc.saveWorkspaceState(useWorkspace.getState().toPersisted())
            .catch((e) => toastError("save state", e));
          break;

        case "Done": {
          if (msgId) finishMessage(tab_id, msgId);
          setStreaming(tab_id, false);
          assistantIdByTab.current.delete(tab_id);
          void persistCompletedChat(tab_id);
          void ipc.saveWorkspaceState(useWorkspace.getState().toPersisted())
            .catch((e) => toastError("save state", e));

          const currentTab = useChat.getState().tabs.find((t) => t.id === tab_id);
          if (currentTab?.kind === "resource") {
            if (currentTab.resourceStatus === "summarizing") {
              setResourceStatus(tab_id, "ready");
            } else if (currentTab.resourceStatus === "confirmed") {
              // CC wrote the wiki file; WikiStore::write linked the DB row and
              // emitted `resource-changed` already, so just close the tab.
              closeTab(tab_id);
            }
          }
          break;
        }
      }
    }));

    reg(onTabCreated((payload) => {
      const newTab: Tab = {
        id: payload.tab_id,
        label: payload.label,
        messages: [],
        isStreaming: false,
        isPinned: false,
        kind: payload.kind,
        agentOptions: payload.agent_options,
        resourceId: payload.resource_id,
        resourceStatus: payload.kind === "resource" ? "summarizing" : undefined,
      };
      addTab(newTab);
      setActiveTab(payload.tab_id);
    }));

    reg(onExperimentStarting(({ tab_id, uuid, pid }) => {
      linkExperimentUuid(tab_id, uuid, pid);
    }));

    reg(onExperimentStatus(({ uuid, status, exit_code }) => {
      updateExperimentStatus(uuid, status as never, exit_code);
    }));

    reg(onExperimentLogLine(({ uuid, line }) => {
      appendExperimentLog(uuid, line);
    }));

    return () => {
      cancelled = true;
      unlisteners.forEach((u) => u());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Return (or lazily create) the stable session UUID + startedAt for a tab. */
  const getSessionMeta = (tabId: string): { uuid: string; startedAt: string } => {
    const tab = useChat.getState().tabs.find((t) => t.id === tabId);
    if (!sessionUuidByTab.current.has(tabId) && tab?.historySessionUuid && tab.historyStartedAt) {
      sessionUuidByTab.current.set(tabId, tab.historySessionUuid);
      sessionStartedAtByTab.current.set(tabId, tab.historyStartedAt);
    }
    if (!sessionUuidByTab.current.has(tabId)) {
      const uuid = crypto.randomUUID();
      const startedAt = new Date().toISOString();
      sessionUuidByTab.current.set(tabId, uuid);
      sessionStartedAtByTab.current.set(tabId, startedAt);
      setTabHistoryMeta(tabId, uuid, startedAt);
    }
    return {
      uuid: sessionUuidByTab.current.get(tabId)!,
      startedAt: sessionStartedAtByTab.current.get(tabId)!,
    };
  };

  const persistCompletedChat = async (tabId: string) => {
    const tab = useChat.getState().tabs.find((t) => t.id === tabId);
    if (!tab || tab.kind !== "chat" || tab.messages.length === 0 || tab.isStreaming) return;
    const { uuid, startedAt } = getSessionMeta(tabId);
    const { model: m, provider: p } = useChatOptions.getState();
    await ipc
      .chatHistorySave(tab.label, p, m, startedAt, JSON.stringify(tab.messages), uuid)
      .catch((e) => toastError("save chat history", e));
  };

  const handleSend = async (text: string) => {
    if (!activeTab || activeTab.isStreaming) return;

    // Ensure a stable session UUID exists for this tab before the first message.
    getSessionMeta(activeTabId);

    addUserMessage(activeTabId, text);
    const aid = beginAssistantMessage(activeTabId);
    assistantIdByTab.current.set(activeTabId, aid);
    setStreaming(activeTabId, true);

    try {
      await ipc
        .saveWorkspaceState(useWorkspace.getState().toPersisted())
        .catch((e) => toastError("save chat options", e));
      await ipc.chatSend(activeTabId, text, { model, provider, effort });
    } catch (err) {
      const currentMsgId = assistantIdByTab.current.get(activeTabId);
      if (currentMsgId) setMessageError(activeTabId, currentMsgId, String(err));
      assistantIdByTab.current.delete(activeTabId);
      setStreaming(activeTabId, false);
      void persistCompletedChat(activeTabId);
      void ipc.saveWorkspaceState(useWorkspace.getState().toPersisted())
        .catch((e) => toastError("save state", e));
    }
  };

  const handleAskSubmit = (ask: AskBlockState, answers: AskAnswer[]) => {
    const lines = ask.questions.map((q, i) => {
      const a = answers[i];
      const value = Array.isArray(a) ? (a.length ? a.join(", ") : "(no answer)") : (a || "(no answer)");
      return `- **${q.header}**: ${value}`;
    });
    const text = `Answers to your questions:\n${lines.join("\n")}`;
    void handleSend(text);
  };

  const handleNewTab = () => {
    if (tabs.length === 0) {
      setHeroMessage(randomHeroMessage());
    }
    createTab();
  };

  const handleCloseTab = async (tabId: string) => {
    if (activeTab.isStreaming && tabId === activeTabId) {
      ipc.chatCancel(tabId);
    }
    // Save history for any non-streaming chat tab with messages before removing it.
    const tab = tabs.find((t) => t.id === tabId);
    if (tab && tab.kind === "chat" && tab.messages.length > 0 && !tab.isStreaming) {
      await persistCompletedChat(tabId);
      sessionUuidByTab.current.delete(tabId);
      sessionStartedAtByTab.current.delete(tabId);
    }
    closeTab(tabId);
    void ipc.saveWorkspaceState(useWorkspace.getState().toPersisted())
      .catch((e) => toastError("save state", e));
  };

  const handleConfirmResource = async () => {
    if (!activeTab.resourceId) return;
    setResourceStatus(activeTabId, "confirmed");
    const aid = beginAssistantMessage(activeTabId);
    assistantIdByTab.current.set(activeTabId, aid);
    setStreaming(activeTabId, true);
    try {
      const prompt = await ipc.getResourceConfirmPrompt();
      await ipc
        .saveWorkspaceState(useWorkspace.getState().toPersisted())
        .catch((e) => toastError("save chat options", e));
      await ipc.chatSend(activeTabId, prompt, activeTab.agentOptions ?? { model, provider, effort });
    } catch (err) {
      const msgId = assistantIdByTab.current.get(activeTabId);
      if (msgId) setMessageError(activeTabId, msgId, String(err));
      setStreaming(activeTabId, false);
    }
  };

  const handleRestore = async (sessionUuid: string, tabLabel: string, startedAt?: string) => {
    const json = await ipc
      .chatHistoryGet(sessionUuid)
      .catch((e) => { toastError("load history", e); return null; });
    if (!json) return;
    const messages: ChatMessage[] = JSON.parse(json);
    // Remove from history — the session is now active again as a tab.
    // It will be re-saved to history when the tab is closed.
    ipc.chatHistoryDelete(sessionUuid).catch(() => {});
    createTabWithMessages(tabLabel, messages, sessionUuid, startedAt);
  };

  const handleDiscardResource = () => {
    if (activeTab.resourceId) {
      ipc.discardResource(activeTab.resourceId).catch((e) => toastError("discard resource", e));
    }
    closeTab(activeTabId);
  };

  const activeHistorySessionUuids = tabs
    .filter((tab) => tab.kind === "chat" && tab.historySessionUuid)
    .map((tab) => tab.historySessionUuid!);

  const tabBar = (
    <TabBar
      tabs={tabs}
      activeTabId={activeTabId}
      onSelect={setActiveTab}
      onClose={handleCloseTab}
      onNew={handleNewTab}
      onRename={renameTab}
      rightSlot={
        <>
          <button
            className="flex h-8 w-8 items-center justify-center text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors"
            title="Chat history"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={() => setHistoryOpen((o) => !o)}
          >
            <i className="fa-solid fa-clock-rotate-left text-[13px]" />
          </button>
          <HistoryPanel
            isOpen={historyOpen}
            onClose={() => setHistoryOpen(false)}
            excludeSessionUuids={activeHistorySessionUuids}
            onRestore={handleRestore}
          />
        </>
      }
    />
  );

  const showResourceBar =
    activeTab?.kind === "resource" && activeTab.resourceStatus === "ready";

  if (tabs.length === 0) {
    return (
      <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
        {tabBar}
        <div className="flex-1 flex flex-col items-center justify-center gap-7 text-center px-10">
          <div className="flex flex-col items-center gap-3">
            <h1 className="text-xl font-semibold text-on-surface-variant tracking-tight">Integrated Research Environment (IRE)</h1>
            <p className="text-[13px] text-on-surface-variant max-w-sm leading-relaxed">
              {heroMessage}
            </p>
          </div>
          <button
            id="ire-new-chat-btn"
            className="inline-flex items-center gap-2 bg-on-surface-variant text-background text-[13px] font-medium px-4 py-2 rounded-lg hover:opacity-85 transition-opacity"
            onClick={handleNewTab}
          >
            <Icon name="chat" className="w-[14px] h-[14px]" />
            New chat
          </button>
        </div>
      </section>
    );
  }

  if (activeTab.kind === "preview") {
    return (
      <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
        {tabBar}
        <div className="flex-1 overflow-hidden relative">
          <ResourcePreviewPane title={activeTab.label} content={previewContent} />
        </div>
      </section>
    );
  } else if (activeTab.kind === "experiment") {
    return (
      <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
        {tabBar}
        <div className="flex-1 overflow-hidden relative">
          <div className="absolute inset-0 overflow-y-auto px-4 py-4 pb-8">
            <ExperimentTabView uuid={activeTab.experimentUuid!} name={activeTab.label} />
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
      {tabBar}
      <div className="flex-1 overflow-hidden relative">
        {/* Chat view */}
        <div className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-4 pb-40 space-y-6">
          <MessageList messages={activeTab.messages} onAskSubmit={handleAskSubmit} />
        </div>
        {/* Floating composer */}
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2 w-full px-6 pointer-events-none" style={{ zIndex: 20 }}>
          <div className="pointer-events-auto">
            {!showResourceBar && (
              <Composer onSend={handleSend} disabled={activeTab.isStreaming} onCancel={() => ipc.chatCancel(activeTabId)} />
            )}
          </div>
        </div>
        {/* Resource bar */}
        {showResourceBar && (
          <div className="absolute bottom-6 left-1/2 -translate-x-1/2 flex gap-3 bg-surface-container border border-outline-variant rounded-lg px-4 py-2 shadow-lg shadow-black/30" style={{ zIndex: 20 }}>
            <button className="text-[13px] text-on-surface font-medium hover:text-ok transition-colors" onClick={handleConfirmResource}>
              Confirm — save to wiki
            </button>
            <span className="text-outline-variant">·</span>
            <button className="text-[13px] text-on-surface-variant hover:text-error transition-colors" onClick={handleDiscardResource}>
              Discard
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
