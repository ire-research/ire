import { useEffect, useRef, useState } from "react";
import { useChatOptions, MODELS, EFFORT_LEVELS } from "../../state/chatOptions";

interface ComposerProps {
  onSend?: (text: string) => void;
  disabled?: boolean;
}

export function Composer({ onSend, disabled }: ComposerProps) {
  const [text, setText] = useState("");
  const [modelOpen, setModelOpen] = useState(false);
  const [effortOpen, setEffortOpen] = useState(false);
  const modelRef = useRef<HTMLDivElement>(null);
  const effortRef = useRef<HTMLDivElement>(null);

  const { model, effort, setModel, setEffort } = useChatOptions();
  const modelLabel = MODELS.find((m) => m.id === model)?.label ?? model;
  const effortLabel = EFFORT_LEVELS.find((l) => l.value === effort)?.label ?? effort;

  useEffect(() => {
    if (!modelOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (modelRef.current && !modelRef.current.contains(e.target as Node)) {
        setModelOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [modelOpen]);

  useEffect(() => {
    if (!effortOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (effortRef.current && !effortRef.current.contains(e.target as Node)) {
        setEffortOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [effortOpen]);

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
        placeholder="Message IRE…"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
      />
      <div className="composer__footer">
        <span className="composer__option-label">model</span>
        <div className="composer__model" ref={modelRef}>
          <button
            className="composer__model-btn"
            onClick={() => setModelOpen((o) => !o)}
          >
            {modelLabel}
          </button>
          {modelOpen && (
            <div className="composer__dropdown">
              {MODELS.map((m) => (
                <button
                  key={m.id}
                  className={m.id === model ? "active" : ""}
                  onClick={() => { setModel(m.id); setModelOpen(false); }}
                >
                  {m.label}
                </button>
              ))}
            </div>
          )}
        </div>
        <span className="composer__option-label">effort</span>
        <div className="composer__model" ref={effortRef}>
          <button
            className="composer__model-btn"
            onClick={() => setEffortOpen((o) => !o)}
          >
            {effortLabel}
          </button>
          {effortOpen && (
            <div className="composer__dropdown">
              {EFFORT_LEVELS.map((lvl) => (
                <button
                  key={lvl.value}
                  className={lvl.value === effort ? "active" : ""}
                  onClick={() => { setEffort(lvl.value); setEffortOpen(false); }}
                >
                  {lvl.label}
                </button>
              ))}
            </div>
          )}
        </div>
        <span className="composer__hint">⌘↵ to send</span>
        <button onClick={handleSend} disabled={!text.trim() || disabled}>
          Send
        </button>
      </div>
    </div>
  );
}
