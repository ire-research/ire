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
  const textareaRef = useRef<HTMLTextAreaElement>(null);

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

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
    el.style.overflowY = el.scrollHeight > 240 ? "auto" : "hidden";
  }, [text]);

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
    <div className="bg-surface-container border border-outline-variant rounded-lg shadow-lg shadow-black/30 flex flex-col overflow-visible">
      <textarea
        ref={textareaRef}
        id="composer-textarea"
        className="w-full bg-transparent border-none text-on-surface text-[14px] focus:ring-0 px-3 py-2.5 placeholder-on-surface-variant/50 outline-none resize-none"
        placeholder="Message IRE…"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
      />
      <div className="flex items-center justify-between px-2 pb-2 pt-0">
        {/* Left: model + effort + slash */}
        <div className="flex items-center gap-1">
          {/* Model picker */}
          <div className="relative" ref={modelRef}>
            <button
              className="flex items-center gap-1 px-2 py-1 text-on-surface-variant hover:bg-surface-container-high rounded hover:text-on-surface transition-colors text-[11px] border border-outline-variant/50"
              onClick={() => setModelOpen((o) => !o)}
            >
              <span className="text-[10px] text-on-surface-variant/60 mr-0.5">model</span>
              {modelLabel}
              <span className="material-symbols-outlined text-[12px]">expand_more</span>
            </button>
            <div className={`${modelOpen ? "block" : "hidden"} absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 py-1 min-w-[140px] z-50`}>
              {MODELS.map((m) => (
                <button
                  key={m.id}
                  className={`w-full text-left px-3 py-1.5 text-[12px] hover:bg-surface-container-highest transition-colors ${m.id === model ? "font-medium text-on-surface" : "text-on-surface-variant"}`}
                  onClick={() => { setModel(m.id); setModelOpen(false); }}
                >
                  {m.label}
                </button>
              ))}
            </div>
          </div>
          {/* Effort picker */}
          <div className="relative" ref={effortRef}>
            <button
              className="flex items-center gap-1 px-2 py-1 text-on-surface-variant hover:bg-surface-container-high rounded hover:text-on-surface transition-colors text-[11px] border border-outline-variant/50"
              onClick={() => setEffortOpen((o) => !o)}
            >
              <span className="text-[10px] text-on-surface-variant/60 mr-0.5">effort</span>
              {effortLabel}
              <span className="material-symbols-outlined text-[12px]">expand_more</span>
            </button>
            <div className={`${effortOpen ? "block" : "hidden"} absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 py-1 min-w-[140px] z-50`}>
              {EFFORT_LEVELS.map((lvl) => (
                <button
                  key={lvl.value}
                  className={`w-full text-left px-3 py-1.5 text-[12px] hover:bg-surface-container-highest transition-colors ${lvl.value === effort ? "font-medium text-on-surface" : "text-on-surface-variant"}`}
                  onClick={() => { setEffort(lvl.value); setEffortOpen(false); }}
                >
                  {lvl.label}
                </button>
              ))}
            </div>
          </div>
        </div>
        {/* Right: hint + send */}
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-on-surface-variant/50">⌘↵</span>
          <button
            className="bg-accent text-accent-fg px-4 py-1 rounded text-[12px] font-medium hover:opacity-90 transition-opacity flex items-center gap-1 disabled:opacity-40"
            onClick={handleSend}
            disabled={!text.trim() || disabled}
          >
            Send <span className="material-symbols-outlined text-[14px]">arrow_upward</span>
          </button>
        </div>
      </div>
    </div>
  );
}
