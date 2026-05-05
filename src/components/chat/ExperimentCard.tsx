import { useState } from "react";
import type { ToolCallState } from "../../types";

interface Props {
  tool: ToolCallState;
}

export function ExperimentCard({ tool }: Props) {
  const [logsOpen, setLogsOpen] = useState(false);
  const status = tool.experimentStatus ?? "starting";
  const lines = tool.logLines ?? [];

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
