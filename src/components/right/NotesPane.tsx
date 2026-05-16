import { useState, useRef, useEffect } from "react";

interface Props {
  content: string;
  onSave: (content: string) => Promise<void>;
}

export function NotesPane({ content, onSave }: Props) {
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(content);
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
    await onSave(draft.trim());
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape") {
      setIsEditing(false);
    } else if (e.key === "Enter" && e.ctrlKey) {
      handleSave();
    }
  };

  const handleBlur = () => {
    handleSave();
  };

  const lines = content
    .split("\n")
    .filter((line) => line.trim().length > 0);

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2">
        <span className="material-symbols-outlined text-[16px] shrink-0 text-on-surface-variant">
          edit_note
        </span>
        <span className="text-[14px] text-on-surface-variant flex-1">
          Notes
        </span>
        <span
          className="material-symbols-outlined text-[14px] cursor-pointer hover:text-on-surface text-on-surface-variant"
          onClick={handleEditClick}
        >
          edit_document
        </span>
      </div>

      {isEditing ? (
        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          className="w-full h-32 bg-transparent border border-outline-variant rounded text-[14px] text-on-surface px-2 py-1.5 focus:outline-none focus:border-outline resize-none"
        />
      ) : (
        <>
          {lines.length > 0 ? (
            <ul className="text-[14px] text-on-surface space-y-2 list-disc pl-4 marker:text-outline-variant">
              {lines.map((line, idx) => (
                <li key={idx} className="pl-1">
                  {line}
                </li>
              ))}
            </ul>
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
