import { useEffect, useRef, useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faLink, faXmark, faCircleExclamation, faChevronDown, faClaude, faOpenai, iconClass } from "../icons";
import { ipc, pickResourceFiles, type ResourceSourceInput } from "../ipc";
import { useChat } from "../state/chat";
import { MODELS, effortLevelsForModel, useChatOptions, type ModelEntry, type Provider } from "../state/chatOptions";

type QueuedSource =
  | { id: string; kind: "url"; url: string }
  | { id: string; kind: "local_file"; path: string };

function sourceKey(source: QueuedSource): string {
  return source.kind === "url" ? `url:${source.url}` : `file:${source.path}`;
}

function sourceDisplay(source: QueuedSource): string {
  return source.kind === "url" ? source.url : source.path;
}

function toInput(source: QueuedSource): ResourceSourceInput {
  return source.kind === "url"
    ? { kind: "url", url: source.url }
    : { kind: "local_file", path: source.path };
}

function parseFailedSourceIndex(message: string): number | null {
  const match = message.match(/^source (\d+): /);
  if (!match) return null;
  const parsed = Number(match[1]);
  return Number.isFinite(parsed) ? parsed - 1 : null;
}

interface Props {
  onClose: () => void;
}

export function AddResourceModal({ onClose }: Props) {
  const { model: globalModel, provider: globalProvider, effort: globalEffort, availableProviders } = useChatOptions();

  const [url, setUrl] = useState("");
  const [sources, setSources] = useState<QueuedSource[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [failedIndex, setFailedIndex] = useState<number | null>(null);

  // Local model/effort — initialized from the composer's current values.
  const [selectedModel, setSelectedModel] = useState(globalModel);
  const [selectedProvider, setSelectedProvider] = useState<Provider>(globalProvider);
  const [selectedEffort, setSelectedEffort] = useState(globalEffort);
  const [modelOpen, setModelOpen] = useState(false);
  const [effortOpen, setEffortOpen] = useState(false);
  const modelRef = useRef<HTMLDivElement>(null);
  const effortRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (availableProviders.length === 0 || availableProviders.includes(selectedProvider)) return;
    const provider = availableProviders[0];
    const entry = MODELS.find((m) => m.provider === provider);
    if (!entry) return;
    setSelectedProvider(provider);
    setSelectedModel(entry.id);
    const levels = effortLevelsForModel(provider, entry.id);
    setSelectedEffort(levels[0]?.value ?? null);
  }, [availableProviders, selectedProvider]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  useEffect(() => {
    if (!modelOpen) return;
    const handler = (e: MouseEvent) => {
      if (modelRef.current && !modelRef.current.contains(e.target as Node)) setModelOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [modelOpen]);

  useEffect(() => {
    if (!effortOpen) return;
    const handler = (e: MouseEvent) => {
      if (effortRef.current && !effortRef.current.contains(e.target as Node)) setEffortOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [effortOpen]);

  const clearError = () => { setError(null); setFailedIndex(null); };

  const addSources = (next: QueuedSource[]) => {
    setSources((current) => {
      const seen = new Set(current.map(sourceKey));
      const deduped = next.filter((s) => {
        const key = sourceKey(s);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      return [...current, ...deduped];
    });
  };

  const handleAddUrl = () => {
    if (loading) return;
    const trimmed = url.trim();
    if (!trimmed) return;
    clearError();
    addSources([{ id: crypto.randomUUID(), kind: "url", url: trimmed }]);
    setUrl("");
  };

  const handlePickFiles = async () => {
    if (loading) return;
    clearError();
    try {
      const paths = await pickResourceFiles();
      addSources(paths.map((path) => ({ id: crypto.randomUUID(), kind: "local_file", path })));
    } catch (e) {
      setError(String(e));
    }
  };

  const removeSource = (id: string) => {
    clearError();
    setSources((current) => current.filter((s) => s.id !== id));
  };

  const handleSubmit = async () => {
    if (loading || sources.length === 0) return;
    setLoading(true);
    clearError();
    try {
      const labels = sources.map(sourceDisplay);
      await ipc.submitResources(sources.map(toInput), { model: selectedModel, provider: selectedProvider, effort: selectedEffort });
      const { activeTabId, addUserMessage } = useChat.getState();
      addUserMessage(activeTabId, `Ingest ${labels.map((l) => `"${l}"`).join(", ")}`);
      onClose();
    } catch (e) {
      const message = String(e);
      setError(message);
      setFailedIndex(parseFailedSourceIndex(message));
    } finally {
      setLoading(false);
    }
  };

  const handleModelSelect = (entry: ModelEntry) => {
    setSelectedModel(entry.id);
    setSelectedProvider(entry.provider);
    const levels = effortLevelsForModel(entry.provider, entry.id);
    setSelectedEffort(levels.some((l) => l.value === selectedEffort) ? selectedEffort : (levels[0]?.value ?? null));
    setModelOpen(false);
  };

  const noProvidersAvailable = availableProviders.length === 0;
  const effortLevels = effortLevelsForModel(selectedProvider, selectedModel);
  const modelLabel = noProvidersAvailable ? "n/a" : MODELS.find((m) => m.id === selectedModel)?.label ?? selectedModel;
  const effortLabel = effortLevels.find((l) => l.value === selectedEffort)?.label ?? selectedEffort;
  const showEffortPicker = !noProvidersAvailable && effortLevels.length > 0;
  const claudeModels = availableProviders.includes("claude") ? MODELS.filter((m) => m.provider === "claude") : [];
  const codexModels = availableProviders.includes("codex") ? MODELS.filter((m) => m.provider === "codex") : [];
  const hasQueue = sources.length > 0;

  const ProviderSection = ({ label, provider: sectionProvider, models }: { label: string; provider: Provider; models: ModelEntry[] }) => (
    <div className="py-2 first:pt-2 last:pb-2 border-t border-outline-variant/50 first:border-t-0">
      <div className="px-3 pb-1.5 text-[10px] font-medium uppercase tracking-normal text-on-surface-variant/60">{label}</div>
      {models.map((entry) => (
        <button
          key={entry.id}
          className={`w-full grid grid-cols-[18px_1fr_14px] items-center gap-2 px-3 py-1.5 text-left text-[12px] hover:bg-surface-container-highest transition-colors ${entry.id === selectedModel ? "font-medium text-on-surface" : "text-on-surface-variant"}`}
          onClick={() => handleModelSelect(entry)}
        >
          <FontAwesomeIcon icon={sectionProvider === "claude" ? faClaude : faOpenai} className="text-[12px] text-on-surface-variant/80 text-center" />
          <span>{entry.label}</span>
          <span className="text-[11px] text-primary">{entry.id === selectedModel ? "✓" : ""}</span>
        </button>
      ))}
    </div>
  );

  return (
    <div
      className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      {/* overflow-visible so upward dropdowns in the footer aren't clipped */}
      <div className="w-[360px] bg-surface-container border border-outline-variant rounded-lg flex flex-col overflow-visible shadow-2xl">

        {/* Header */}
        <div className="flex items-center gap-2 px-4 pt-3.5 pb-3 border-b border-outline-variant shrink-0">
          <FontAwesomeIcon icon={faLink} className={`${iconClass.lg} shrink-0 text-on-surface-variant`} />
          <span className="flex-1 text-[13px] font-medium text-on-surface">Add resources</span>
          <button onClick={onClose} className="app-icon-button w-6 h-6">
            <FontAwesomeIcon icon={faXmark} className={iconClass.md} />
          </button>
        </div>

        {/* Inputs */}
        <div className="px-4 pt-3.5 pb-3.5 flex flex-col gap-2.5">
          <div className="flex items-center gap-2">
            <input
              type="text"
              placeholder="https://arxiv.org/abs/…"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); handleAddUrl(); } }}
              disabled={loading}
              // biome-ignore lint/a11y/noAutofocus: intentional — modal opens for this field
              autoFocus
              className="flex-1 min-w-0 bg-surface-container-low border border-outline-variant rounded text-[13px] text-on-surface px-2.5 py-1.5 focus:border-outline placeholder-on-surface-variant/45 outline-none"
            />
            <span className="text-[11px] text-on-surface-variant shrink-0 whitespace-nowrap">
              Paste a URL and press Enter to queue
            </span>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handlePickFiles}
              disabled={loading}
              className="border border-outline-variant text-on-surface px-2 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50"
            >
              Choose files
            </button>
            <span className="text-[11px] text-on-surface-variant">
              Supports .txt, .md, .pdf, .docx
            </span>
          </div>
        </div>

        {/* Queue — only when non-empty */}
        {hasQueue && (
          <div className="pb-2.5">
            {sources.map((source, index) => {
              const failed = failedIndex === index;
              return (
                <div
                  key={source.id}
                  className={`flex items-center gap-2 px-4 py-1 ${failed ? "bg-error/5" : ""}`}
                >
                  <p
                    className={`flex-1 min-w-0 text-[12px] truncate ${failed ? "text-error" : "text-on-surface-variant"}`}
                    title={sourceDisplay(source)}
                  >
                    {failed && <FontAwesomeIcon icon={faCircleExclamation} className={`${iconClass.sm} inline mr-1`} />}
                    {sourceDisplay(source)}
                  </p>
                  <button
                    type="button"
                    onClick={() => removeSource(source.id)}
                    disabled={loading}
                    className="app-icon-button p-1 disabled:opacity-50 shrink-0"
                  >
                    <FontAwesomeIcon icon={faXmark} className={iconClass.sm} />
                  </button>
                </div>
              );
            })}
          </div>
        )}

        {/* Footer — only when non-empty */}
        {hasQueue && (
          <div className="flex flex-col gap-2.5 px-4 py-3.5 border-t border-outline-variant shrink-0">
            {/* Existing row: description + ingest button */}
            <div className="flex items-center gap-2.5">
              <p className="flex-1 min-w-0 text-[11px] text-on-surface-variant">
                One summary resource will be created from all queued sources.
              </p>
              <button
                onClick={handleSubmit}
                disabled={loading}
                className={`border border-outline text-on-surface px-4 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50 ${loading ? "ingest-loading-button" : ""}`}
              >
                {loading ? "Ingesting…" : "Ingest"}
              </button>
            </div>

            {/* New row: model + effort pills, centered */}
            <div className="flex items-center justify-center gap-[10px]">
              {/* Model picker */}
              <div className="relative" ref={modelRef}>
                <button
                  className={`flex items-center gap-1 px-2 py-1 rounded transition-colors text-[11px] border border-outline-variant/50 ${
                    noProvidersAvailable
                      ? "text-on-surface-variant/50 cursor-not-allowed"
                      : "text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface"
                  }`}
                  onClick={() => { if (noProvidersAvailable) return; setModelOpen((o) => !o); setEffortOpen(false); }}
                  disabled={noProvidersAvailable}
                >
                  <span className="text-[10px] text-on-surface-variant/60 mr-0.5">model</span>
                  {modelLabel}
                  {!noProvidersAvailable && <FontAwesomeIcon icon={faChevronDown} className={iconClass.sm} />}
                </button>
                <div className={`${modelOpen ? "block" : "hidden"} absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 min-w-[230px] overflow-hidden z-50`}>
                  {claudeModels.length > 0 && <ProviderSection label="Claude Code" provider="claude" models={claudeModels} />}
                  {codexModels.length > 0 && <ProviderSection label="Codex" provider="codex" models={codexModels} />}
                </div>
              </div>

              {/* Effort picker */}
              {showEffortPicker && (
                <div className="relative" ref={effortRef}>
                  <button
                    className="flex items-center gap-1 px-2 py-1 text-on-surface-variant hover:bg-surface-container-high rounded hover:text-on-surface transition-colors text-[11px] border border-outline-variant/50"
                    onClick={() => { setEffortOpen((o) => !o); setModelOpen(false); }}
                  >
                    <span className="text-[10px] text-on-surface-variant/60 mr-0.5">effort</span>
                    {effortLabel}
                    <FontAwesomeIcon icon={faChevronDown} className={iconClass.sm} />
                  </button>
                  <div className={`${effortOpen ? "block" : "hidden"} absolute bottom-full left-0 mb-1 bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 min-w-[140px] overflow-hidden z-50`}>
                    <div className="px-3 pt-2 pb-1.5 text-[10px] font-medium uppercase tracking-normal text-on-surface-variant/60">Effort</div>
                    {effortLevels.map((lvl) => (
                      <button
                        key={lvl.value}
                        className={`w-full grid grid-cols-[1fr_14px] items-center gap-2 px-3 py-1.5 text-left text-[12px] hover:bg-surface-container-highest transition-colors ${lvl.value === selectedEffort ? "font-medium text-on-surface" : "text-on-surface-variant"}`}
                        onClick={() => { setSelectedEffort(lvl.value); setEffortOpen(false); }}
                      >
                        <span>{lvl.label}</span>
                        <span className="text-[11px] text-primary">{lvl.value === selectedEffort ? "✓" : ""}</span>
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {error && !hasQueue && (
          <p className="text-[12px] text-error px-4 pb-3">{error}</p>
        )}
      </div>
    </div>
  );
}
