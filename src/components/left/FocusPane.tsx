import { useEffect, useRef, useState } from "react";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";
import { useWorkspaceData } from "../../state/workspaceData";
import { Icon } from "../Icon";

export function FocusPane() {
  const pulse = useWorkspaceData((s) => s.pulse);
  const [editingField, setEditingField] = useState<"research_question" | "this_week" | null>(null);
  const [draftRq, setDraftRq] = useState(pulse.research_question);
  const [draftTw, setDraftTw] = useState(pulse.this_week);
  const rqRef = useRef<HTMLTextAreaElement>(null);
  const twRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setDraftRq(pulse.research_question);
    setDraftTw(pulse.this_week);
  }, [pulse]);

  useEffect(() => {
    if (editingField === "research_question") rqRef.current?.focus();
    else if (editingField === "this_week") twRef.current?.focus();
  }, [editingField]);

  const handleSave = async (field: "research_question" | "this_week") => {
    const content = field === "research_question" ? draftRq : draftTw;
    const trimmed = content.trim();
    const original = field === "research_question" ? pulse.research_question : pulse.this_week;

    if (trimmed === original.trim()) {
      setEditingField(null);
      return;
    }

    try {
      await ipc.savePulseField(field, trimmed);
      setEditingField(null);
    } catch (e) {
      toastError("save focus", e);
    }
  };

  const handleCancel = (field: "research_question" | "this_week") => {
    if (field === "research_question") {
      setDraftRq(pulse.research_question);
    } else {
      setDraftTw(pulse.this_week);
    }
    setEditingField(null);
  };

  const handleKeyDown = (e: React.KeyboardEvent, field: "research_question" | "this_week") => {
    if (e.key === "Escape") {
      handleCancel(field);
    } else if (e.key === "Enter" && e.ctrlKey) {
      handleSave(field);
    }
  };

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      {/* Header */}
      <div className="sticky top-0 z-10 flex items-center gap-2 px-0 py-1 mb-2 bg-surface-container-low">
        <Icon name="target" className="w-[16px] h-[16px] shrink-0 text-on-surface-variant" />
        <span className="text-[14px] text-on-surface-variant">Focus</span>
      </div>

      {/* Research Question */}
      <div className="mb-3 group/rq pl-1">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-[11px] text-on-surface-variant font-medium">Research question</span>
          <button
            onClick={() => setEditingField("research_question")}
            className="app-icon-button opacity-0 group-hover/rq:opacity-100 transition-opacity p-0.5"
            title="Edit research question"
          >
            <Icon name="edit_document" className="w-[14px] h-[14px]" />
          </button>
        </div>
        {editingField === "research_question" ? (
          <textarea
            ref={rqRef}
            className="w-full bg-transparent border border-outline-variant rounded text-[14px] text-on-surface leading-relaxed px-1 py-0.5 focus:outline-none focus:border-outline resize-none"
            value={draftRq}
            onChange={(e) => setDraftRq(e.target.value)}
            onBlur={() => handleSave("research_question")}
            onKeyDown={(e) => handleKeyDown(e, "research_question")}
          />
        ) : (
          <p className="text-[14px] text-on-surface leading-relaxed">
            {pulse.research_question ? (
              pulse.research_question
            ) : (
              <span className="italic text-on-surface-variant">No research question set</span>
            )}
          </p>
        )}
      </div>

      {/* This Week */}
      <div className="group/tw pl-1">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-[11px] text-on-surface-variant font-medium">This week</span>
          <button
            onClick={() => setEditingField("this_week")}
            className="app-icon-button opacity-0 group-hover/tw:opacity-100 transition-opacity p-0.5"
            title="Edit this week"
          >
            <Icon name="edit_document" className="w-[14px] h-[14px]" />
          </button>
        </div>
        {editingField === "this_week" ? (
          <textarea
            ref={twRef}
            className="w-full bg-transparent border border-outline-variant rounded text-[14px] text-on-surface leading-relaxed px-1 py-0.5 focus:outline-none focus:border-outline resize-none"
            value={draftTw}
            onChange={(e) => setDraftTw(e.target.value)}
            onBlur={() => handleSave("this_week")}
            onKeyDown={(e) => handleKeyDown(e, "this_week")}
          />
        ) : (
          <p className="text-[14px] text-on-surface leading-relaxed">
            {pulse.this_week ? (
              pulse.this_week
            ) : (
              <span className="italic text-on-surface-variant">No focus set for this week</span>
            )}
          </p>
        )}
      </div>
    </div>
  );
}
