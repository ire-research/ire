import { useSystemStatus } from "../hooks/useSystemStatus";
import { Icon } from "./Icon";

function getUsageColor(usage: number): string {
  if (usage < 70) return "text-ok";
  if (usage <= 90) return "text-warn";
  return "text-error";
}

export function StatusBar() {
  const status = useSystemStatus();

  if (!status) {
    return <footer className="h-6 bg-surface-container-lowest border-t border-outline-variant shrink-0" />;
  }

  return (
    <footer className="h-6 flex items-center px-3 bg-surface-container-lowest border-t border-outline-variant text-on-surface-variant font-mono text-[10px] shrink-0 overflow-hidden select-none cursor-default">
      <div className="flex items-center gap-0 w-full overflow-x-auto no-scrollbar">
        {/* Git item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <span className="text-on-surface-variant/70">{status.workspace_path}</span>
          <span className="text-outline-variant">·</span>
          <span className="text-primary">{status.git_branch}</span>
          {status.git_insertions > 0 && <span className="text-ok">+{status.git_insertions}</span>}
          {status.git_deletions > 0 && <span className="text-error">-{status.git_deletions}</span>}
        </div>

        {/* CPU item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <Icon name="memory" className="w-[11px] h-[11px]" />
          <span>{status.cpu_model}</span>
          <span className="text-outline-variant">·</span>
          <span className={getUsageColor(status.cpu_usage_pct)}>{Math.round(status.cpu_usage_pct)}%</span>
        </div>

        {/* GPU item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <Icon name="developer_board" className="w-[11px] h-[11px]" />
          {status.gpu_model !== null ? (
            <>
              <span>{status.gpu_model}</span>
              <span className="text-outline-variant">·</span>
              <span className={getUsageColor(status.gpu_usage_pct ?? 0)}>
                {status.gpu_usage_pct !== null ? `${Math.round(status.gpu_usage_pct)}%` : "n/a"}
              </span>
              <span className="text-outline-variant">·</span>
              <span>{status.gpu_vram_gb !== null ? `${status.gpu_vram_gb} GB VRAM` : "n/a"}</span>
            </>
          ) : (
            <span>n/a</span>
          )}
        </div>

        {/* RAM item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <Icon name="storage" className="w-[11px] h-[11px]" />
          <span>{status.ram_total_gb} GB RAM</span>
        </div>

        {/* Hostname item */}
        <div className="flex items-center gap-1.5 px-2 border-r border-outline-variant shrink-0 h-6">
          <span>
            {status.username}@{status.hostname}
          </span>
        </div>

        {/* Agent status */}
        <div className="flex items-center gap-1.5 px-2 shrink-0 h-6 ml-auto">
          <span className={status.cc_connected ? "text-ok" : "text-error"}>claude-code</span>
          <span className={`w-1.5 h-1.5 rounded-full ${status.cc_connected ? "bg-ok" : "bg-error"}`} />
          <span className="text-outline-variant">·</span>
          <span className={status.codex_connected ? "text-ok" : "text-on-surface-variant/40"}>codex</span>
          <span className={`w-1.5 h-1.5 rounded-full ${status.codex_connected ? "bg-ok" : "bg-surface-container-high"}`} />
        </div>
      </div>
    </footer>
  );
}
