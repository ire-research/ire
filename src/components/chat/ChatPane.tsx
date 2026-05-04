import { useEffect, useRef } from "react";
import { useWorkspace } from "../../state/workspace";
import { useChat, MAIN_TAB_ID } from "../../state/chat";
import { ipc, onChatStream, onTabCreated } from "../../ipc";
import { MessageList } from "./MessageList";
import { Composer } from "./Composer";
import { TabBar } from "./TabBar";
import type { Tab } from "../../types";

export function ChatPane() {
  const mode = useWorkspace((s) => s.mode);
  const setMode = useWorkspace((s) => s.setMode);

  const {
    tabs,
    activeTabId,
    addTab,
    createTab,
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
    clearMessages,
  } = useChat();

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0];

  // Maps tab_id → in-flight assistant message id.
  const assistantIdByTab = useRef<Map<string, string>>(new Map());

  // Global stream listener — routes events to the correct tab.
  //
  // Tauri's listen() is async; it returns a Promise<UnlistenFn>. React Strict
  // Mode runs cleanup synchronously before that Promise resolves, so the naive
  // pattern (store the fn, call it in cleanup) leaves the first listener alive
  // and registers a second one — every event fires twice. The `cancelled` flag
  // lets the .then() handler unlisten immediately when cleanup already ran.
  useEffect(() => {
    let cancelled = false;
    let unlistenStream: (() => void) | null = null;
    let unlistenTabCreated: (() => void) | null = null;

    onChatStream(({ tab_id, event }) => {
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

        case "Result":
          if (msgId && event.text) appendText(tab_id, msgId, event.text);
          break;

        case "Error":
          if (msgId) setMessageError(tab_id, msgId, event.message);
          setStreaming(tab_id, false);
          assistantIdByTab.current.delete(tab_id);
          break;

        case "Done": {
          if (msgId) finishMessage(tab_id, msgId);
          setStreaming(tab_id, false);
          assistantIdByTab.current.delete(tab_id);

          const currentTab = useChat.getState().tabs.find((t) => t.id === tab_id);
          if (currentTab?.kind === "resource") {
            if (currentTab.resourceStatus === "summarizing") {
              setResourceStatus(tab_id, "ready");
            } else if (currentTab.resourceStatus === "confirmed") {
              closeTab(tab_id);
            }
          }
          break;
        }
      }
    }).then((u) => { if (cancelled) u(); else unlistenStream = u; });

    onTabCreated((payload) => {
      const newTab: Tab = {
        id: payload.tab_id,
        label: payload.label,
        messages: [],
        isStreaming: false,
        isPinned: false,
        kind: payload.kind,
        resourceId: payload.resource_id,
        resourceStatus: payload.kind === "resource" ? "summarizing" : undefined,
      };
      addTab(newTab);
      setActiveTab(payload.tab_id);
    }).then((u) => { if (cancelled) u(); else unlistenTabCreated = u; });

    return () => {
      cancelled = true;
      unlistenStream?.();
      unlistenTabCreated?.();
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleSend = async (text: string) => {
    if (activeTab.isStreaming) return;

    addUserMessage(activeTabId, text);
    const aid = beginAssistantMessage(activeTabId);
    assistantIdByTab.current.set(activeTabId, aid);
    setStreaming(activeTabId, true);

    try {
      await ipc.chatSend(activeTabId, text, mode);
    } catch (err) {
      const currentMsgId = assistantIdByTab.current.get(activeTabId);
      if (currentMsgId) setMessageError(activeTabId, currentMsgId, String(err));
    } finally {
      // Belt-and-suspenders: clear streaming state even if Done never fires.
      const currentMsgId = assistantIdByTab.current.get(activeTabId);
      if (currentMsgId) finishMessage(activeTabId, currentMsgId);
      assistantIdByTab.current.delete(activeTabId);
      setStreaming(activeTabId, false);
    }
  };

  const handleNewTab = () => {
    createTab();
  };

  const handleCloseTab = (tabId: string) => {
    if (activeTab.isStreaming && tabId === activeTabId) {
      ipc.chatCancel(tabId);
    }
    closeTab(tabId);
  };

  const handleConfirmResource = () => {
    if (!activeTab.resourceId) return;
    setResourceStatus(activeTabId, "confirmed");
    // Kick a follow-up CC turn to write the wiki file.
    const aid = beginAssistantMessage(activeTabId);
    assistantIdByTab.current.set(activeTabId, aid);
    setStreaming(activeTabId, true);
    ipc.chatSend(
      activeTabId,
      "The user approved this resource. Write a wiki page to resources/ using the wiki.write MCP tool. Frontmatter: url, date. Body: the summary from your previous response.",
      mode
    ).catch((err) => {
      const msgId = assistantIdByTab.current.get(activeTabId);
      if (msgId) setMessageError(activeTabId, msgId, String(err));
      setStreaming(activeTabId, false);
    });
  };

  const handleDiscardResource = () => {
    closeTab(activeTabId);
  };

  const isMainTab = activeTab.id === MAIN_TAB_ID;
  const showResourceBar =
    activeTab.kind === "resource" && activeTab.resourceStatus === "ready";

  return (
    <section className="chat-pane">
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onSelect={setActiveTab}
        onClose={handleCloseTab}
        onNew={handleNewTab}
      />
      <header className="chat-pane__header">
        {isMainTab && (
          <div className="chat-pane__mode">
            <button
              className={mode === "brainstorm" ? "active" : ""}
              onClick={() => setMode("brainstorm")}
            >
              Brainstorm
            </button>
            <button
              className={mode === "experiment" ? "active" : ""}
              onClick={() => setMode("experiment")}
            >
              Experiment
            </button>
          </div>
        )}
        <div className="topbar__spacer" />
        <div className="chat-pane__actions">
          {activeTab.isStreaming && (
            <button onClick={() => ipc.chatCancel(activeTabId)}>Cancel</button>
          )}
          {!activeTab.isStreaming && (
            <button
              className="chat-pane__reset"
              title="Reset conversation"
              onClick={() => {
                clearMessages(activeTabId);
                ipc.chatResetSession(activeTabId);
              }}
            >
              ↺
            </button>
          )}
        </div>
      </header>
      <MessageList messages={activeTab.messages} />
      {showResourceBar && (
        <div className="chat-pane__resource-bar">
          <button className="chat-pane__confirm" onClick={handleConfirmResource}>
            Confirm — save to wiki
          </button>
          <button className="chat-pane__discard" onClick={handleDiscardResource}>
            Discard
          </button>
        </div>
      )}
      {!showResourceBar && (
        <Composer onSend={handleSend} disabled={activeTab.isStreaming} />
      )}
    </section>
  );
}
