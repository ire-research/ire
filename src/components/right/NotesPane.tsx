import { useState, useRef, useEffect } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import { useWorkspaceData } from "../../state/workspaceData";
import { Icon } from "../Icon";
import { MessageMarkdown } from "../chat/MessageMarkdown";

export function NotesPane() {
  const content = useWorkspaceData((s) => s.notes);
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(content);
  const [isSaving, setIsSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (isEditing && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isEditing]);

  const handleEditClick = () => {
    setDraft(content);
    setIsEditing(true);
  };

  const handleSave = async () => {
    const next = draft.trim();
    if (isSaving || next === content.trim()) {
      setIsEditing(false);
      return;
    }
    setIsSaving(true);
    await ipc.saveNotes(next).catch((e) => toastError("save notes", e));
    setIsSaving(false);
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape") {
      setIsEditing(false);
    } else if (e.key === "Enter" && e.ctrlKey) {
      e.preventDefault();
      handleSave();
    }
  };

  const handleBlur = () => {
    handleSave();
  };

  const hasNotes = content.trim().length > 0;

  return (
    <div className="px-4 pt-4 pb-3 overflow-hidden flex-1 min-h-0 flex flex-col">
      <div className="sticky top-0 z-10 flex items-center gap-2 py-1 mb-2 bg-surface-container-low shrink-0">
        <Icon name="edit_note" className="w-[16px] h-[16px] shrink-0 text-on-surface-variant" />
        <span className="text-[14px] text-on-surface-variant flex-1">
          Notes
        </span>
        <button
          className="app-icon-button cursor-pointer p-0.5"
          onClick={handleEditClick}
          title="Edit notes"
        >
          <Icon name="edit_document" className="w-[14px] h-[14px]" />
        </button>
      </div>

      {isEditing ? (
        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          className="w-full flex-1 min-h-0 bg-transparent border border-outline-variant rounded text-[14px] text-on-surface px-2 py-1.5 focus:outline-none focus:border-outline resize-none"
        />
      ) : (
        <>
          {hasNotes ? (
            <div className="text-on-surface overflow-y-auto flex-1 min-h-0">
              <MessageMarkdown content={content} />
            </div>
          ) : (
            <p className="text-[13px] text-on-surface-variant italic">
              No notes yet.
            </p>
          )}
        </>
      )}
    </div>
  );
}
