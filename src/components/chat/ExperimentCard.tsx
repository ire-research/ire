import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ToolCallState } from "../../types";

interface Props {
  tool: ToolCallState;
}

function dotClass(status: string): string {
  if (status === "starting" || status === "running") {
    return "w-2 h-2 rounded-full bg-warn animate-pulse shrink-0";
  }
  if (status === "completed") {
    return "w-2 h-2 rounded-full bg-ok shrink-0";
  }
  return "w-2 h-2 rounded-full bg-error shrink-0";
}

function statusColorClass(status: string): string {
  if (status === "starting" || status === "running") return "text-warn text-[10px] uppercase";
  if (status === "completed") return "text-ok text-[10px] uppercase";
  return "text-error text-[10px] uppercase";
}

export function ExperimentCard({ tool }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const status = tool.experimentStatus ?? "starting";
  const lines = tool.logLines ?? [];
  const isLive = status === "starting" || status === "running";

  const handleCancel = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!tool.experimentUuid) return;
    setCancelling(true);
    try {
      await ipc.experimentCancel(tool.experimentUuid);
    } catch (err) {
      toastError("cancel experiment", err);
    } finally {
      setCancelling(false);
    }
  };

  return (
    <div className="w-full border border-outline-variant rounded overflow-hidden">
      <div
        className="flex items-center gap-3 px-3 py-2 bg-surface-container-low cursor-pointer text-xs"
        onClick={() => setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && setExpanded((v) => !v)}
      >
        <span className={dotClass(status)} />
        <span className="font-mono text-on-surface-variant flex-1">
          ⚗ {tool.tool_name || "Experiment"}
        </span>
        <span className={statusColorClass(status)}>{statusLabel(status)}</span>
        {tool.experimentPid !== undefined && (
          <span className="text-[10px] text-on-surface-variant/60">PID {tool.experimentPid}</span>
        )}
        {tool.experimentExitCode !== undefined && status === "failed" && (
          <span className="text-[10px] text-on-surface-variant/60">exit {tool.experimentExitCode}</span>
        )}
        <span className="text-on-surface-variant">{expanded ? "▾" : "▸"}</span>
        {isLive && tool.experimentUuid && (
          <button
            className="text-[11px] text-on-surface-variant border border-outline-variant px-1.5 py-0.5 rounded hover:border-error hover:text-error transition-colors"
            disabled={cancelling}
            onClick={handleCancel}
          >
            {cancelling ? "Cancelling…" : "Cancel"}
          </button>
        )}
      </div>

      {expanded && (
        <div className="bg-surface-container-lowest border-t border-outline-variant font-mono text-[11px] leading-relaxed">
          {tool.input_full && (
            <div className="grid grid-cols-[42px_minmax(0,1fr)] border-b border-outline-variant">
              <div className="px-3 py-2 text-on-surface-variant/60 uppercase">IN</div>
              <pre className="px-3 py-2 text-on-surface-variant whitespace-pre-wrap break-words overflow-x-auto">{tool.input_full}</pre>
            </div>
          )}
          {lines.length > 0 ? (
            <div className="grid grid-cols-[42px_minmax(0,1fr)]">
              <div className="px-3 py-2 text-on-surface-variant/60 uppercase">OUT</div>
              <div className="px-3 py-2 text-on-surface-variant max-h-32 overflow-y-auto">
                {lines.slice(-10).map((line, i) => (
                  <div key={i}>{line}</div>
                ))}
              </div>
            </div>
          ) : tool.output_full ? (
            <div className="grid grid-cols-[42px_minmax(0,1fr)]">
              <div className="px-3 py-2 text-on-surface-variant/60 uppercase">OUT</div>
              <pre className="px-3 py-2 text-on-surface-variant whitespace-pre-wrap break-words overflow-x-auto">{tool.output_full}</pre>
            </div>
          ) : (
            <div className="grid grid-cols-[42px_minmax(0,1fr)]">
              <div className="px-3 py-2 text-on-surface-variant/60 uppercase">OUT</div>
              <div className="px-3 py-2 text-on-surface-variant/40">No output yet.</div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function statusLabel(status: string): string {
  switch (status) {
    case "starting": return "Starting…";
    case "running": return "Running";
    case "completed": return "Completed";
    case "failed": return "Failed";
    case "cancelled": return "Cancelled";
    default: return status;
  }
}
