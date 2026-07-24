import { useEffect, useRef, useState } from "react";
import { useChat } from "../../state/chat";
import { defaultEffortForModel, lightweightModelForProvider, useChatOptions } from "../../state/chatOptions";
import { useWorkspace } from "../../state/workspace";
import { toastError } from "../../state/toasts";
import {
  ipc,
  onChatStream,
  onExperimentLogLine,
  onExperimentStarting,
  onExperimentStatus,
  onResourcePending,
  onTabCreated,
} from "../../ipc";
import { MessageList } from "./MessageList";
import { Composer } from "./Composer";
import { TabBar } from "./TabBar";
import { HistoryPanel } from "./HistoryPanel";
import { ResourcePreviewPane, resourcePreviewTitle } from "./ResourcePreviewPane";
import { ExperimentTabView } from "./ExperimentTabView";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faMessage, faClockRotateLeft, iconClass } from "../../icons";
import type { AskAnswer, AskBlockState, ChatMessage, ChatOptions, Provider, Tab } from "../../types";

const seenStreamEventIds = new Map<string, number>();

const HERO_MESSAGES = [
  "advancing science...",
  "answering big questions...",
  "accelerating discovery...",
  "exploring the unknown...",
  "pushing knowledge forward...",
  "investigating new ideas...",
  "connecting the dots...",
  "uncovering new knowledge...",
  "discovering what matters...",
  "research without limits...",
  "think deeper...",
  "explore further...",
  "discover faster...",
];

