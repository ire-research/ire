import { useEffect, useRef, useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faMicrochip, faGamepad, faDatabase, faChevronDown, iconClass } from "../icons";
import { useSystemInfo } from "../hooks/useSystemStatus";
import { useWorkspace } from "../state/workspace";
import { PROVIDER_LABELS, type BinaryStatus, type SystemMetrics } from "../types";

function getUsageColor(usage: number): string {
  if (usage < 70) return "text-ok";
  if (usage <= 90) return "text-warn";
  return "text-error";
}

function AgentRow({ label, status }: { label: string; status: BinaryStatus }) {
  const dotClass =
    status.kind === "ready"
      ? "bg-ok"
      : status.kind === "logged_out"
        ? "bg-error"
        : "bg-surface-container-high border border-outline";
  const statusText =
    status.kind === "ready" ? "ready" : status.kind === "logged_out" ? "logged out" : "not installed";
  const statusClass =
    status.kind === "ready" ? "text-ok" : status.kind === "logged_out" ? "text-error" : "text-on-surface-variant/50";

  return (
    <div className="flex items-center gap-2 px-2.5 py-1.5 border-t border-outline-variant/60 first:border-t-0">
      <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${dotClass}`} />
      <span className={`flex-1 ${status.kind === "missing" ? "text-on-surface-variant/50" : "text-on-surface"}`}>
        {label}
      </span>
      <span className={statusClass}>{statusText}</span>
    </div>
  );
}

interface StatusBarProps {
  metrics: SystemMetrics | null;
}

export function StatusBar({ metrics }: StatusBarProps) {
  const info = useSystemInfo();
  const phase = useWorkspace((s) => s.phase);
  const workspacePath = phase.kind === "ready" ? phase.workspace.path : "";
  const [agentsOpen, setAgentsOpen] = useState(false);
  const agentsRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!agentsOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (agentsRef.current && !agentsRef.current.contains(e.target as Node)) {
        setAgentsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [agentsOpen]);

  if (!info || !metrics) {
    return <footer className="h-6 bg-surface-container-lowest border-t border-outline-variant shrink-0" />;
  }

  const anyAgentReady = metrics.providers.some((p) => p.binary.kind === "ready");

  return (
    <footer className="h-6 flex items-center px-3 bg-surface-container-lowest border-t border-outline-variant text-on-surface-variant font-mono text-[10px] shrink-0 overflow-visible select-none cursor-default">
      <div className="flex items-center gap-0 flex-1 min-w-0 overflow-x-auto no-scrollbar">
        {/* Git item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <span className="text-on-surface-variant/70">{workspacePath}</span>
          <span className="text-outline-variant">·</span>
          <span className="text-primary">{metrics.git_branch}</span>
          {metrics.git_insertions > 0 && <span className="text-ok">+{metrics.git_insertions}</span>}
          {metrics.git_deletions > 0 && <span className="text-error">-{metrics.git_deletions}</span>}
        </div>

        {/* CPU item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <FontAwesomeIcon icon={faMicrochip} className={iconClass.xs} />
          <span>{info.cpu_model}</span>
          <span className="text-outline-variant">·</span>
          <span className={getUsageColor(metrics.cpu_usage_pct)}>{Math.round(metrics.cpu_usage_pct)}%</span>
        </div>

        {/* GPU item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <FontAwesomeIcon icon={faGamepad} className={iconClass.xs} />
          {info.gpu_model !== null ? (
            <>
              <span>{info.gpu_model}</span>
              <span className="text-outline-variant">·</span>
              <span className={getUsageColor(metrics.gpu_usage_pct ?? 0)}>
                {metrics.gpu_usage_pct !== null ? `${Math.round(metrics.gpu_usage_pct)}%` : "n/a"}
              </span>
              <span className="text-outline-variant">·</span>
              <span>{info.gpu_vram_gb !== null ? `${info.gpu_vram_gb} GB VRAM` : "n/a"}</span>
            </>
          ) : (
            <span>n/a</span>
          )}
        </div>

        {/* RAM item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <FontAwesomeIcon icon={faDatabase} className={iconClass.xs} />
          <span>{info.ram_total_gb} GB RAM</span>
        </div>

        {/* Hostname item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <span>
            {info.username}@{info.hostname}
          </span>
        </div>
      </div>

      {/* Agents status */}
      <div className="relative shrink-0" ref={agentsRef}>
        <button
          className={`flex items-center gap-1.5 px-2 h-6 transition-colors hover:bg-surface-container-high hover:text-on-surface ${agentsOpen ? "bg-surface-container-high text-on-surface" : ""}`}
          onClick={() => setAgentsOpen((open) => !open)}
        >
          <span className={`w-1.5 h-1.5 rounded-full ${anyAgentReady ? "bg-ok" : "bg-error"}`} />
          <span>agents</span>
          <FontAwesomeIcon
            icon={faChevronDown}
            className={`${iconClass.xs} transition-transform ${agentsOpen ? "rotate-180" : ""}`}
          />
        </button>
        {agentsOpen && (
          <div className="absolute bottom-full right-0 mb-1 min-w-[190px] bg-surface-container-high border border-outline-variant rounded shadow-lg shadow-black/30 overflow-hidden z-50 text-[11px]">
            <div className="px-2.5 pt-2 pb-1.5 text-[10px] font-medium uppercase tracking-normal text-on-surface-variant/60">
              Agents
            </div>
            {metrics.providers.map((p) => (
              <AgentRow key={p.provider} label={PROVIDER_LABELS[p.provider]} status={p.binary} />
            ))}
          </div>
        )}
      </div>
    </footer>
  );
}
