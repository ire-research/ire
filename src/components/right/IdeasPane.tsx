import { useState } from "react";
import type { IdeaItem } from "../../types";

interface Props {
  ideas: IdeaItem[];
  onSave: (ideas: IdeaItem[]) => Promise<void>;
}

export function IdeasPane({ ideas, onSave }: Props) {
  const [draggedId, setDraggedId] = useState<string | null>(null);

  const activeIdeas = ideas
    .filter((idea) => !idea.trashed)
    .sort((a, b) => a.order - b.order);

  const handleAddClick = async () => {
    const newIdea: IdeaItem = {
      id: crypto.randomUUID(),
      text: "",
      trashed: false,
      order: 0,
    };

    const updated = [newIdea, ...activeIdeas.map((idea, idx) => ({ ...idea, order: idx + 1 }))];
    const allIdeas = [...updated, ...ideas.filter((idea) => idea.trashed)];
    await onSave(allIdeas);
  };

  const handleTrash = async (id: string) => {
    const updated = ideas.map((idea) =>
      idea.id === id ? { ...idea, trashed: true } : idea
    );
    await onSave(updated);
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
    await onSave(allIdeas);
    setDraggedId(null);
  };

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2">
        <span className="material-symbols-outlined text-[16px] shrink-0 text-on-surface-variant">
          lightbulb
        </span>
        <span className="text-[14px] text-on-surface-variant flex-1">
          Ideas
        </span>
        <span
          className="material-symbols-outlined text-[14px] cursor-pointer hover:text-on-surface text-on-surface-variant"
          onClick={handleAddClick}
        >
          add
        </span>
      </div>

      {activeIdeas.length > 0 ? (
        <div className="space-y-2">
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
                className="p-0.5 text-on-surface-variant hover:text-error transition-colors shrink-0 mt-0.5 opacity-0 group-hover:opacity-100"
                title="Remove idea"
                onClick={() => handleTrash(idea.id)}
              >
                <span className="material-symbols-outlined text-[14px]">
                  delete
                </span>
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
