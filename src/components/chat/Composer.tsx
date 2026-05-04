import { useState } from "react";

interface ComposerProps {
  onSend?: (text: string) => void;
  disabled?: boolean;
}

export function Composer({ onSend, disabled }: ComposerProps) {
  const [text, setText] = useState("");

  const handleSend = () => {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSend?.(trimmed);
    setText("");
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="composer">
      <textarea
        placeholder="Message Claude…"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
      />
      <div className="composer__footer">
        <span className="composer__hint">⌘↵ to send</span>
        <button onClick={handleSend} disabled={!text.trim() || disabled}>
          Send
        </button>
      </div>
    </div>
  );
}
