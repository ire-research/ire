import { useRef } from "react";
import { useWorkspace } from "../../state/workspace";
import { useChat } from "../../state/chat";
import { ipc, onChatStream } from "../../ipc";
import { MessageList } from "./MessageList";
import { Composer } from "./Composer";

export function ChatPane() {
  const mode = useWorkspace((s) => s.mode);
  const setMode = useWorkspace((s) => s.setMode);
  const {
    messages,
    isStreaming,
    addUserMessage,
    beginAssistantMessage,
    appendText,
    appendThinking,
    finishMessage,
    setMessageError,
    setStreaming,
    clearMessages,
  } = useChat();

  const assistantIdRef = useRef<string | null>(null);

  const handleSend = async (text: string) => {
    if (isStreaming) return;

    addUserMessage(text);
    const aid = beginAssistantMessage();
    assistantIdRef.current = aid;
    setStreaming(true);

    // Subscribe before invoke to avoid missing early events
    const unlisten = await onChatStream((event) => {
      const id = assistantIdRef.current!;
      switch (event.kind) {
        case "TextDelta":
          appendText(id, event.text);
          break;
        case "ThinkingDelta":
          appendThinking(id, event.text);
          break;
        case "Result":
          if (event.text) appendText(id, event.text);
          break;
        case "Error":
          setMessageError(id, event.message);
          setStreaming(false);
          break;
        case "Done":
          finishMessage(id);
          setStreaming(false);
          break;
      }
    });

    try {
      await ipc.chatSend(text, mode);
    } catch (err) {
      setMessageError(assistantIdRef.current!, String(err));
    } finally {
      unlisten();
      // Belt-and-suspenders: ensure streaming state is always cleared even if
      // the subprocess exited without emitting Done (e.g. --resume failure).
      setStreaming(false);
      finishMessage(assistantIdRef.current!);
    }
  };

  return (
    <section className="chat-pane">
      <header className="chat-pane__header">
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
        <div className="topbar__spacer" />
        <div className="chat-pane__actions">
          {isStreaming && (
            <button onClick={() => ipc.chatCancel()}>Cancel</button>
          )}
          <button
            className="chat-pane__reset"
            title="New conversation"
            onClick={() => {
              clearMessages();
              ipc.chatResetSession();
            }}
            disabled={isStreaming}
          >
            ↺
          </button>
        </div>
      </header>
      <MessageList messages={messages} />
      <Composer onSend={handleSend} disabled={isStreaming} />
    </section>
  );
}
