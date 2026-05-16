import { useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ExperimentRow } from "../../types";

interface Props {
  experiments: ExperimentRow[];
  onOpen: (uuid: string, name: string) => void;
  onDelete: (uuid: string) => void;
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

export function ExperimentsSection({ experiments, onOpen, onDelete }: Props) {
  const [deletingUuid, setDeletingUuid] = useState<string | null>(null);

  const handleDelete = async (e: React.MouseEvent, uuid: string) => {
    e.stopPropagation();
    setDeletingUuid(uuid);
    try {
      await ipc.experimentDelete(uuid);
      onDelete(uuid);
    } catch (err) {
      toastError("delete experiment", err);
    } finally {
      setDeletingUuid(null);
    }
  };

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2 text-on-surface-variant text-[14px]">
        <span className="material-symbols-outlined text-[16px] shrink-0">science</span>
        Experiments
      </div>
      <div className="space-y-0.5">
        {experiments.length > 0 ? (
          experiments.map((exp) => {
            const pill = getStatusPill(exp.status);
            return (
              <div
                key={exp.uuid}
                className="group w-full flex items-center px-2 py-1.5 rounded hover:bg-surface-container-high transition-colors"
              >
                <button
                  className="flex-1 flex items-center justify-between gap-2 min-w-0 cursor-pointer"
                  onClick={() => onOpen(exp.uuid, exp.name)}
                >
                  <span className="font-mono text-[13px] text-on-surface truncate">{exp.name}</span>
                  <span className={`text-[10px] uppercase border ${pill.borderColor} px-1 rounded ${pill.textColor} ${pill.bgColor} shrink-0`}>
                    {pill.text}
                  </span>
                </button>
                <button
                  className="opacity-0 group-hover:opacity-100 ml-1 p-0.5 text-on-surface-variant hover:text-error transition-colors shrink-0"
                  title="Delete experiment"
                  disabled={deletingUuid === exp.uuid}
                  onClick={(e) => handleDelete(e, exp.uuid)}
                >
                  <span className="material-symbols-outlined text-[14px]">delete</span>
                </button>
              </div>
            );
          })
        ) : (
          <p className="text-[13px] text-on-surface-variant italic">no experiments yet</p>
        )}
      </div>
    </div>
  );
}
