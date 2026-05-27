import { useEffect, useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faLink, faXmark, faCircleExclamation, iconClass } from "../icons";
import { ipc, pickResourceFiles, type ResourceSourceInput } from "../ipc";
import { useChatOptions } from "../state/chatOptions";

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
  const { model, provider, effort } = useChatOptions();
  const [url, setUrl] = useState("");
  const [sources, setSources] = useState<QueuedSource[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [failedIndex, setFailedIndex] = useState<number | null>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

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
      await ipc.submitResources(sources.map(toInput), { model, provider, effort });
      onClose();
    } catch (e) {
      const message = String(e);
      setError(message);
      setFailedIndex(parseFailedSourceIndex(message));
    } finally {
      setLoading(false);
    }
  };

  const hasQueue = sources.length > 0;

  return (
    <div
      className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="w-[360px] bg-surface-container border border-outline-variant rounded-lg flex flex-col overflow-hidden shadow-2xl">

        {/* Header */}
        <div className="flex items-center gap-2 px-4 pt-3.5 pb-3 border-b border-outline-variant shrink-0">
          <FontAwesomeIcon icon={faLink} className={`${iconClass.lg} shrink-0 text-on-surface-variant`} />
          <span className="flex-1 text-[13px] font-medium text-on-surface">Add resources</span>
          <button
            onClick={onClose}
            className="app-icon-button w-6 h-6"
          >
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
          <div className="flex items-center gap-2.5 px-4 py-3.5 border-t border-outline-variant shrink-0">
            <p className="flex-1 min-w-0 text-[11px] text-on-surface-variant">
              One summary resource will be created from all queued sources.
            </p>
            <button
              onClick={handleSubmit}
              disabled={loading}
              className={`border border-outline text-on-surface px-4 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50 ${
                loading ? "ingest-loading-button" : ""
              }`}
            >
              {loading ? "Ingesting…" : "Ingest"}
            </button>
          </div>
        )}

        {error && !hasQueue && (
          <p className="text-[12px] text-error px-4 pb-3">{error}</p>
        )}
      </div>
    </div>
  );
}
