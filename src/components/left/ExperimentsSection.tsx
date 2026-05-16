import type { ExperimentRow } from "../../types";

interface Props {
  experiments: ExperimentRow[];
  onOpen: (uuid: string, name: string) => void;
}

function getStatusPill(status: string): { text: string; textColor: string; borderColor: string; bgColor: string } {
  const normalized = status.toLowerCase();
  if (normalized === "running") {
    return { text: "Run", textColor: "text-warn", borderColor: "border-warn/30", bgColor: "bg-warn/10" };
  }
  if (normalized === "completed") {
    return { text: "Done", textColor: "text-ok", borderColor: "border-ok/30", bgColor: "bg-ok/10" };
  }
  if (normalized === "failed" || normalized === "cancelled") {
    return { text: "Fail", textColor: "text-error", borderColor: "border-error/30", bgColor: "bg-error/10" };
  }
  const truncated = normalized.slice(0, 4).toUpperCase();
  return { text: truncated, textColor: "text-on-surface-variant", borderColor: "border-on-surface-variant/30", bgColor: "bg-on-surface-variant/10" };
}

export function ExperimentsSection({ experiments, onOpen }: Props) {
  if (experiments.length === 0) {
    return <div />;
  }

  return (
    <div className="overflow-y-auto flex-1 py-1">
      <div className="flex items-center gap-2 px-4 py-2 text-on-surface-variant text-[14px]">
        <span className="material-symbols-outlined text-[16px] shrink-0">science</span>
        Experiments
      </div>
      <div className="px-4 pb-1 space-y-0.5">
        {experiments.map((exp) => {
          const pill = getStatusPill(exp.status);
          return (
            <button
              key={exp.uuid}
              onClick={() => onOpen(exp.uuid, exp.name)}
              className="w-full flex items-center justify-between px-2 py-1.5 rounded hover:bg-surface-container-high transition-colors cursor-pointer"
            >
              <span className="font-mono text-[13px] text-on-surface truncate pr-2">{exp.name}</span>
              <span className={`text-[10px] uppercase border ${pill.borderColor} px-1 rounded ${pill.textColor} ${pill.bgColor} shrink-0`}>
                {pill.text}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
