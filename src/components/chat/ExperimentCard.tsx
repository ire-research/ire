import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ToolCallState } from "../../types";
import { Icon } from "../Icon";
import { ToolIoField } from "./ToolCard";

interface Props {
  tool: ToolCallState;
}

export function ExperimentCard({ tool }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const status = tool.meta.experiment_status ?? "starting";
  const lines = tool.logLines ?? [];
  const isLive = status === "starting" || status === "running";
  const preview = tool.input.preview ?? null;

  const handleCancel = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!tool.meta.experiment_uuid) return;
    setCancelling(true);
    try {
      await ipc.experimentCancel(tool.meta.experiment_uuid);
    } catch (err) {
      toastError("cancel experiment", err);
    } finally {
      setCancelling(false);
    }
  };

  return (
    <div className="w-full font-mono text-xs">
      {/* ── Clickable summary row ── */}
      <div
        className="flex items-center gap-2 py-[3px] cursor-pointer group"
        onClick={() => setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === "Enter" && setExpanded((v) => !v)}
      >
        {/* IRE badge */}
        <div className="w-[18px] h-[18px] flex items-center justify-center shrink-0">
          <span className="font-sans font-bold text-[9px] tracking-wide leading-none text-on-surface-variant/50 select-none">
            IRE
          </span>
        </div>

        {/* Name badge */}
        <span className="shrink-0 border border-outline rounded-sm px-[7px] py-px text-[11px] leading-5 text-on-surface-variant bg-surface-container-low transition-colors group-hover:bg-surface-container group-hover:border-outline-strong">
          experiment_start
        </span>

        {/* Status dot */}
        <span className={dotClass(status)} />

        {/* Inline arg preview — hidden when expanded */}
        {!expanded && preview && (
          <span className="text-on-surface-variant/50 text-[11px] flex-1 min-w-0 truncate">
            {preview}
          </span>
        )}

        {/* Spacer when no preview or expanded */}
        {(expanded || !preview) && <span className="flex-1" />}

        {/* Metadata badges (PID / exit code) */}
        {tool.meta.experiment_pid !== undefined && (
          <span className="text-[10px] text-on-surface-variant/50 shrink-0">
            PID {tool.meta.experiment_pid}
          </span>
        )}
        {tool.meta.experiment_exit_code !== undefined && status === "failed" && (
          <span className="text-[10px] text-error/70 shrink-0">
            exit {tool.meta.experiment_exit_code}
          </span>
        )}

        {/* Cancel button — only when live */}
        {isLive && tool.meta.experiment_uuid && (
          <button
            className="text-[10px] text-on-surface-variant/60 border border-outline rounded-sm px-1.5 py-px hover:border-error hover:text-error transition-colors shrink-0"
            disabled={cancelling}
            onClick={handleCancel}
          >
            {cancelling ? "Cancelling…" : "Cancel"}
          </button>
        )}

        {/* Chevron */}
        <Icon
          name="expand_more"
          className={`w-[13px] h-[13px] text-on-surface-variant ml-0 shrink-0 opacity-0 group-hover:opacity-60 transition-all duration-150 ${expanded ? "rotate-180 !opacity-60" : ""}`}
        />
      </div>

      {/* ── Expanded body ── */}
      {expanded && (
        <div className="ml-[26px] mt-1 border border-outline-variant rounded overflow-hidden bg-surface-container-lowest font-mono text-[11px] leading-relaxed">
          {tool.input.full && (
            <ToolIoField
              label="IN"
              content={tool.input.full}
              format={tool.input.format}
            />
          )}
          {lines.length > 0 ? (
            <div className="grid grid-cols-[36px_minmax(0,1fr)] border-t border-outline-variant">
              <div className="px-2 py-1.5 pt-2 text-on-surface-variant/40 uppercase text-[9.5px] tracking-wide leading-none">
                OUT
              </div>
              <div className="px-2.5 py-1.5 text-on-surface-variant max-h-32 overflow-y-auto">
                {lines.slice(-10).map((line, i) => (
                  <div key={i}>{line}</div>
                ))}
              </div>
            </div>
          ) : tool.output?.full ? (
            <ToolIoField
              label="OUT"
              content={tool.output.full}
              format={tool.output.format}
            />
          ) : (
            <div className="grid grid-cols-[36px_minmax(0,1fr)] border-t border-outline-variant">
              <div className="px-2 py-1.5 pt-2 text-on-surface-variant/40 uppercase text-[9.5px] tracking-wide leading-none">
                OUT
              </div>
              <div className="px-2.5 py-1.5 text-on-surface-variant/30">
                No output yet.
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function dotClass(status: string): string {
  if (status === "starting" || status === "running") {
    return "w-1.5 h-1.5 rounded-full bg-warn animate-pulse shrink-0";
  }
  if (status === "completed") {
    return "w-1.5 h-1.5 rounded-full bg-ok shrink-0";
  }
  return "w-1.5 h-1.5 rounded-full bg-error shrink-0";
}
