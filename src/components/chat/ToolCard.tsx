import { useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faFileLines, faPenToSquare, faMagnifyingGlass, faLink, faTerminal, faWrench, faChevronDown, iconClass } from "../../icons";
import type { IconDefinition } from "@fortawesome/fontawesome-svg-core";
import type { ToolCallState, ToolFormat, ToolKind } from "../../types";

interface Props {
  tool: ToolCallState;
}

/** Tool kinds that belong to IRE (shown with the IRE text badge instead of a material icon). */
const IRE_KINDS = new Set<ToolKind>([
  "ire_read",
  "ire_edit",
  "resource_add",
  "memory_write",
  "experiment_start",
  "experiment_status",
  "experiment_tail_logs",
]);

export function ToolCard({ tool }: Props) {
  const [expanded, setExpanded] = useState(false);
  const input = tool.input.full ?? null;
  const output = tool.output?.full ?? null;
  const canExpand = !!(input || output);
  const preview = previewForTool(tool);
  const isIre = IRE_KINDS.has(tool.kind);

  return (
    <div className="w-full font-mono text-xs">
      {/* ── Clickable summary row ── */}
      <div
        className="flex items-center gap-2 py-[3px] cursor-pointer group"
        onClick={() => canExpand && setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && canExpand && setExpanded((v) => !v)}
      >
        {/* Icon: IRE badge or material icon */}
        <div className="w-[18px] h-[18px] flex items-center justify-center shrink-0">
          {isIre ? (
        <span className="font-sans font-bold text-[9px] tracking-wide leading-none text-on-surface-variant/50 select-none">
              IRE
            </span>
          ) : (
            <FontAwesomeIcon icon={iconForKind(tool.kind)} className={`${iconClass.md} text-on-surface-variant/60`} />
          )}
        </div>

        {/* Name badge — the bordered rectangle */}
        <span className="shrink-0 border border-outline rounded-sm px-[7px] py-px text-[11px] leading-5 text-on-surface-variant bg-surface-container-low transition-colors group-hover:bg-surface-container group-hover:border-outline-strong">
          {labelForKind(tool.kind, tool.raw_name)}
        </span>

        {/* Status dot — experiment_start only */}
        {tool.kind === "experiment_start" && (
          <span className={statusDotClass(tool.status)} />
        )}

        {/* Inline arg preview — hidden when expanded */}
        {!expanded && preview && (
          <span className="text-on-surface-variant/50 text-[11px] flex-1 min-w-0 truncate">
            {preview}
          </span>
        )}

        {/* Expand chevron */}
        {canExpand && (
          <FontAwesomeIcon
            icon={faChevronDown}
            className={`${iconClass.sm} text-on-surface-variant ml-auto shrink-0 opacity-0 group-hover:opacity-60 transition-all duration-150 ${expanded ? "rotate-180 !opacity-60" : ""}`}
          />
        )}
      </div>

      {/* ── Expanded body ── */}
      {expanded && (
        <div className="ml-[26px] mt-1 border border-outline-variant rounded overflow-hidden bg-surface-container-lowest font-mono text-[11px] leading-relaxed">
          {input && (
            <ToolIoField
              label="IN"
              content={input}
              format={tool.input.format}
            />
          )}
          {output && (
            <ToolIoField
              label="OUT"
              content={output}
              format={tool.output!.format}
            />
          )}
        </div>
      )}
    </div>
  );
}

// ─── ToolIoField ───────────────────────────────────────────────────────────────

interface ToolIoFieldProps {
  label: string;
  content: string;
  /** When provided, controls whether JSON is rendered as key:value pairs. */
  format?: ToolFormat;
}

export function ToolIoField({ label, content, format }: ToolIoFieldProps) {
  const body = renderIoContent(content, format ?? "text");
  return (
    <div className="grid grid-cols-[36px_minmax(0,1fr)] border-t border-outline-variant first:border-t-0">
      <div className="px-2 py-1.5 pt-2 text-on-surface-variant/40 uppercase text-[9.5px] tracking-wide leading-none">
        {label}
      </div>
      <pre className="px-2.5 py-1.5 text-on-surface-variant whitespace-pre-wrap break-words overflow-x-auto">
        {body}
      </pre>
    </div>
  );
}

// ─── Rendering helpers ─────────────────────────────────────────────────────────

/**
 * Render IO content for the expanded panel.
 *
 * JSON format:
 *   - Single key  → show the value only (no key, no brackets)
 *   - Multi key   → one line per key, dimmed key + value, no quotes/brackets
 * Text format: verbatim.
 */
function renderIoContent(content: string, format: ToolFormat): React.ReactNode {
  if (format !== "json") return content;

  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch {
    return content;
  }

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return content;
  }

  const obj = parsed as Record<string, unknown>;

  // Unwrap mcp_tool_call shape: { server, tool, arguments: {...} }
  // Show only the contents of `arguments`, drop the outer wrapper.
  const inner =
    obj["arguments"] !== undefined &&
    obj["arguments"] !== null &&
    typeof obj["arguments"] === "object" &&
    !Array.isArray(obj["arguments"])
      ? (obj["arguments"] as Record<string, unknown>)
      : obj;

  const entries = Object.entries(inner);
  if (entries.length === 0) return content;

  // Single argument — show value only
  if (entries.length === 1) {
    return String(entries[0][1]);
  }

  // Multi-argument — key: value lines
  const maxKeyLen = Math.max(...entries.map(([k]) => k.length));
  return entries.map(([key, val], i) => {
    const padding = " ".repeat(maxKeyLen - key.length + 1);
    const valueStr = typeof val === "string" ? val : JSON.stringify(val);
    return (
      <span key={i}>
        {i > 0 && "\n"}
        <span className="text-on-surface-variant/40">{key}</span>
        {padding}
        <span className="text-on-surface-variant">{valueStr}</span>
      </span>
    );
  });
}

// ─── Per-tool helpers ──────────────────────────────────────────────────────────

function previewForTool(tool: ToolCallState): string | null {
  const preview = tool.input.preview ?? tool.output?.preview ?? null;
  if (tool.kind !== "other") return preview;
  return preview ? `${tool.raw_name} · ${preview}` : tool.raw_name;
}

function labelForKind(kind: ToolKind, rawName: string): string {
  if (kind === "command") return "bash";
  if (kind === "other") return rawName;
  return kind;
}

function iconForKind(kind: ToolKind): IconDefinition {
  switch (kind) {
    case "file_read": return faFileLines;
    case "file_write":
    case "file_edit": return faPenToSquare;
    case "file_search": return faMagnifyingGlass;
    case "web_fetch": return faLink;
    case "command": return faTerminal;
    default: return faWrench;
  }
}

function statusDotClass(status: string): string {
  if (status === "running") return "w-1.5 h-1.5 rounded-full bg-warn animate-pulse shrink-0";
  if (status === "completed") return "w-1.5 h-1.5 rounded-full bg-ok shrink-0";
  return "w-1.5 h-1.5 rounded-full bg-error shrink-0";
}
