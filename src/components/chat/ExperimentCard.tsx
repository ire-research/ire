import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ToolCallState } from "../../types";

interface Props {
  tool: ToolCallState;
}

export function ExperimentCard({ tool }: Props) {
  const [logsOpen, setLogsOpen] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const status = tool.experimentStatus ?? "starting";
  const lines = tool.logLines ?? [];
  const isLive = status === "starting" || status === "running";

  const handleCancel = async () => {
    if (!tool.experimentUuid) return;
    setCancelling(true);
    try {
      await ipc.experimentCancel(tool.experimentUuid);
    } catch (e) {
      toastError("cancel experiment", e);
    } finally {
      setCancelling(false);
    }
  };

  return (
    <div className="experiment-card">
      <div className="experiment-card__header">
        <span className="experiment-card__label">⚗ Experiment</span>
        <span className={`experiment-card__status experiment-card__status--${status}`}>
          {statusLabel(status)}
        </span>
        {tool.experimentExitCode !== undefined && status !== "completed" && (
          <span className="experiment-card__exit">exit {tool.experimentExitCode}</span>
        )}
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

      {lines.length > 0 && (
        <div className="experiment-card__log-tail">
          {lines.slice(-10).map((line, i) => (
            <div key={i} className="experiment-card__log-line">{line}</div>
          ))}
        </div>
      )}

      {lines.length > 10 && (
        <button
          className="experiment-card__toggle"
          onClick={() => setLogsOpen((v) => !v)}
        >
          {logsOpen ? "Hide full logs" : `View all ${lines.length} log lines`}
        </button>
      )}

      {logsOpen && lines.length > 10 && (
        <div className="experiment-card__log-full">
          {lines.map((line, i) => (
            <div key={i} className="experiment-card__log-line">{line}</div>
          ))}
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
