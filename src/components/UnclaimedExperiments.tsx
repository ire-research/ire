import { useState } from "react";
import { ipc } from "../ipc";
import { toastError } from "../state/toasts";
import { useChat } from "../state/chat";
import { useChatOptions } from "../state/chatOptions";
import { useWorkspaceData } from "../state/workspaceData";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faFlask, iconClass } from "../icons";

/**
 * A completed experiment no longer resumes its originating chat automatically
 * (see docs/proposals/experiment-lifecycle-v2.md §2). Instead it sits
 * "unclaimed" until the user opens a chat about it or resolves it directly —
 * this is that resolution surface, one card per unclaimed run.
 */
export function UnclaimedExperiments() {
  const experiments = useWorkspaceData((s) => s.experiments);
  const tabs = useChat((s) => s.tabs);
  const unclaimed = experiments.filter(
    (e) => !e.claimed && (e.status === "completed" || e.status === "failed")
  );
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  const [busyUuid, setBusyUuid] = useState<string | null>(null);
  const [pickerFor, setPickerFor] = useState<string | null>(null);

  const visible = unclaimed.filter((e) => !dismissed.has(e.uuid));
  if (visible.length === 0) return null;

  const openInNewChat = async (uuid: string) => {
    setBusyUuid(uuid);
    try {
      const tabId = useChat.getState().createTab("Untitled");
      useChat.getState().setActiveTab(tabId);
      const sessionUuid = crypto.randomUUID();
      const startedAt = new Date().toISOString();
      const { provider, model, effort } = useChatOptions.getState();
      useChat.getState().setTabHistoryMeta(tabId, sessionUuid, startedAt);
      useChat.getState().setTabAgentOptions(tabId, { provider, model, effort });
      await ipc.experimentWake(uuid, tabId, sessionUuid, provider, model, effort ?? undefined);
    } catch (err) {
      toastError("open experiment in new chat", err);
    } finally {
      setBusyUuid(null);
    }
  };

  const openInExistingChat = async (uuid: string, tabId: string) => {
    const tab = useChat.getState().tabs.find((t) => t.id === tabId);
    if (!tab?.historySessionUuid) return;
    const { provider, model, effort } = tab.agentOptions ?? useChatOptions.getState();
    setBusyUuid(uuid);
    setPickerFor(null);
    try {
      useChat.getState().setActiveTab(tabId);
      await ipc.experimentWake(uuid, tabId, tab.historySessionUuid, provider, model, effort ?? undefined);
    } catch (err) {
      toastError("open experiment in existing chat", err);
    } finally {
      setBusyUuid(null);
    }
  };

  const resolve = async (uuid: string) => {
    setBusyUuid(uuid);
    try {
      await ipc.experimentResolve(uuid);
    } catch (err) {
      toastError("resolve experiment", err);
    } finally {
      setBusyUuid(null);
    }
  };

  const existingChatTabs = tabs.filter((t) => t.kind === "chat" && t.historySessionUuid);

  return (
    <div className="fixed bottom-4 right-4 z-40 flex flex-col gap-2 w-80">
      {visible.map((exp) => {
        const busy = busyUuid === exp.uuid;
        return (
          <div
            key={exp.uuid}
            className="bg-surface-container-high border border-outline/30 rounded shadow-lg p-3 font-mono text-[13px]"
          >
            <div className="flex items-center gap-2 mb-1">
              <FontAwesomeIcon icon={faFlask} className={`${iconClass.md} shrink-0 text-on-surface-variant`} />
              <span className="text-on-surface truncate flex-1">{exp.name}</span>
              <span className={exp.status === "completed" ? "text-ok" : "text-error"}>
                {exp.status}
              </span>
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              <button
                className="border border-outline-variant text-on-surface-variant px-2 py-1 rounded text-[12px] hover:bg-surface-container-high transition-colors disabled:opacity-50"
                disabled={busy}
                onClick={() => openInNewChat(exp.uuid)}
              >
                Open in new chat
              </button>
              <button
                className="border border-outline-variant text-on-surface-variant px-2 py-1 rounded text-[12px] hover:bg-surface-container-high transition-colors disabled:opacity-50"
                disabled={busy || existingChatTabs.length === 0}
                onClick={() => setPickerFor(pickerFor === exp.uuid ? null : exp.uuid)}
              >
                Open in existing chat
              </button>
              <button
                className="border border-outline-variant text-on-surface-variant px-2 py-1 rounded text-[12px] hover:bg-surface-container-high transition-colors disabled:opacity-50"
                disabled={busy}
                onClick={() => resolve(exp.uuid)}
              >
                Resolve
              </button>
              <button
                className="text-on-surface-variant hover:text-on-surface text-[12px] ml-auto disabled:opacity-50"
                disabled={busy}
                onClick={() => setDismissed((s) => new Set(s).add(exp.uuid))}
                title="I'll see it later"
              >
                Dismiss
              </button>
            </div>
            {pickerFor === exp.uuid && (
              <div className="mt-2 border-t border-outline/20 pt-2 flex flex-col gap-1">
                {existingChatTabs.map((t) => (
                  <button
                    key={t.id}
                    className="text-left text-on-surface-variant hover:text-on-surface truncate"
                    onClick={() => openInExistingChat(exp.uuid, t.id)}
                  >
                    {t.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
