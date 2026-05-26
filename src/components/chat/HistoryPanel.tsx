import { useEffect, useRef, useState } from "react";
import { ipc } from "../../ipc";
import type { ChatSessionSummary } from "../../types";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  excludeSessionUuids: string[];
  onRestore: (sessionUuid: string, tabLabel: string, startedAt: string) => void;
}

function formatTime(iso: string): string {
  const then = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - then.getTime();
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) return `${diffHour}h ago`;
  const diffDay = Math.floor(diffHour / 24);
  if (diffDay === 1) return "Yesterday";
  if (diffDay < 7) return `${diffDay}d ago`;
  return then.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function HistoryPanel({ isOpen, onClose, excludeSessionUuids, onRestore }: Props) {
  const [sessions, setSessions] = useState<ChatSessionSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const excluded = new Set(excludeSessionUuids);
  const visibleSessions = sessions.filter((s) => !excluded.has(s.session_uuid));

  // Load sessions each time the panel opens.
  useEffect(() => {
    if (!isOpen) return;
    setLoading(true);
    ipc
      .chatHistoryList()
      .then(setSessions)
      .catch(() => setSessions([]))
      .finally(() => setLoading(false));
  }, [isOpen]);

  // Close on outside click.
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    // Delay so the toggle-button click that opened the panel doesn't immediately close it.
    const id = setTimeout(() => document.addEventListener("mousedown", handler), 0);
    return () => {
      clearTimeout(id);
      document.removeEventListener("mousedown", handler);
    };
  }, [isOpen, onClose]);

  const handleDelete = async (sessionUuid: string, e: React.MouseEvent) => {
    e.stopPropagation();
    await ipc.chatHistoryDelete(sessionUuid).catch(() => {});
    setSessions((prev) => prev.filter((s) => s.session_uuid !== sessionUuid));
  };

  return (
    <div
      ref={panelRef}
      className={[
        "absolute right-0 top-full z-50",
        "w-[300px] max-h-[320px] overflow-y-auto",
        "bg-surface-container-high border border-outline-variant rounded-lg",
        "shadow-[0_8px_32px_rgba(0,0,0,0.55),0_2px_8px_rgba(0,0,0,0.3)]",
        "transition-all duration-[160ms] ease-[cubic-bezier(0.4,0,0.2,1)]",
        "origin-top-right",
        isOpen
          ? "opacity-100 scale-100 translate-y-0 pointer-events-auto"
          : "opacity-0 scale-[0.98] -translate-y-1.5 pointer-events-none",
      ].join(" ")}
    >
      {(loading || visibleSessions.length === 0) && (
        <div className="flex items-center px-3 py-2 text-[12px] text-on-surface-variant/60">
          {loading ? "Loading…" : "No history yet."}
        </div>
      )}

      {!loading &&
        visibleSessions.map((s) => (
          <div
            key={s.session_uuid}
            className="group flex items-center border-b border-outline-variant/50 last:border-b-0 hover:bg-surface-container-highest transition-colors"
          >
            <button
              className="flex-1 flex items-center gap-2.5 px-3 py-2 text-left min-w-0"
              onClick={() => {
                onRestore(s.session_uuid, s.tab_label, s.started_at);
                onClose();
              }}
            >
              <i
                className={`fa-brands ${s.provider === "claude" ? "fa-claude" : "fa-openai"} text-[13px] text-on-surface-variant/80 shrink-0`}
              />
              <span className="flex-1 min-w-0 text-[12px] text-on-surface truncate">
                {s.first_user_msg ?? (
                  <span className="text-on-surface-variant italic">Untitled</span>
                )}
              </span>
              <span className="text-[11px] text-on-surface-variant/60 shrink-0 whitespace-nowrap">
                {formatTime(s.ended_at)}
              </span>
            </button>
            <button
              className="opacity-0 group-hover:opacity-100 transition-opacity pr-2.5 py-2 text-on-surface-variant/50 hover:text-error shrink-0"
              title="Delete session"
              onClick={(e) => handleDelete(s.session_uuid, e)}
            >
              <i className="fa-solid fa-trash text-[10px]" />
            </button>
          </div>
        ))}
    </div>
  );
}
