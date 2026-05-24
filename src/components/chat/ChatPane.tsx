import { useEffect, useRef, useState } from "react";
import { useChat } from "../../state/chat";
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
import { ResourcePreviewPane } from "./ResourcePreviewPane";
import { ExperimentTabView } from "./ExperimentTabView";
import { Icon } from "../Icon";
import type { Tab } from "../../types";

export function ChatPane() {
  const { model, effort } = useChatOptions();

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
              input_full: event.input_full,
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
              // CC has written the wiki file; index it, then close the tab.
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
      await ipc.chatSend(activeTabId, text, { model, effort });
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
      await ipc.chatSend(activeTabId, prompt, { model, effort });
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

  const showResourceBar =
    activeTab.kind === "resource" && activeTab.resourceStatus === "ready";

  if (activeTab.kind === "preview") {
    return (
      <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
        <TabBar tabs={tabs} activeTabId={activeTabId} onSelect={setActiveTab} onClose={handleCloseTab} onNew={handleNewTab} onRename={renameTab} />
        <div className="flex-1 overflow-hidden relative">
          <ResourcePreviewPane title={activeTab.label} content={previewContent} />
        </div>
      </section>
    );
  } else if (activeTab.kind === "experiment") {
    return (
      <section className="flex flex-col h-full min-h-0 overflow-hidden bg-background">
        <TabBar tabs={tabs} activeTabId={activeTabId} onSelect={setActiveTab} onClose={handleCloseTab} onNew={handleNewTab} onRename={renameTab} />
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
      <TabBar tabs={tabs} activeTabId={activeTabId} onSelect={setActiveTab} onClose={handleCloseTab} onNew={handleNewTab} onRename={renameTab} />
      <div className="flex items-center justify-end px-4 h-8 shrink-0 border-b border-outline-variant/30">
        {activeTab.isStreaming && (
          <button className="text-on-surface-variant hover:text-on-surface transition-colors text-xs" onClick={() => ipc.chatCancel(activeTabId)}>
            Cancel
          </button>
        )}
        {!activeTab.isStreaming && (
          <button
            className="text-on-surface-variant hover:text-on-surface transition-colors p-1"
            title="Reset conversation"
            onClick={() => {
              clearMessages(activeTabId);
              ipc.chatResetSession(activeTabId);
            }}
          >
            <Icon name="refresh" className="w-[16px] h-[16px]" />
          </button>
        )}
      </div>
      <div className="flex-1 overflow-hidden relative">
        {/* Chat view */}
        <div className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-4 pb-40 space-y-6">
          <MessageList messages={activeTab.messages} />
        </div>
        {/* Floating composer */}
        <div className="absolute bottom-6 left-1/2 -translate-x-1/2 w-full px-6 pointer-events-none" style={{ zIndex: 20 }}>
          <div className="pointer-events-auto">
            {!showResourceBar && (
              <Composer onSend={handleSend} disabled={activeTab.isStreaming} />
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
