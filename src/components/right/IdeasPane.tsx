import { useEffect, useRef, useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import { useWorkspaceData } from "../../state/workspaceData";
import type { IdeaItem } from "../../types";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faLightbulb, faPlus, faTrash, iconClass } from "../../icons";

export function IdeasPane() {
  const ideas = useWorkspaceData((s) => s.ideas);
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const draftRef = useRef<HTMLInputElement>(null);

  const save = (next: IdeaItem[]) =>
    ipc.saveIdeasJson(next).catch((e) => toastError("save ideas", e));

  const activeIdeas = ideas
    .filter((idea) => !idea.trashed)
    .sort((a, b) => a.order - b.order);

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
    if (!text) return;
    const newIdea: IdeaItem = {
      id: crypto.randomUUID(),
      text,
      trashed: false,
      order: 0,
    };

    const updated = [newIdea, ...activeIdeas].map((idea, idx) => ({ ...idea, order: idx }));
    const allIdeas = [...updated, ...ideas.filter((idea) => idea.trashed)];
    await save(allIdeas);
    setDraft(null);
  };

  const handleTrash = async (id: string) => {
    const updated = ideas.map((idea) =>
      idea.id === id ? { ...idea, trashed: true } : idea
    );
    await save(updated);
  };

  const handleDragStart = (id: string) => {
    setDraggedId(id);
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
  };

  const handleDrop = async (targetId: string) => {
    if (!draggedId || draggedId === targetId) {
      setDraggedId(null);
      return;
    }

    const draggedIdx = activeIdeas.findIndex((idea) => idea.id === draggedId);
    const targetIdx = activeIdeas.findIndex((idea) => idea.id === targetId);

    if (draggedIdx === -1 || targetIdx === -1) {
      setDraggedId(null);
      return;
    }

    const reordered = [...activeIdeas];
    const [moved] = reordered.splice(draggedIdx, 1);
    reordered.splice(targetIdx, 0, moved);

    const updated = reordered.map((idea, idx) => ({ ...idea, order: idx }));
    const allIdeas = [...updated, ...ideas.filter((idea) => idea.trashed)];
    await save(allIdeas);
    setDraggedId(null);
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
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="sticky top-0 z-10 flex items-center gap-2 py-1 mb-2 bg-surface-container-low">
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

      {draft !== null || activeIdeas.length > 0 ? (
        <div className="space-y-2">
          {draft !== null && (
            <div className="bg-surface-container border border-outline-variant p-2 rounded text-[14px] text-on-surface flex items-start justify-between gap-2">
              <input
                ref={draftRef}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={handleDraftKeyDown}
                className="flex-1 min-w-0 bg-transparent border-none outline-none text-[14px] text-on-surface placeholder-on-surface-variant/50"
                placeholder="New idea"
              />
            </div>
          )}
          {activeIdeas.map((idea) => (
            <div
              key={idea.id}
              draggable
              onDragStart={() => handleDragStart(idea.id)}
              onDragOver={handleDragOver}
              onDrop={() => handleDrop(idea.id)}
              className="group idea-entry bg-surface-container border border-outline-variant p-2 rounded text-[14px] text-on-surface cursor-pointer hover:border-outline transition-colors flex items-start justify-between gap-2"
            >
              <span className="flex-1">{idea.text}</span>
              <button
                className="app-danger-icon-button p-0.5 shrink-0 mt-0.5 opacity-0 group-hover:opacity-100"
                title="Remove idea"
                onClick={() => handleTrash(idea.id)}
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
  );
}
