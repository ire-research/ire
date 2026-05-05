import { useState } from "react";
import { ipc } from "../ipc";

export function ResourceInput() {
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

  return (
    <section className="resource-input">
      <h3>Add Resource</h3>
      <input
        type="url"
        placeholder="https://arxiv.org/abs/..."
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
        disabled={loading}
      />
      <button disabled={!url.trim() || loading} onClick={handleSubmit}>
        {loading ? "Fetching…" : "Submit"}
      </button>
      {error && <p className="resource-input__error">{error}</p>}
    </section>
  );
}
