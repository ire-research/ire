import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownPaneProps {
  title: string;
  initialContent: string;
  showSubmit?: boolean;
  onSubmit?: (content: string) => void;
}

type Mode = "preview" | "edit";

export function MarkdownPane({
  title,
  initialContent,
  showSubmit,
  onSubmit,
}: MarkdownPaneProps) {
  const [mode, setMode] = useState<Mode>("preview");
  const [draft, setDraft] = useState(initialContent);

  const handleToggle = (next: Mode) => {
    if (next === "preview" && draft !== initialContent) {
      const ok = window.confirm("Discard unsaved edits?");
      if (!ok) return;
      setDraft(initialContent);
    }
    setMode(next);
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
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{draft}</ReactMarkdown>
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
          <button onClick={() => onSubmit?.(draft)}>Submit</button>
        </footer>
      )}
    </section>
  );
}
