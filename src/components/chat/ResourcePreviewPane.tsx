import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faPencil, iconClass } from "../../icons";

interface Props {
  title: string;
  content: string;
  onSave?: (content: string) => Promise<void>;
}

interface Frontmatter {
  title?: string;
  sources?: string[];
  updated?: string;
  tldr?: string;
}

function parseFrontmatter(content: string): { fm: Frontmatter | null; body: string } {
  if (!content.startsWith("---\n")) return { fm: null, body: content };
  const end = content.indexOf("\n---\n", 4);
  if (end === -1) return { fm: null, body: content };

  const raw = content.slice(4, end);
  const body = content.slice(end + 5);
  const fm: Frontmatter = {};
  let currentKey: string | null = null;

  for (const line of raw.split("\n")) {
    const listMatch = line.match(/^\s+-\s+(.+)$/);
    if (listMatch && currentKey === "sources") {
      fm.sources = [...(fm.sources ?? []), listMatch[1].trim()];
      continue;
    }
    const kvMatch = line.match(/^([^:]+):\s*(.*)$/);
    if (!kvMatch) continue;
    const key = kvMatch[1].trim();
    const val = kvMatch[2].trim().replace(/^"(.*)"$/, "$1");
    currentKey = key;
    if (key === "title") fm.title = val;
    else if (key === "sources" && val) fm.sources = [val];
    else if (key === "updated") fm.updated = val;
    else if (key === "TL;DR") fm.tldr = val;
  }

  return { fm: Object.keys(fm).length > 0 ? fm : null, body };
}

export function resourcePreviewTitle(content: string, fallback = "Resource"): string {
  return parseFrontmatter(content).fm?.title?.trim() || fallback;
}

function FmRow({ label, value, valueClass = "text-on-surface-variant" }: { label: string; value: React.ReactNode; valueClass?: string }) {
  return (
    <div className="grid grid-cols-[72px_1fr] gap-1.5 leading-[1.6]">
      <span className="text-on-surface-variant/40">{label}</span>
      <span className={`min-w-0 ${valueClass}`}>{value}</span>
    </div>
  );
}

const SOURCE_PREVIEW_MAX_CHARS = 72;

function truncateSource(source: string): string {
  if (source.length <= SOURCE_PREVIEW_MAX_CHARS) return source;
  return `${source.slice(0, SOURCE_PREVIEW_MAX_CHARS - 3)}...`;
}

function SourceText({ source }: { source: string }) {
  const [tooltip, setTooltip] = useState<{ label: string; x: number; y: number } | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const spanRef = useRef<HTMLSpanElement | null>(null);
  const display = truncateSource(source);
  const isTruncated = display !== source;

  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current); }, []);

  const handleMouseEnter = () => {
    if (!isTruncated) return;
    const span = spanRef.current;
    if (!span) return;
    const rect = span.getBoundingClientRect();
    timerRef.current = setTimeout(() => {
      setTooltip({ label: source, x: rect.left, y: rect.bottom + 4 });
    }, 250);
  };

  const handleMouseLeave = () => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setTooltip(null);
  };

  return (
    <>
      <span
        ref={spanRef}
        className="block min-w-0 truncate"
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {display}
      </span>
      {tooltip && (
        <div
          className="fixed z-50 px-2 py-1 bg-surface-container-high border border-outline/30 text-on-surface text-[13px] rounded shadow-md whitespace-nowrap pointer-events-none"
          style={{ left: tooltip.x, top: tooltip.y }}
        >
          {tooltip.label}
        </div>
      )}
    </>
  );
}

export function ResourcePreviewPane({ title, content, onSave }: Props) {
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(content);
  const [isSaving, setIsSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    setIsEditing(false);
    setDraft(content);
  }, [content]);

  useEffect(() => {
    if (isEditing && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isEditing]);

  const handleSave = async () => {
    if (isSaving || !onSave) return;
    setIsSaving(true);
    await onSave(draft);
    setIsSaving(false);
    setIsEditing(false);
  };

  const handleCancel = () => {
    setDraft(content);
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape") {
      handleCancel();
    } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void handleSave();
    }
  };

  if (isEditing) {
    return (
      <div className="absolute inset-0 flex flex-col px-4 md:px-8 lg:px-12 pt-6 pb-4">
        <div className="flex items-center justify-between mb-4 shrink-0">
          <p className="text-[11px] uppercase tracking-widest text-on-surface-variant">Resource · Wiki</p>
          <div className="flex items-center gap-2">
            <button
              onClick={handleCancel}
              className="text-[11px] font-mono text-on-surface-variant hover:text-on-surface transition-colors px-2 py-1"
            >
              Cancel
            </button>
            <button
              onClick={() => void handleSave()}
              disabled={isSaving}
              className="text-[11px] font-mono text-ok/80 hover:text-ok transition-colors px-2 py-1 disabled:opacity-50"
            >
              {isSaving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          className="flex-1 min-h-0 bg-transparent border border-outline-variant rounded text-[13px] font-mono text-on-surface px-3 py-2 focus:outline-none focus:border-outline resize-none"
        />
      </div>
    );
  }

  const { fm, body } = parseFrontmatter(content);

  // Normalize repeated sources and render a deterministic preview date.
  if (fm) {
    fm.sources = fm.sources ? [...new Set(fm.sources)] : undefined;
    fm.updated = new Date().toISOString().slice(0, 10);
  }

  const isNotRelevant = fm?.tldr?.toLowerCase() === "not relevant";
  const displayTitle = fm?.title || title;

  return (
    <div className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-6 pb-8">
      <div className="flex items-center justify-between mb-4">
        <p className="text-[11px] uppercase tracking-widest text-on-surface-variant">Resource · Wiki</p>
        {onSave && (
          <button
            onClick={() => setIsEditing(true)}
            className="app-icon-button cursor-pointer p-0.5"
            title="Edit resource"
          >
            <FontAwesomeIcon icon={faPencil} className={iconClass.md} />
          </button>
        )}
      </div>
      <h2 className="text-base font-semibold text-on-surface mb-2">{displayTitle}</h2>

      {fm && (
        <div className={`font-mono text-[11px] border rounded px-3 py-2 mb-5 flex flex-col gap-[3px] ${
          isNotRelevant ? "bg-[#231e1b] border-[#4a3428]" : "bg-surface-container-low border-outline"
        }`}>
          {fm.title && <FmRow label="title:" value={fm.title} />}
          {fm.sources && fm.sources.length > 0 && (
            <div className="grid grid-cols-[72px_1fr] gap-1.5 leading-[1.6]">
              <span className="text-on-surface-variant/40">sources:</span>
              <div className="min-w-0 flex flex-col gap-px text-on-surface-variant">
                {fm.sources.length > 1 ? (
                  <ul className="min-w-0 list-disc pl-4 m-0 flex flex-col gap-px">
                    {fm.sources.map((s, i) => <li key={i} className="min-w-0"><SourceText source={s} /></li>)}
                  </ul>
                ) : (
                  <SourceText source={fm.sources[0]} />
                )}
              </div>
            </div>
          )}
          {fm.updated && <FmRow label="updated:" value={fm.updated} />}
          {fm.tldr !== undefined && (
            <FmRow
              label="TL;DR:"
              value={fm.tldr}
              valueClass={isNotRelevant ? "text-[#c4714a]" : "text-on-surface-variant"}
            />
          )}
        </div>
      )}

      <div className="md-body text-on-surface">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{body}</ReactMarkdown>
      </div>
    </div>
  );
}
