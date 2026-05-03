import { useState } from "react";
import { useWorkspace } from "../../state/workspace";
import type { ChatMessage } from "../../types";
import { MessageList } from "./MessageList";
import { Composer } from "./Composer";

let nextId = 0;

export function ChatPane() {
  const mode = useWorkspace((s) => s.mode);
  const setMode = useWorkspace((s) => s.setMode);
  const [messages, setMessages] = useState<ChatMessage[]>([]);

  const handleSend = (text: string) => {
    setMessages((prev) => [
      ...prev,
      { id: String(nextId++), role: "user", text },
    ]);
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
      </header>
      <MessageList messages={messages} />
      <Composer onSend={handleSend} />
    </section>
  );
}
