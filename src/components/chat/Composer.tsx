import { useEffect, useRef, useState } from "react";
import {
  CLAUDE_EFFORT_LEVELS,
  CODEX_EFFORT_LEVELS,
  MODELS,
  type ModelEntry,
  type Provider,
  useChatOptions,
} from "../../state/chatOptions";
import { Icon } from "../Icon";

interface ComposerProps {
  onSend?: (text: string) => void;
  disabled?: boolean;
}

const PLACEHOLDER_SENTENCES = [
  "Advancing science...",
  "Answering big questions...",
  "Accelerating discovery...",
  "Exploring the unknown...",
  "Pushing knowledge forward...",
  "Investigating new ideas...",
  "Connecting the dots...",
  "Uncovering new knowledge...",
  "Discovering what matters...",
  "Research without limits...",
  "Think deeper...",
  "Explore further...",
  "Discover faster...",
];

export function Composer({ onSend, disabled }: ComposerProps) {
  const [text, setText] = useState("");
  const [placeholder] = useState(
    () => PLACEHOLDER_SENTENCES[Math.floor(Math.random() * PLACEHOLDER_SENTENCES.length)],
  );
  const [modelOpen, setModelOpen] = useState(false);
  const [effortOpen, setEffortOpen] = useState(false);
  const modelRef = useRef<HTMLDivElement>(null);
  const effortRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const { model, provider, effort, availableProviders, setModel, setEffort } = useChatOptions();
  const modelLabel = MODELS.find((m) => m.id === model)?.label ?? model;
  const effortLevels = provider === "codex" ? CODEX_EFFORT_LEVELS : CLAUDE_EFFORT_LEVELS;
  const effortLabel = effortLevels.find((l) => l.value === effort)?.label ?? effort;
  const claudeModels = availableProviders.includes("claude")
    ? MODELS.filter((m) => m.provider === "claude")
    : [];
  const codexModels = availableProviders.includes("codex")
    ? MODELS.filter((m) => m.provider === "codex")
    : [];

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

  const handleModelSelect = (entry: ModelEntry) => {
    setModel(entry.id, entry.provider);
    setModelOpen(false);
  };

  const toggleModelOpen = () => {
    setModelOpen((open) => !open);
    setEffortOpen(false);
  };

  const toggleEffortOpen = () => {
    setEffortOpen((open) => !open);
    setModelOpen(false);
  };

  const ProviderSection = ({
    label,
    provider: sectionProvider,
    models,
  }: {
    label: string;
    provider: Provider;
    models: ModelEntry[];
  }) => (
    <div className="py-2 first:pt-2 last:pb-2 border-t border-outline-variant/50 first:border-t-0">
      <div className="px-3 pb-1.5 text-[10px] font-medium uppercase tracking-normal text-on-surface-variant/60">
        {label}
      </div>
      {models.map((entry) => (
        <button
          key={entry.id}
          className={`w-full grid grid-cols-[18px_1fr_14px] items-center gap-2 px-3 py-1.5 text-left text-[12px] hover:bg-surface-container-highest transition-colors ${
            entry.id === model ? "font-medium text-on-surface" : "text-on-surface-variant"
          }`}
          onClick={() => handleModelSelect(entry)}
        >
          <i className={`fa-brands ${sectionProvider === "claude" ? "fa-claude" : "fa-openai"} text-[12px] text-on-surface-variant/80 text-center`} />
          <span>{entry.label}</span>
          <span className="text-[11px] text-primary">{entry.id === model ? "✓" : ""}</span>
        </button>
      ))}
    </div>
  );

  return (
    <div className="bg-surface-container border border-outline-variant rounded-lg shadow-lg shadow-black/30 flex flex-col overflow-visible">
      <textarea
        ref={textareaRef}
        id="composer-textarea"
        className="w-full bg-transparent border-none text-on-surface text-[14px] focus:ring-0 px-3 py-2.5 placeholder-on-surface-variant/50 outline-none resize-none"
        placeholder={placeholder}
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
              onClick={toggleModelOpen}
            >
              <span className="text-[10px] text-on-surface-variant/60 mr-0.5">model</span>
              {modelLabel}
              <Icon name="expand_more" className="w-[12px] h-[12px]" />
            </button>
            <div className={`${modelOpen ? "block" : "hidden"} absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 min-w-[230px] overflow-hidden z-50`}>
              {claudeModels.length > 0 && (
                <ProviderSection label="Claude Code" provider="claude" models={claudeModels} />
              )}
              {codexModels.length > 0 && (
                <ProviderSection label="Codex" provider="codex" models={codexModels} />
              )}
            </div>
          </div>
          {/* Effort picker */}
          <div className="relative" ref={effortRef}>
            <button
              className="flex items-center gap-1 px-2 py-1 text-on-surface-variant hover:bg-surface-container-high rounded hover:text-on-surface transition-colors text-[11px] border border-outline-variant/50"
              onClick={toggleEffortOpen}
            >
              <span className="text-[10px] text-on-surface-variant/60 mr-0.5">
                {provider === "codex" ? "reasoning" : "effort"}
              </span>
              {effortLabel}
              <Icon name="expand_more" className="w-[12px] h-[12px]" />
            </button>
            <div className={`${effortOpen ? "block" : "hidden"} absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 min-w-[140px] overflow-hidden z-50`}>
              <div className="px-3 pt-2 pb-1.5 text-[10px] font-medium uppercase tracking-normal text-on-surface-variant/60">
                {provider === "codex" ? "Reasoning" : "Effort"}
              </div>
              {effortLevels.map((lvl) => (
                <button
                  key={lvl.value}
                  className={`w-full grid grid-cols-[1fr_14px] items-center gap-2 px-3 py-1.5 text-left text-[12px] hover:bg-surface-container-highest transition-colors ${lvl.value === effort ? "font-medium text-on-surface" : "text-on-surface-variant"}`}
                  onClick={() => { setEffort(lvl.value); setEffortOpen(false); }}
                >
                  <span>{lvl.label}</span>
                  <span className="text-[11px] text-primary">{lvl.value === effort ? "✓" : ""}</span>
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
            Send <Icon name="arrow_upward" className="w-[14px] h-[14px]" />
          </button>
        </div>
      </div>
    </div>
  );
}
