import type { ChatMessage } from "../../types";

interface MessageListProps {
  messages: ChatMessage[];
}

export function MessageList({ messages }: MessageListProps) {
  if (messages.length === 0) {
    return (
      <div className="messages messages--empty">
        <p>Start a conversation. Brainstorm ideas or kick off an experiment.</p>
      </div>
    );
  }
  return (
    <div className="messages">
      {messages.map((m) => (
        <div key={m.id} className={`message message--${m.role}`}>
          <div className="message__role">{m.role}</div>
          <div className="message__text">{m.text}</div>
        </div>
      ))}
    </div>
  );
}
