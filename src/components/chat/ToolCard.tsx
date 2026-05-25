import { useState } from "react";
import type { ToolCallState, ToolKind, ToolStatus } from "../../types";
import { Icon } from "../Icon";

interface Props {
  tool: ToolCallState;
}

export function ToolCard({ tool }: Props) {
  const [expanded, setExpanded] = useState(false);
  const input = tool.input.full ?? null;
  const output = tool.output?.full ?? null;
  const canExpand = !!(input || output);
  const preview = previewForTool(tool);

  return (
    <div className="w-full flex flex-col">
      <div
        className="w-full bg-surface-container-low border border-outline-variant rounded px-3 py-2 flex items-center gap-3 text-xs cursor-pointer hover:bg-surface-container transition-colors"
        onClick={() => canExpand && setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && canExpand && setExpanded((v) => !v)}
      >
        <Icon name={iconForKind(tool.kind)} className="w-[16px] h-[16px] text-on-surface-variant shrink-0" />
        {tool.kind === "experiment_start" && (
          <span className={statusDotClass(tool.status)} />
        )}
        <span className="font-mono text-on-surface-variant shrink-0">{tool.title}</span>
        {preview && (
          <span className="font-mono text-on-surface-variant/70 min-w-0 flex-1 truncate">
            {preview}
          </span>
        )}
      </div>
      {expanded && (
        <div className="bg-surface-container-lowest border-x border-b border-outline-variant rounded-b overflow-hidden font-mono text-[11px] leading-relaxed">
          {input && <ToolIoField label="IN" content={input} />}
          {output && <ToolIoField label="OUT" content={output} />}
        </div>
      )}
    </div>
  );
}

export function ToolIoField({ label, content }: { label: string; content: string }) {
  return (
    <div className="grid grid-cols-[42px_minmax(0,1fr)] border-t border-outline-variant first:border-t-0">
      <div className="px-3 py-2 text-on-surface-variant/60 uppercase">{label}</div>
      <pre className="px-3 py-2 text-on-surface-variant whitespace-pre-wrap break-words overflow-x-auto">{content}</pre>
    </div>
  );
}

function statusDotClass(status: ToolStatus): string {
  if (status === "running") return "w-1.5 h-1.5 rounded-full bg-warn animate-pulse shrink-0";
  if (status === "completed") return "w-1.5 h-1.5 rounded-full bg-ok shrink-0";
  return "w-1.5 h-1.5 rounded-full bg-error shrink-0";
}

function previewForTool(tool: ToolCallState): string | null {
  const preview = tool.input.preview ?? tool.output?.preview ?? null;
  if (tool.kind !== "other") return preview;
  return preview ? `${tool.raw_name} · ${preview}` : tool.raw_name;
}

function iconForKind(kind: ToolKind): string {
  switch (kind) {
    case "file_read":
    case "wiki_read":
      return "description";
    case "file_write":
    case "file_edit":
    case "wiki_write":
    case "wiki_append":
    case "wiki_rename":
    case "memory_write":
    case "pulse_update":
      return "edit_document";
    case "web_fetch":
      return "add_link";
    default:
      return "build";
  }
}
