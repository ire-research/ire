import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ToolCallState } from "../../types";

interface Props {
  tool: ToolCallState;
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
    <div className="experiment-card">
      <div
        className="experiment-card__header"
        onClick={() => setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && setExpanded((v) => !v)}
      >
        <span className={`experiment-card__dot experiment-card__dot--${status}`} />
        <span className="experiment-card__label">⚗ Experiment</span>
        <span className={`experiment-card__status experiment-card__status--${status}`}>
          {statusLabel(status)}
        </span>
        {tool.experimentPid !== undefined && (
          <span className="experiment-card__pid">PID {tool.experimentPid}</span>
        )}
        {tool.experimentExitCode !== undefined && status === "failed" && (
          <span className="experiment-card__exit">exit {tool.experimentExitCode}</span>
        )}
        <span className="experiment-card__chevron">{expanded ? "▾" : "▸"}</span>
        {isLive && tool.experimentUuid && (
          <button
            className="experiment-card__cancel"
            disabled={cancelling}
            onClick={handleCancel}
          >
            {cancelling ? "Cancelling…" : "Cancel"}
          </button>
        )}
      </div>

      {expanded && (
        lines.length > 0 ? (
          <div className="experiment-card__log-tail">
            {lines.slice(-10).map((line, i) => (
              <div key={i} className="experiment-card__log-line">{line}</div>
            ))}
          </div>
        ) : (
          <div className="experiment-card__empty">No output yet.</div>
        )
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
