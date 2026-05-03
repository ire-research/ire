import { useState } from "react";

export function ResourceInput() {
  const [url, setUrl] = useState("");

  return (
    <section className="resource-input">
      <h3>Add Resource</h3>
      <input
        type="url"
        placeholder="https://arxiv.org/abs/..."
        value={url}
        onChange={(e) => setUrl(e.target.value)}
      />
      <button disabled={!url}>Submit</button>
    </section>
  );
}
