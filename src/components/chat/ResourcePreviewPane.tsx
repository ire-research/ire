import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface Props {
  title: string;
  content: string;
  actualSources?: string[];
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

function FmRow({ label, value, valueClass = "text-on-surface-variant" }: { label: string; value: React.ReactNode; valueClass?: string }) {
  return (
    <div className="grid grid-cols-[72px_1fr] gap-1.5 leading-[1.6]">
      <span className="text-on-surface-variant/40">{label}</span>
      <span className={valueClass}>{value}</span>
    </div>
  );
}

export function ResourcePreviewPane({ title, content, actualSources }: Props) {
  const { fm, body } = parseFrontmatter(content);

  // Override sources and updated with deterministic values.
  if (fm) {
    fm.sources = actualSources ?? (fm.sources ? [...new Set(fm.sources)] : undefined);
    fm.updated = new Date().toISOString().slice(0, 10);
  }

  const isNotRelevant = fm?.tldr?.toLowerCase() === "not relevant";

  return (
    <div className="absolute inset-0 overflow-y-auto px-4 md:px-8 lg:px-12 pt-6 pb-8">
      <p className="text-[11px] uppercase tracking-widest text-on-surface-variant mb-4">Resource · Wiki</p>
      <h2 className="text-base font-semibold text-on-surface mb-2">{title}</h2>

      {fm && (
        <div className={`font-mono text-[11px] border rounded px-3 py-2 mb-5 flex flex-col gap-[3px] ${
          isNotRelevant ? "bg-[#231e1b] border-[#4a3428]" : "bg-surface-container-low border-outline"
        }`}>
          {fm.title && <FmRow label="title:" value={fm.title} />}
          {fm.sources && fm.sources.length > 0 && (
            <div className="grid grid-cols-[72px_1fr] gap-1.5 leading-[1.6]">
              <span className="text-on-surface-variant/40">sources:</span>
              <div className="flex flex-col gap-px text-on-surface-variant">
                {fm.sources.length > 1 ? (
                  <ul className="list-disc pl-4 m-0 flex flex-col gap-px">
                    {fm.sources.map((s, i) => <li key={i}>{s}</li>)}
                  </ul>
                ) : (
                  <span>{fm.sources[0]}</span>
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

      <div className="text-[14px] text-on-surface leading-relaxed">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{body}</ReactMarkdown>
      </div>
    </div>
  );
}
