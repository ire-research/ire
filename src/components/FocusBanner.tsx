import { useEffect, useRef, useState } from "react";
import { ipc } from "../ipc";
import { toastError } from "../state/toasts";

interface FocusBannerProps {
  focus: string;
}

export function FocusBanner({ focus }: FocusBannerProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(focus);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!editing) setDraft(focus);
  }, [focus, editing]);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  const commit = async () => {
    const next = draft.trim();
    if (next === focus.trim()) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      await ipc.updatePulseFocus(next);
      setEditing(false);
    } catch (e) {
      toastError("update focus", e);
    } finally {
      setSaving(false);
    }
  };

  const cancel = () => {
    setDraft(focus);
    setEditing(false);
  };

  return (
    <div className="focus-banner">
      <div className="focus-banner__label">FOCUS</div>
      {editing ? (
        <input
          ref={inputRef}
          className="focus-banner__input"
          value={draft}
          disabled={saving}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            else if (e.key === "Escape") cancel();
          }}
        />
      ) : (
        <button
          className="focus-banner__text"
          onClick={() => setEditing(true)}
          title="Click to edit"
        >
          {focus || <span className="focus-banner__placeholder">Set focus…</span>}
        </button>
      )}
    </div>
  );
}
