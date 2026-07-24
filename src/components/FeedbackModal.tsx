import { useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faMessage, iconClass } from "../icons";
import { ipc } from "../ipc";
import { useToasts } from "../state/toasts";

interface Props {
  onClose: () => void;
  initialMessage?: string;
}

export function FeedbackModal({ onClose, initialMessage = "" }: Props) {
  const [message, setMessage] = useState(initialMessage);
  const [email, setEmail] = useState("");
  const [sending, setSending] = useState(false);
  const push = useToasts((s) => s.push);

  const send = async () => {
    setSending(true);
    try {
      await ipc.submitFeedback(message.trim(), email.trim() || undefined);
      push({ kind: "success", message: "Feedback sent — thanks!" });
      onClose();
    } catch (e) {
      push({ kind: "error", scope: "send feedback", message: e instanceof Error ? e.message : String(e) });
      setSending(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center">
      <div className="w-[360px] bg-surface-container border border-outline-variant rounded-lg flex flex-col shadow-2xl">
        <div className="flex items-center gap-2 px-4 pt-3.5 pb-3 border-b border-outline-variant shrink-0">
          <FontAwesomeIcon icon={faMessage} className={`${iconClass.lg} shrink-0 text-on-surface-variant`} />
          <span className="flex-1 text-[13px] font-medium text-on-surface">Send feedback</span>
        </div>

        <div className="px-4 pt-3.5 pb-4 flex flex-col gap-3">
          <p className="text-[12px] text-on-surface-variant">
            Bugs, ideas, anything that's bugging you. Sent straight to the team — no reply, but every message gets read.
          </p>
          <textarea
            autoFocus
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            placeholder="What's on your mind?"
            rows={4}
            className="w-full resize-none bg-surface-container-low border border-outline-variant rounded px-2.5 py-2 text-[12px] text-on-surface placeholder:text-on-surface-variant/60 focus:outline-none focus:border-outline"
          />
          <div className="flex flex-col gap-1">
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Email (optional)"
              className="w-full bg-surface-container-low border border-outline-variant rounded px-2.5 py-1.5 text-[12px] text-on-surface placeholder:text-on-surface-variant/60 focus:outline-none focus:border-outline"
            />
            <span className="text-[10px] text-on-surface-variant/70">
              By adding your email, you may be contacted about this feedback.
            </span>
          </div>
          <div className="flex items-center justify-end gap-2">
            <button
              onClick={onClose}
              className="border border-outline-variant text-on-surface-variant px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={send}
              disabled={!message.trim() || sending}
              className="border border-outline text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {sending ? "Sending…" : "Send"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
