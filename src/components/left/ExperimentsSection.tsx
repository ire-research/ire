import { useRef, useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import type { ExperimentRow } from "../../types";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faFlask, faPencil, faTrash, iconClass } from "../../icons";

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
  const [deletingUuid, setDeletingUuid] = useState<string | null>(null);
  const [renamingUuid, setRenamingUuid] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const handleDelete = async (e: React.MouseEvent, uuid: string) => {
    e.stopPropagation();
    setDeletingUuid(uuid);
    try {
      await ipc.experimentDelete(uuid);
    } catch (err) {
      toastError("delete experiment", err);
    } finally {
      setDeletingUuid(null);
    }
  };

  const startRename = (e: React.MouseEvent, exp: ExperimentRow) => {
    e.stopPropagation();
    setRenamingUuid(exp.uuid);
    setRenameValue(exp.name);
  };

  const commitRename = async (uuid: string) => {
    const trimmed = renameValue.trim();
    if (!trimmed) {
      setRenamingUuid(null);
      return;
    }
    try {
      await ipc.experimentRename(uuid, trimmed);
    } catch (err) {
      toastError("rename experiment", err);
    } finally {
      setRenamingUuid(null);
    }
  };

  const handleRenameKeyDown = (e: React.KeyboardEvent, uuid: string) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commitRename(uuid);
    } else if (e.key === "Escape") {
      setRenamingUuid(null);
    }
  };

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="sticky top-0 z-10 flex items-center gap-2 py-1 mb-2 bg-surface-container-low text-on-surface-variant text-[14px]">
        <FontAwesomeIcon icon={faFlask} className={`${iconClass.lg} shrink-0`} />
        Experiments
      </div>
      <div className="space-y-0.5">
        {experiments.length > 0 ? (
          experiments.map((exp) => {
            const pill = getStatusPill(exp.status);
            const isRenaming = renamingUuid === exp.uuid;
            return (
              <div
                key={exp.uuid}
                className="group w-full flex items-center px-2 py-1.5 rounded hover:bg-surface-container-high transition-colors"
              >
                {isRenaming ? (
                  <input
                    ref={inputRef}
                    autoFocus
                    className="flex-1 font-mono text-[13px] text-on-surface bg-transparent border-b border-outline outline-none min-w-0"
                    value={renameValue}
                    onChange={(e) => setRenameValue(e.target.value)}
                    onKeyDown={(e) => handleRenameKeyDown(e, exp.uuid)}
                    onBlur={() => setRenamingUuid(null)}
                    onClick={(e) => e.stopPropagation()}
                  />
                ) : (
                  <>
                    <button
                      className="flex-1 min-w-0 cursor-pointer text-left"
                      onClick={() => onOpen(exp.uuid, exp.name)}
                    >
                      <span className="font-mono text-[13px] text-on-surface truncate block">{exp.name}</span>
                    </button>
                    <button
                      className="app-icon-button opacity-0 group-hover:opacity-100 mx-1 h-5 w-5 shrink-0"
                      title="Rename experiment"
                      onClick={(e) => startRename(e, exp)}
                    >
                      <FontAwesomeIcon icon={faPencil} className={iconClass.md} />
                    </button>
                    <span className={`text-[10px] uppercase border ${pill.borderColor} px-1 rounded ${pill.textColor} ${pill.bgColor} shrink-0`}>
                      {pill.text}
                    </span>
                    <button
                      className="app-danger-icon-button opacity-0 group-hover:opacity-100 ml-1 p-0.5 shrink-0"
                      title="Delete experiment"
                      disabled={deletingUuid === exp.uuid}
                      onClick={(e) => handleDelete(e, exp.uuid)}
                    >
                      <FontAwesomeIcon icon={faTrash} className={iconClass.md} />
                    </button>
                  </>
                )}
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
