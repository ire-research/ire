import { useEffect, useRef, useState } from "react";
import type { AssistantMessage, ChatMessage } from "../../types";

interface MessageListProps {
  messages: ChatMessage[];
}

export function MessageList({ messages }: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  if (messages.length === 0) {
    return (
      <div className="messages messages--empty">
        <p>Start a conversation. Brainstorm ideas or kick off an experiment.</p>
      </div>
    );
  }

  return (
    <div className="messages">
      {messages.map((m) =>
        m.role === "user" ? (
          <div key={m.id} className="message message--user">
            <div className="message__role">You</div>
            <div className="message__text">{m.text}</div>
          </div>
        ) : (
          <AssistantBubble key={m.id} msg={m as AssistantMessage} />
        )
      )}
      <div ref={bottomRef} />
    </div>
  );
}

function AssistantBubble({ msg }: { msg: AssistantMessage }) {
  const [thinkingOpen, setThinkingOpen] = useState(false);
  const thinkingRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (msg.isStreaming && thinkingOpen && thinkingRef.current) {
      thinkingRef.current.scrollTop = thinkingRef.current.scrollHeight;
    }
  }, [msg.thinking, msg.isStreaming, thinkingOpen]);

  return (
    <div className="message message--assistant">
      <div className="message__role">Claude</div>

      {msg.thinking && (
        <div className="thinking-block">
          <button
            className="thinking-block__toggle"
            onClick={() => setThinkingOpen((v) => !v)}
          >
            {thinkingOpen ? "▾" : "▸"} Thinking
          </button>
          {thinkingOpen && (
            <div ref={thinkingRef} className="thinking-block__content">
              {msg.thinking}
            </div>
          )}
        </div>
      )}

      {msg.error ? (
        <div className="message__text message__error">{msg.error}</div>
      ) : msg.text ? (
        <div className="message__text">{msg.text}</div>
      ) : msg.isStreaming ? (
        <div className="message__text">
          <span className="typing-dot" />
          <span className="typing-dot" />
          <span className="typing-dot" />
        </div>
      ) : null}
    </div>
  );
}
