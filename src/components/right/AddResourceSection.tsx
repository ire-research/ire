import { useState } from "react";
import { ipc } from "../../ipc";
import { Icon } from "../Icon";

export function AddResourceSection() {
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!url.trim() || loading) return;
    setLoading(true);
    setError(null);
    try {
      await ipc.submitResource(url.trim());
      setUrl("");
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

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-3">
        <Icon name="add_link" className="w-[16px] h-[16px] shrink-0 text-on-surface-variant" />
        <span className="text-[14px] text-on-surface-variant">
          Add resource
        </span>
      </div>
      <div className="flex gap-2">
        <input
          type="text"
          placeholder="https://arxiv.org/abs/..."
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={loading}
          className="flex-1 bg-surface-container border border-outline-variant rounded text-[13px] text-on-surface px-2 py-1.5 focus:border-outline focus:ring-0 placeholder-on-surface-variant/50 min-w-0 outline-none"
        />
        <button
          onClick={handleSubmit}
          disabled={!url.trim() || loading}
          className="border border-outline text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors shrink-0 disabled:opacity-50"
        >
          {loading ? "Adding…" : "Add"}
        </button>
      </div>
      {error && <p className="text-[12px] text-error mt-1">{error}</p>}
    </div>
  );
}
