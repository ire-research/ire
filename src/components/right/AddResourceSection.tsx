import { useState } from "react";
import { ipc, pickResourceFile } from "../../ipc";
import { Icon } from "../Icon";

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function AddResourceSection() {
  const [url, setUrl] = useState("");
  const [filePath, setFilePath] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (loading) return;

    const trimmedUrl = url.trim();
    if (!trimmedUrl && !filePath) return;
    if (trimmedUrl && filePath) {
      setError("Upload one single file or use one URL to proceed.");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      if (filePath) {
        await ipc.submitLocalResource(filePath);
      } else {
        await ipc.submitResource(trimmedUrl);
      }
      setUrl("");
      setFilePath(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      handleSubmit();
    }
  };

  const handlePickFile = async () => {
    if (loading) return;
    setError(null);
    try {
      const path = await pickResourceFile();
      if (path) setFilePath(path);
    } catch (e) {
      setError(String(e));
    }
  };

  const hasUrl = Boolean(url.trim());
  const hasFile = Boolean(filePath);
  const canSubmit = (hasUrl || hasFile) && !(hasUrl && hasFile) && !loading;
  const validationMessage = hasUrl && hasFile
    ? "Upload one single file or use one URL to proceed."
    : null;
  const visibleMessage = validationMessage ?? error;

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-3">
        <Icon name="add_link" className="w-[16px] h-[16px] shrink-0 text-on-surface-variant" />
        <span className="text-[14px] text-on-surface-variant">
          Add resource
        </span>
      </div>
      <div className="space-y-2">
        <input
          type="text"
          placeholder="https://arxiv.org/abs/..."
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={loading}
          className="w-full bg-surface-container border border-outline-variant rounded text-[13px] text-on-surface px-2 py-1.5 focus:border-outline focus:ring-0 placeholder-on-surface-variant/50 min-w-0 outline-none"
        />
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handlePickFile}
            disabled={loading}
            className="border border-outline-variant text-on-surface px-2 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50"
          >
            Choose file
          </button>
          <span
            className="text-[12px] text-on-surface min-w-0 flex-1 truncate"
            title={filePath ?? undefined}
          >
            {filePath ? basename(filePath) : "No file selected"}
          </span>
          {filePath && (
            <button
              type="button"
              onClick={() => setFilePath(null)}
              disabled={loading}
              title="Clear file"
              className="text-on-surface-variant hover:text-on-surface transition-colors p-1 disabled:opacity-50"
            >
              <Icon name="close" className="w-[14px] h-[14px]" />
            </button>
          )}
        </div>
        <div className="flex items-center justify-between gap-2">
          <p className="text-[11px] text-on-surface-variant">
            Supports .txt, .md, .pdf, .docx
          </p>
          <button
            onClick={handleSubmit}
            disabled={!canSubmit}
            className="border border-outline text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50"
          >
            {loading ? "Adding..." : "Add"}
          </button>
        </div>
      </div>
      {visibleMessage && <p className="text-[12px] text-error mt-1">{visibleMessage}</p>}
    </div>
  );
}
