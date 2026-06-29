import { useEffect, useRef, useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import { useWorkspaceData } from "../../state/workspaceData";
import type { IdeaItem } from "../../types";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faLightbulb, faPlus, faTrash, iconClass } from "../../icons";

export function IdeasPane() {
  const ideas = useWorkspaceData((s) => s.ideas);
  const [draggedIdx, setDraggedIdx] = useState<number | null>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const draftRef = useRef<HTMLInputElement>(null);

  const save = (next: IdeaItem[]) =>
    ipc.saveIdeas(next).catch((e) => toastError("save ideas", e));

  useEffect(() => {
    if (draft !== null) {
      draftRef.current?.focus();
    }
  }, [draft]);

  const handleAddClick = () => {
    setDraft("");
  };

  const handleSubmitDraft = async () => {
    const text = draft?.trim();
    if (!text) {
      setDraft(null);
      return;
    }
    await save([{ text }, ...ideas]);
    setDraft(null);
  };

  const handleTrash = async (idx: number) => {
    await save(ideas.filter((_, i) => i !== idx));
  };

  const handleDragStart = (idx: number) => {
    setDraggedIdx(idx);
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
  };

  const handleDrop = async (targetIdx: number) => {
    if (draggedIdx === null || draggedIdx === targetIdx) {
      setDraggedIdx(null);
      return;
    }
    const reordered = [...ideas];
    const [moved] = reordered.splice(draggedIdx, 1);
    reordered.splice(targetIdx, 0, moved);
    await save(reordered);
    setDraggedIdx(null);
  };

  const handleDraftKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleSubmitDraft();
    } else if (e.key === "Escape") {
      setDraft(null);
    }
  };

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="px-4 pt-4 shrink-0 flex items-center gap-2 py-1 mb-2 bg-surface-container-low">
        <FontAwesomeIcon icon={faLightbulb} className={`${iconClass.lg} shrink-0 text-on-surface-variant`} />
        <span className="text-[14px] text-on-surface-variant flex-1">
          Ideas
        </span>
        <button
          className="app-icon-button cursor-pointer p-0.5"
          onClick={handleAddClick}
          title="Add idea"
        >
          <FontAwesomeIcon icon={faPlus} className={iconClass.md} />
        </button>
      </div>
      <div className="px-4 pb-3 overflow-y-auto flex-1">
      {draft !== null || ideas.length > 0 ? (
        <div className="space-y-2">
          {draft !== null && (
            <div className="bg-surface-container border border-outline-variant p-2 rounded text-[14px] text-on-surface flex items-start justify-between gap-2">
              <input
                ref={draftRef}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={handleDraftKeyDown}
                onBlur={handleSubmitDraft}
                className="flex-1 min-w-0 bg-transparent border-none outline-none text-[14px] text-on-surface placeholder-on-surface-variant/50"
                placeholder="New idea"
              />
            </div>
          )}
          {ideas.map((idea, idx) => (
            <div
              key={idx}
              draggable
              onDragStart={() => handleDragStart(idx)}
              onDragOver={handleDragOver}
              onDrop={() => handleDrop(idx)}
              className="group idea-entry bg-surface-container border border-outline-variant p-2 rounded text-[14px] text-on-surface cursor-pointer hover:border-outline transition-colors flex items-start justify-between gap-2"
            >
              <span className="flex-1">{idea.text}</span>
              <button
                className="app-danger-icon-button p-0.5 shrink-0 mt-0.5 opacity-0 group-hover:opacity-100"
                title="Remove idea"
                onClick={() => handleTrash(idx)}
              >
                <FontAwesomeIcon icon={faTrash} className={iconClass.md} />
              </button>
            </div>
          ))}
        </div>
      ) : (
        <p className="text-[13px] text-on-surface-variant italic">
          No ideas yet.
        </p>
      )}
      </div>
    </div>
  );
}
