import { useEffect, useRef, useState } from "react";
import { useWorkspace } from "../../state/workspace";
import { useChat, MAIN_TAB_ID } from "../../state/chat";
import { useChatOptions } from "../../state/chatOptions";
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
import { MarkdownPane } from "../MarkdownPane";
import type { Tab } from "../../types";

export function ChatPane() {
  const mode = useWorkspace((s) => s.mode);
  const setMode = useWorkspace((s) => s.setMode);
  const { model, effort } = useChatOptions();

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
    addTool,
    markToolDone,
    linkExperimentUuid,
    updateExperimentStatus,
    appendExperimentLog,
  } = useChat();

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0];

  const [previewContent, setPreviewContent] = useState("");

  useEffect(() => {
    if (activeTab?.kind !== "preview" || !activeTab.wikiPath) return;
    ipc.readWikiFile(activeTab.wikiPath).then((f) => setPreviewContent(f.content));
  }, [activeTab?.id]);

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
    const unlisteners: (() => void)[] = [];

    function reg(p: Promise<() => void>) {
      p.then((u) => { if (cancelled) u(); else unlisteners.push(u); });
    }

    reg(onChatStream(({ tab_id, event }) => {
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
            addTool(tab_id, msgId, {
              tool_id: event.tool_id,
              tool_name: event.tool_name,
              input_preview: event.input_preview,
              output_preview: null,
              output_full: null,
              isDone: false,
            });
          }
          break;

        case "ToolDone":
          markToolDone(tab_id, event.tool_id, event.output_preview, event.output_full);
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
              // CC has written the wiki file — commit it then close the tab
              if (currentTab.resourceId) {
                ipc.indexResource(currentTab.resourceId).catch((e) => toastError("index resource", e));
              }
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

  const handleSend = async (text: string) => {
    if (activeTab.isStreaming) return;

    addUserMessage(activeTabId, text);
    const aid = beginAssistantMessage(activeTabId);
    assistantIdByTab.current.set(activeTabId, aid);
    setStreaming(activeTabId, true);

    try {
      await ipc.chatSend(activeTabId, text, mode, { model, effort });
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

  const handleConfirmResource = async () => {
    if (!activeTab.resourceId) return;
    setResourceStatus(activeTabId, "confirmed");
    const aid = beginAssistantMessage(activeTabId);
    assistantIdByTab.current.set(activeTabId, aid);
    setStreaming(activeTabId, true);
    try {
      const prompt = await ipc.getResourceConfirmPrompt();
      await ipc.chatSend(activeTabId, prompt, mode, { model, effort });
    } catch (err) {
      const msgId = assistantIdByTab.current.get(activeTabId);
      if (msgId) setMessageError(activeTabId, msgId, String(err));
      setStreaming(activeTabId, false);
    }
  };

  const handleDiscardResource = () => {
    if (activeTab.resourceId) {
      ipc.discardResource(activeTab.resourceId).catch((e) => toastError("discard resource", e));
    }
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
      {activeTab.kind === "preview" ? (
        <MarkdownPane
          title={activeTab.label}
          content={previewContent}
          showSubmit
          onSubmit={(content) =>
            ipc.saveWikiFile(activeTab.wikiPath!, content).catch((e) =>
              toastError("save wiki file", e)
            )
          }
        />
      ) : (
        <>
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
        </>
      )}
    </section>
  );
}
