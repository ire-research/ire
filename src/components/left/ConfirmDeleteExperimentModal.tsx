import { useEffect, useRef } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faTrash, iconClass } from "../../icons";

interface Props {
  name: string;
  deleting: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDeleteExperimentModal({ name, deleting, onConfirm, onCancel }: Props) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !deleting) onCancel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [deleting, onCancel]);

  // Dismissing mid-delete would hide an in-flight call, so the backdrop and
  // Escape both go inert until it settles.
  const dismiss = () => {
    if (!deleting) onCancel();
  };

  return (
    <div
      className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center"
      onClick={dismiss}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-delete-experiment-title"
        className="w-[380px] bg-surface-container border border-outline-variant rounded-lg flex flex-col shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 px-4 pt-3.5 pb-3 border-b border-outline-variant shrink-0">
          <FontAwesomeIcon icon={faTrash} className={`${iconClass.lg} shrink-0 text-error`} />
          <span id="confirm-delete-experiment-title" className="flex-1 text-[13px] font-medium text-on-surface">
            Delete experiment
          </span>
        </div>

        <div className="px-4 pt-3.5 pb-4 flex flex-col gap-3">
          <p className="text-[12px] text-on-surface-variant">
            This deletes the whole <span className="text-on-surface">{name}</span> folder — its
            EXPERIMENT.md, and any scripts, result files, and notes saved next to it. Its logs go
            too. If the folder was committed, git history is the only way back.
          </p>
          <div className="flex items-center justify-end gap-2">
            <button
              ref={cancelRef}
              onClick={dismiss}
              disabled={deleting}
              className="border border-outline-variant text-on-surface-variant px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={onConfirm}
              disabled={deleting}
              className="border border-error text-error px-3 py-1.5 rounded text-[12px] hover:bg-error/10 transition-colors disabled:opacity-50"
            >
              {deleting ? "Deleting…" : "Delete everything"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
