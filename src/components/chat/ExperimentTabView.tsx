import { useEffect, useRef, useState } from "react";
import { ipc, onExperimentLogLine } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ExperimentRow } from "../../types";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faPencil, iconClass } from "../../icons";

interface Props {
  uuid: string;
  name: string;
}

function getStatusPill(status: string): { text: string; textColor: string; borderColor: string; bgColor: string } {
  const normalized = status.toLowerCase();
  if (normalized === "running") {
    return { text: "Running", textColor: "text-warn", borderColor: "border-warn/30", bgColor: "bg-warn/10" };
  }
  if (normalized === "completed") {
    return { text: "Done", textColor: "text-ok", borderColor: "border-ok/30", bgColor: "bg-ok/10" };
  }
  if (normalized === "failed" || normalized === "cancelled") {
    return { text: "Fail", textColor: "text-error", borderColor: "border-error/30", bgColor: "bg-error/10" };
  }
  return { text: status, textColor: "text-on-surface-variant", borderColor: "border-on-surface-variant/30", bgColor: "bg-on-surface-variant/10" };
}

function getStatusColor(status: string): string {
  const normalized = status.toLowerCase();
  if (normalized === "running") return "text-warn";
  if (normalized === "completed") return "text-ok";
  if (normalized === "failed" || normalized === "cancelled") return "text-error";
  return "text-on-surface-variant";
}

function formatElapsed(startedAt: string, endedAt: string | null): string {
  const start = new Date(startedAt).getTime();
  const end = endedAt ? new Date(endedAt).getTime() : Date.now();
  const totalSeconds = Math.max(0, Math.floor((end - start) / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}m ${seconds}s`;
}

export function ExperimentTabView({ uuid, name }: Props) {
  const [experiment, setExperiment] = useState<ExperimentRow | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [elapsed, setElapsed] = useState<string>("");
  const [displayName, setDisplayName] = useState(name);
  const [isRenaming, setIsRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState(name);
  const logRef = useRef<HTMLDivElement>(null);

  // Load experiment data and initial logs
  useEffect(() => {
    ipc.experimentList(100).then((rows) => {
      const found = rows.find((r) => r.uuid === uuid) ?? null;
      setExperiment(found);
    });

    ipc.experimentLogs(uuid).then(({ stdout }) => {
      const lines = stdout.split("\n").filter((l) => l.length > 0);
      setLogs(lines);
    });
  }, [uuid]);

  // Auto-scroll on new log lines
  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [logs]);

  // Subscribe to live log lines
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    onExperimentLogLine((payload) => {
      if (payload.uuid === uuid && payload.stream === "stdout") {
        setLogs((prev) => [...prev, payload.line]);
      }
    }).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [uuid]);

  // Update elapsed every second when running
  useEffect(() => {
    if (!experiment) return;
    if (experiment.status.toLowerCase() !== "running") {
      setElapsed(formatElapsed(experiment.started_at, experiment.ended_at));
      return;
    }
    const tick = () => setElapsed(formatElapsed(experiment.started_at, null));
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [experiment]);

  const startRename = () => {
    setRenameValue(displayName);
    setIsRenaming(true);
  };

  const commitRename = async () => {
    const trimmed = renameValue.trim();
    if (!trimmed) {
      setIsRenaming(false);
      return;
    }
    try {
      await ipc.experimentRename(uuid, trimmed);
      setDisplayName(trimmed);
    } catch (err) {
      toastError("rename experiment", err);
    } finally {
      setIsRenaming(false);
    }
  };

  const handleRenameKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commitRename();
    } else if (e.key === "Escape") {
      setIsRenaming(false);
    }
  };

  if (!experiment) {
    return (
      <div className="text-on-surface-variant text-[12px] p-4">Loading…</div>
    );
  }

  const pill = getStatusPill(experiment.status);
  const statusColor = getStatusColor(experiment.status);
  const isRunning = experiment.status.toLowerCase() === "running";

  return (
    <>
      {/* Header */}
      <div className="group flex items-center gap-2 mb-4">
        {isRenaming ? (
          <input
            autoFocus
            className="font-mono font-semibold text-[14px] text-on-surface bg-transparent border-b border-outline outline-none"
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            onKeyDown={handleRenameKeyDown}
            onBlur={() => setIsRenaming(false)}
          />
        ) : (
          <>
            <span className="font-mono font-semibold text-[14px] text-on-surface">
              {displayName}
            </span>
            <button
              className="app-icon-button opacity-0 group-hover:opacity-100 h-5 w-5 shrink-0"
              title="Rename experiment"
              onClick={startRename}
            >
              <FontAwesomeIcon icon={faPencil} className={iconClass.md} />
            </button>
          </>
        )}
        <span className={`text-[10px] uppercase border ${pill.borderColor} px-1.5 py-0.5 rounded ${pill.bgColor} ${pill.textColor}`}>
          {pill.text}
        </span>
      </div>

      {/* Metadata grid */}
      <div className="grid grid-cols-2 gap-x-6 gap-y-3 mb-5 border border-outline-variant rounded p-3 bg-surface-container-low text-[12px]">
        <div>
          <span className="text-on-surface-variant block mb-0.5">Status</span>
          <span className={`${statusColor} font-medium`}>
            {isRunning ? `Running · ${elapsed}` : experiment.status}
          </span>
        </div>
        <div>
          <span className="text-on-surface-variant block mb-0.5">Runtime</span>
          <span className="text-on-surface font-mono">{elapsed}</span>
        </div>
        <div className="col-span-2">
          <span className="text-on-surface-variant block mb-0.5">Command</span>
          <code className="font-mono text-[11px] text-on-surface bg-surface-container px-2 py-1 rounded block truncate">
            {experiment.command}
          </code>
        </div>
      </div>

      {/* Logs */}
      <div className="border border-outline-variant rounded overflow-hidden">
        <div className="flex items-center justify-between px-3 py-1.5 bg-surface-container-low border-b border-outline-variant">
          <span className="text-[10px] uppercase tracking-widest text-on-surface-variant">Logs</span>
          {isRunning && (
            <span className="text-[10px] text-on-surface-variant font-mono">live</span>
          )}
        </div>
        <div
          ref={logRef}
          className="font-mono text-[11px] text-on-surface-variant p-3 bg-surface-container-lowest h-48 overflow-y-auto leading-relaxed"
        >
          {logs.map((line, i) => (
            <div key={i}>{line}</div>
          ))}
        </div>
      </div>
    </>
  );
}