let heroMessageIdx = Math.floor(Math.random() * HERO_MESSAGES.length);
function nextHeroMessage() {
  heroMessageIdx = (heroMessageIdx + 1) % HERO_MESSAGES.length;
  return HERO_MESSAGES[heroMessageIdx];
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
    setResourcePending,
    addTool,
    markToolDone,
    addAskQuestion,
    linkExperimentUuid,
    updateExperimentStatus,
    appendExperimentLog,
    setTabAgentOptions,
    setTabHistoryMeta,
    createTabWithMessages,
    openDraftPreviewTab,
  } = useChat();

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0];

  const [previewContent, setPreviewContent] = useState("");
  const [historyOpen, setHistoryOpen] = useState(false);
  const [heroMessage, setHeroMessage] = useState(() => HERO_MESSAGES[heroMessageIdx]);

  useEffect(() => {
    if (tabs.length > 0) return;
    const t = setInterval(() => setHeroMessage(nextHeroMessage()), 3500);
    return () => clearInterval(t);
  }, [tabs.length]);

  useEffect(() => {
    if (activeTab?.kind !== "preview") return;
    if (activeTab.draftContent) {
      setPreviewContent(activeTab.draftContent);
    } else if (activeTab.irePath) {
      ipc.readResource(activeTab.irePath).then((f) => {
        setPreviewContent(f.content);
        renameTab(activeTab.id, resourcePreviewTitle(f.content));
      });
    }
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
          void useWorkspace.getState().persist()
            .catch((e) => toastError("save state", e));
          break;

        case "Done": {
          if (msgId) finishMessage(tab_id, msgId);
          setStreaming(tab_id, false);
          assistantIdByTab.current.delete(tab_id);
          void persistChat(tab_id);
          void useWorkspace.getState().persist()
            .catch((e) => toastError("save state", e));

          const currentTab = useChat.getState().tabs.find((t) => t.id === tab_id);
          if (currentTab?.kind === "resource" && currentTab.resourceStatus === "summarizing") {
            setResourceStatus(tab_id, "ready");
          }
          break;
        }
      }
    }));

    reg(onTabCreated((payload) => {
      if (payload.agent_options) {
        useChatOptions.getState().setOptions(payload.agent_options);
      }
      const newTab: Tab = {
        id: payload.tab_id,
        label: payload.kind === "resource" ? "Ingest" : payload.label,
        messages: [],
        isStreaming: false,
        isPinned: false,
        kind: payload.kind,
        agentOptions: payload.agent_options,
        resourceId: payload.resource_id,
        resourceStatus:
          payload.kind === "resource"
            ? (payload.resource_status ?? "summarizing")
            : undefined,
      };
      addTab(newTab);
      setActiveTab(payload.tab_id);
    }));

    reg(onResourcePending((payload) => {
      const currentTabId = useChat.getState().activeTabId;
      setResourcePending(currentTabId, payload.resource_id, payload.resource_status);
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

  // Persist a chat tab's messages to chat_sessions (the durable store). Streaming
  // tabs are skipped unless `allowStreaming` is set — used by the debounced
  // mid-stream save so an in-flight turn survives a crash.
  const persistChat = async (tabId: string, allowStreaming = false) => {
    const tab = useChat.getState().tabs.find((t) => t.id === tabId);
    if (!tab || tab.kind !== "chat" || tab.messages.length === 0) return;
    if (tab.isStreaming && !allowStreaming) return;
    const { uuid, startedAt } = getSessionMeta(tabId);
    const savedOptions = tab.agentOptions ?? useChatOptions.getState();
    await ipc
      .chatHistorySave(tab.label, savedOptions.provider, savedOptions.model, startedAt, JSON.stringify(tab.messages), uuid)
      .catch((e) => toastError("save chat history", e));
  };

  // Debounced mid-stream save: persist the streaming tab ~1s after the last
  // message change so an in-flight turn survives a crash. The Done/Error/close
  // paths still save synchronously.
  useEffect(() => {
    if (!activeTab || activeTab.kind !== "chat" || !activeTab.isStreaming) return;
    const id = activeTab.id;
    const handle = setTimeout(() => { void persistChat(id, true); }, 1000);
    return () => clearTimeout(handle);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab?.id, activeTab?.isStreaming, activeTab?.messages]);

  const handleSend = async (text: string) => {
    if (!activeTab || activeTab.isStreaming) return;

    // Auto-name a brand-new, still-default chat from its first message (best-effort).
    const shouldTitle =
      activeTab.kind === "chat" && activeTab.messages.length === 0 && activeTab.label === "Untitled";
    const titleTabId = activeTabId;

    if (activeTab.kind === "resource" && activeTab.resourceStatus === "ready") {
      setResourceStatus(activeTabId, "summarizing");
    }

    // Ensure a stable session UUID exists for this tab before the first message.
    const { uuid: sessionUuid, startedAt } = getSessionMeta(activeTabId);
    const tabLabel = activeTab.label;
    setTabAgentOptions(activeTabId, { model, provider, effort });

    addUserMessage(activeTabId, text);
    const aid = beginAssistantMessage(activeTabId);
    assistantIdByTab.current.set(activeTabId, aid);
    setStreaming(activeTabId, true);

    if (shouldTitle) {
      ipc
        // OpenCode has no fixed "lightweight" model to fall back to (its
        // catalog is dynamic) — the chat's own selected model is already
        // guaranteed non-empty by Composer's send guard, so reuse it.
        .generateChatTitle(text, provider === "opencode" ? model : lightweightModelForProvider(provider), provider)
        .then((title) => { if (title) renameTab(titleTabId, title); })
        .catch(() => {}); // best-effort; leave label as "Untitled" on failure
    }

    try {
      await useWorkspace
        .getState()
        .persist()
        .catch((e) => toastError("save chat options", e));
      await ipc.chatSend(activeTabId, text, { model, provider, effort }, sessionUuid, tabLabel, startedAt);
    } catch (err) {
      const currentMsgId = assistantIdByTab.current.get(activeTabId);
      if (currentMsgId) setMessageError(activeTabId, currentMsgId, String(err));
      assistantIdByTab.current.delete(activeTabId);
      setStreaming(activeTabId, false);
      void persistChat(activeTabId);
      void useWorkspace.getState().persist()
        .catch((e) => toastError("save state", e));
    }
  };

  const handleAskSubmit = (_ask: AskBlockState, answers: AskAnswer[]) => {
    void ipc.submitAskAnswer(activeTabId, answers).catch((e) => toastError("submit answer", e));
  };

  const handleNewTab = () => {
    if (tabs.length === 0) {
      setHeroMessage(nextHeroMessage());
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
      await persistChat(tabId);
      sessionUuidByTab.current.delete(tabId);
      sessionStartedAtByTab.current.delete(tabId);
    }
    closeTab(tabId);
    void useWorkspace.getState().persist()
      .catch((e) => toastError("save state", e));
  };

  const clearResourceFromTab = () => {
    setResourcePending(activeTabId, undefined, undefined);
  };

  const handleConfirmResource = async () => {
    if (!activeTab.resourceId) return;
    setResourceStatus(activeTabId, "confirmed");
    try {
      await ipc.confirmResource(activeTab.resourceId);
      if (activeTab.kind === "resource") {
        closeTab(activeTabId);
      } else {
        clearResourceFromTab();
      }
    } catch (err) {
      toastError("confirm resource", String(err));
      setResourceStatus(activeTabId, "ready");
    }
  };

  const handleViewDraft = async () => {
    if (!activeTab.resourceId) return;
    try {
      const content = await ipc.readResourceDraft(activeTab.resourceId);
      openDraftPreviewTab(resourcePreviewTitle(content), content, activeTab.resourceId);
    } catch {
      toastError("view draft", "Draft not ready yet — the agent may still be writing.");
    }
  };

  const handleRestore = async (sessionUuid: string, tabLabel: string, startedAt?: string, provider?: string, model?: string) => {
    const json = await ipc
      .chatHistoryGet(sessionUuid)
      .catch((e) => { toastError("load history", e); return null; });
    if (!json) return;
    const messages: ChatMessage[] = JSON.parse(json);
    // chat_sessions is the live store: reopening just binds a tab to the existing
    // row (keeping its resume id) — no delete. excludeSessionUuids hides it from
    // the history panel while it is open.
    const agentOptions: ChatOptions | undefined = provider && model
      ? {
          provider: provider as Provider,
          model,
          effort: defaultEffortForModel(provider as Provider, model),
        }
      : undefined;
    createTabWithMessages(tabLabel, messages, sessionUuid, startedAt, agentOptions);
  };

  const handleDiscardResource = () => {
    if (activeTab.resourceId) {
      ipc.discardResource(activeTab.resourceId).catch((e) => toastError("discard resource", e));
    }
    if (activeTab.kind === "resource") {
      closeTab(activeTabId);
    } else {
      clearResourceFromTab();
    }
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
            className="app-icon-button h-8 w-8"
            title="Chat history"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={() => setHistoryOpen((o) => !o)}
          >
            <FontAwesomeIcon icon={faClockRotateLeft} className={iconClass.lg} />
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
    activeTab?.resourceId != null && activeTab.resourceStatus === "ready";

  if (tabs.length === 0) {
    return (
      <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
        {tabBar}
        <div className="relative flex-1 flex flex-col items-center justify-center text-center px-10">
          <div className="hero-grid absolute inset-0 pointer-events-none" />
          <div className="relative z-[1] flex flex-col items-center">
            <svg
              viewBox="0 310 1024 480"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
              className="select-none"
              style={{ width: 'clamp(200px, 27vmin, 480px)' }}
            >
              <defs>
                <linearGradient id="ireMarkGrad" gradientUnits="userSpaceOnUse" x1="20" y1="10" x2="320" y2="150">
                  <stop offset="0%" stopColor="#3d3d48" />
                  <stop offset="60%" stopColor="#252530" />
                  <stop offset="100%" stopColor="#323240" />
                </linearGradient>
              </defs>
              <g transform="translate(153.6 340.8) scale(2.1397)">
                <g strokeWidth="14" strokeLinecap="square" strokeLinejoin="miter" fill="none">
                  <line stroke="url(#ireMarkGrad)" x1="50" y1="14" x2="50" y2="146" />
                  <path stroke="url(#ireMarkGrad)" d="M 110,14 L 140,14 C 175,14 175,80 140,80 L 110,80 L 168,146" />
                  <line stroke="#2a2a36" x1="215" y1="20" x2="295" y2="20" />
                  <line stroke="#2a2a36" x1="215" y1="80" x2="295" y2="80" />
                  <line stroke="#2a2a36" x1="215" y1="140" x2="295" y2="140" />
                </g>
              </g>
            </svg>
            <p className="font-mono text-[11px] tracking-[0.09em] text-outline mt-2.5">
              Integrated Research Environment
            </p>
            <div className="w-full h-px bg-outline-variant mt-5" />
            <p
              key={heroMessage}
              className="font-mono text-[11px] tracking-[0.09em] text-outline mt-5"
              style={{ animation: 'hero-tagline-in 400ms ease forwards' }}
            >
              {heroMessage}
            </p>
          </div>
          <button
            id="ire-new-chat-btn"
            className="relative z-[1] inline-flex items-center gap-2 bg-on-surface text-background text-[12px] px-4 py-1.5 rounded-lg hover:opacity-85 transition-opacity mt-10"
            onClick={handleNewTab}
          >
            <FontAwesomeIcon icon={faMessage} className={iconClass.md} />
            New chat
          </button>
        </div>
      </section>
    );
  }

  if (activeTab.kind === "preview") {
    const handleSaveResource = async (content: string) => {
      if (activeTab.irePath) {
        await ipc.saveResource(activeTab.irePath, content).catch((e) => toastError("save resource", e));
      } else if (activeTab.resourceId) {
        await ipc.saveResourceDraft(activeTab.resourceId, content).catch((e) => toastError("save draft", e));
      }
      setPreviewContent(content);
    };

    return (
      <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
        {tabBar}
        <div className="flex-1 overflow-hidden relative">
          <ResourcePreviewPane title={activeTab.label} content={previewContent} onSave={handleSaveResource} />
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
        {/* Floating composer + resource bar */}
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2 w-full px-6 flex flex-col gap-2 pointer-events-none" style={{ zIndex: 20 }}>
          <div className="contents">
            {showResourceBar && (
              <div className="flex justify-center pointer-events-auto">
                <div className="flex bg-surface-container border border-outline-variant rounded-lg shadow-lg shadow-black/30 overflow-hidden">
                  <button
                    className="flex items-center gap-1.5 px-4 h-9 text-[11px] font-mono text-on-surface font-medium hover:bg-surface-container-high transition-colors"
                    onClick={handleViewDraft}
                  >
                    <svg width="11" height="11" viewBox="0 0 16 16" fill="none" className="opacity-60"><path d="M2 8s2.5-5 6-5 6 5 6 5-2.5 5-6 5-6-5-6-5z" stroke="currentColor" strokeWidth="1.5" fill="none"/><circle cx="8" cy="8" r="2" stroke="currentColor" strokeWidth="1.5"/></svg>
                    View Summary
                  </button>
                  <div className="w-px self-stretch bg-outline-variant my-1.5" />
                  <button
                    className="flex items-center gap-1.5 px-4 h-9 text-[11px] font-mono text-ok/80 hover:bg-ok/5 transition-colors"
                    onClick={handleConfirmResource}
                  >
                    <svg width="10" height="10" viewBox="0 0 16 16" fill="none" className="opacity-70"><path d="M3 8.5l3.5 3.5 6.5-7" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"/></svg>
                    Approve
                  </button>
                  <div className="w-px self-stretch bg-outline-variant my-1.5" />
                  <button
                    className="flex items-center gap-1.5 px-4 h-9 text-[11px] font-mono text-on-surface-variant hover:text-error hover:bg-error/5 transition-colors"
                    onClick={handleDiscardResource}
                  >
                    <svg width="10" height="10" viewBox="0 0 16 16" fill="none" className="opacity-60"><path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/></svg>
                    Discard
                  </button>
                </div>
              </div>
            )}
            <div className="pointer-events-auto">
              <Composer onSend={handleSend} disabled={activeTab.isStreaming} onCancel={() => ipc.chatCancel(activeTabId)} />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
