import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faMicrochip, faGamepad, faDatabase, iconClass } from "../icons";
import { useSystemInfo, useSystemMetrics } from "../hooks/useSystemStatus";
import { useWorkspace } from "../state/workspace";

function getUsageColor(usage: number): string {
  if (usage < 70) return "text-ok";
  if (usage <= 90) return "text-warn";
  return "text-error";
}

export function StatusBar() {
  const info = useSystemInfo();
  const metrics = useSystemMetrics();
  const phase = useWorkspace((s) => s.phase);
  const workspacePath = phase.kind === "ready" ? phase.workspace.path : "";

  if (!info || !metrics) {
    return <footer className="h-6 bg-surface-container-lowest border-t border-outline-variant shrink-0" />;
  }

  return (
    <footer className="h-6 flex items-center px-3 bg-surface-container-lowest border-t border-outline-variant text-on-surface-variant font-mono text-[10px] shrink-0 overflow-hidden select-none cursor-default">
      <div className="flex items-center gap-0 w-full overflow-x-auto no-scrollbar">
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

        {/* Agent status */}
        <div className="flex items-center gap-1.5 px-2 shrink-0 h-6 ml-auto">
          <span className={info.cc_connected ? "" : "text-error"}>claude-code</span>
          <span className={`w-1.5 h-1.5 rounded-full ${info.cc_connected ? "bg-ok" : "bg-error"}`} />
          <span className="text-outline-variant">·</span>
          <span className={info.codex_connected ? "" : "text-on-surface-variant/40"}>codex</span>
          <span className={`w-1.5 h-1.5 rounded-full ${info.codex_connected ? "bg-ok" : "bg-surface-container-high"}`} />
        </div>
      </div>
    </footer>
  );
}
