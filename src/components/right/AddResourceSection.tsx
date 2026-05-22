import { useState } from "react";
import { ipc, pickResourceFiles, type ResourceSourceInput } from "../../ipc";
import { Icon } from "../Icon";

type QueuedSource =
  | { id: string; kind: "url"; url: string; label: string }
  | { id: string; kind: "local_file"; path: string; label: string };

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function urlLabel(url: string): string {
  try {
    const parsed = new URL(url);
    return `${parsed.host}${parsed.pathname}`;
  } catch {
    return url.replace(/^https?:\/\//, "");
  }
}

function sourceKey(source: QueuedSource): string {
  return source.kind === "url" ? `url:${source.url}` : `file:${source.path}`;
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

export function AddResourceSection() {
  const [url, setUrl] = useState("");
  const [sources, setSources] = useState<QueuedSource[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [failedIndex, setFailedIndex] = useState<number | null>(null);

  const clearError = () => {
    setError(null);
    setFailedIndex(null);
  };

  const addSources = (nextSources: QueuedSource[]) => {
    setSources((current) => {
      const seen = new Set(current.map(sourceKey));
      const deduped = nextSources.filter((source) => {
        const key = sourceKey(source);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      return [...current, ...deduped];
    });
  };

  const handleAddLink = () => {
    if (loading) return;

    const trimmedUrl = url.trim();
    if (!trimmedUrl) return;

    clearError();
    addSources([{
      id: crypto.randomUUID(),
      kind: "url",
      url: trimmedUrl,
      label: urlLabel(trimmedUrl),
    }]);
    setUrl("");
  };

  const handleSubmit = async () => {
    if (loading || sources.length === 0) return;

    setLoading(true);
    clearError();
    try {
      await ipc.submitResources(sources.map(toInput));
      setUrl("");
      setSources([]);
    } catch (e) {
      const message = String(e);
      setError(message);
      setFailedIndex(parseFailedSourceIndex(message));
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleAddLink();
    }
  };

  const handlePickFiles = async () => {
    if (loading) return;
    clearError();
    try {
      const paths = await pickResourceFiles();
      addSources(paths.map((path) => ({
        id: crypto.randomUUID(),
        kind: "local_file",
        path,
        label: basename(path),
      })));
    } catch (e) {
      setError(String(e));
    }
  };

  const removeSource = (id: string) => {
    clearError();
    setSources((current) => current.filter((source) => source.id !== id));
  };

  const canSubmit = sources.length > 0 && !loading;
  const sourceCount = sources.length === 1 ? "1 source" : `${sources.length} sources`;

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-3">
        <Icon name="add_link" className="w-[16px] h-[16px] shrink-0 text-on-surface-variant" />
        <span className="text-[14px] text-on-surface-variant">
          Add resources
        </span>
      </div>
      <div className="space-y-2">
        <div className="flex items-center gap-1.5">
          <input
            type="text"
            placeholder="https://arxiv.org/abs/..."
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={loading}
            className="w-full bg-surface-container border border-outline-variant rounded text-[13px] text-on-surface px-2 py-1.5 focus:border-outline focus:ring-0 placeholder-on-surface-variant/50 min-w-0 outline-none"
          />
          <button
            type="button"
            onClick={handleAddLink}
            disabled={!url.trim() || loading}
            className="border border-outline-variant text-on-surface px-2 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50"
          >
            Add link
          </button>
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
          <span className="text-[11px] text-on-surface-variant min-w-0 flex-1 truncate">
            Supports .txt, .md, .pdf, .docx
          </span>
        </div>
        <div className="border-t border-outline-variant/50 pt-2">
          <div className="flex items-center justify-between gap-2 mb-1.5">
            <span className="text-[12px] text-on-surface-variant">
              Waiting for ingestion
            </span>
            <span className="font-mono text-[11px] text-on-surface-variant border border-outline-variant rounded-full px-1.5 py-0.5">
              {sourceCount}
            </span>
          </div>
          <div className="border border-outline-variant rounded bg-surface-container-lowest/30 overflow-hidden">
            {sources.length === 0 ? (
              <p className="text-[12px] text-on-surface-variant italic px-2.5 py-3">
                No sources queued
              </p>
            ) : (
              sources.map((source, index) => {
                const failed = failedIndex === index;
                return (
                  <div
                    key={source.id}
                    className={`grid grid-cols-[18px_minmax(0,1fr)_24px] items-center gap-2 px-2 py-1.5 border-b border-outline-variant/50 last:border-b-0 ${
                      failed ? "bg-error/5" : ""
                    }`}
                  >
                    <Icon
                      name={failed ? "error" : source.kind === "url" ? "add_link" : "description"}
                      className={`w-[15px] h-[15px] ${failed ? "text-error" : "text-on-surface-variant"}`}
                    />
                    <div className="min-w-0">
                      <p className="text-[12px] text-on-surface truncate" title={source.label}>
                        {index + 1}. {source.label}
                      </p>
                      <p
                        className="text-[11px] text-on-surface-variant truncate"
                        title={source.kind === "url" ? source.url : source.path}
                      >
                        {source.kind === "url" ? source.url : source.path}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => removeSource(source.id)}
                      disabled={loading}
                      title="Remove source"
                      className="text-on-surface-variant hover:text-on-surface transition-colors p-1 disabled:opacity-50"
                    >
                      <Icon name="close" className="w-[14px] h-[14px]" />
                    </button>
                  </div>
                );
              })
            )}
          </div>
        </div>
        <div className="flex items-center justify-between gap-2">
          <p className="text-[11px] text-on-surface-variant min-w-0">
            One summary resource will be created from all queued sources.
          </p>
          <button
            onClick={handleSubmit}
            disabled={!canSubmit}
            className={`border border-outline text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 ${
              loading ? "ingest-loading-button" : "disabled:opacity-50"
            }`}
          >
            {loading ? "Ingesting..." : "Ingest"}
          </button>
        </div>
      </div>
      {error && <p className="text-[12px] text-error mt-1">{error}</p>}
    </div>
  );
}
