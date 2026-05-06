import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownPaneProps {
  title: string;
  content: string;
  showSubmit?: boolean;
  onSubmit?: (content: string) => void;
}

type Mode = "preview" | "edit";

function stripFrontmatter(content: string): string {
  if (!content.startsWith("---")) return content;
  const end = content.indexOf("\n---", 3);
  if (end === -1) return content;
  return content.slice(end + 4).replace(/^\n/, "");
}

export function MarkdownPane({
  title,
  content,
  showSubmit,
  onSubmit,
}: MarkdownPaneProps) {
  const [mode, setMode] = useState<Mode>("preview");
  const [draft, setDraft] = useState(content);

  // Sync draft when persisted content changes (e.g. after save or wiki-changed event)
  useEffect(() => {
    if (mode === "preview") {
      setDraft(content);
    }
  }, [content]);

  const handleToggle = (next: Mode) => {
    if (next === "preview" && draft !== content) {
      const ok = window.confirm("Discard unsaved edits?");
      if (!ok) return;
      setDraft(content);
    }
    setMode(next);
  };

  const handleSubmit = () => {
    onSubmit?.(draft);
    setMode("preview");
  };

  return (
    <section className="md-pane">
      <header className="md-pane__header">
        <h3>{title}</h3>
        <div className="md-pane__toolbar">
          <button
            className={mode === "preview" ? "active" : ""}
            onClick={() => handleToggle("preview")}
          >
            Preview
          </button>
          <button
            className={mode === "edit" ? "active" : ""}
            onClick={() => handleToggle("edit")}
          >
            Edit
          </button>
        </div>
      </header>
      <div className="md-pane__body">
        {mode === "preview" ? (
          <div className="md-pane__preview">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{stripFrontmatter(draft)}</ReactMarkdown>
          </div>
        ) : (
          <textarea
            className="md-pane__editor"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
        )}
      </div>
      {showSubmit && mode === "edit" && (
        <footer className="md-pane__footer">
          <button onClick={handleSubmit}>Submit</button>
        </footer>
      )}
    </section>
  );
}
