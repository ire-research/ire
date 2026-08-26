import { useEffect, useRef, useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faFlask, iconClass } from "../../icons";
import { MODELS, effortLevelsForModel, useChatOptions } from "../../state/chatOptions";
import type { EffortLevel, ExperimentRow, Provider, Tab } from "../../types";

interface Props {
  experiment: ExperimentRow;
  existingChatTabs: Tab[];
  busy: boolean;
  onOpenNewChat: (provider: Provider, model: string, effort: EffortLevel | null) => void;
  onOpenExistingChat: (tabId: string) => void;
  onResolve: () => void;
  onClose: () => void;
}

const STATUS_LABEL: Record<string, { text: string; color: string }> = {
  completed: { text: "Completed", color: "text-ok" },
  failed: { text: "Failed", color: "text-error" },
  cancelled: { text: "Cancelled", color: "text-on-surface-variant" },
};

export function ExperimentResolutionModal({
  experiment,
  existingChatTabs,
  busy,
  onOpenNewChat,
  onOpenExistingChat,
  onResolve,
  onClose,
}: Props) {
  const { provider: defaultProvider, model: defaultModel, effort: defaultEffort } = useChatOptions();
  const [provider, setProvider] = useState<Provider>(defaultProvider);
  const [model, setModel] = useState(defaultModel);
  const [effort, setEffort] = useState<EffortLevel | null>(defaultEffort);
  const [existingTabId, setExistingTabId] = useState(existingChatTabs[0]?.id ?? "");
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, onClose]);

  const dismiss = () => {
    if (!busy) onClose();
  };

  const models = MODELS.filter((m) => m.provider === provider);
  const effortLevels = effortLevelsForModel(provider, model);

  const pickProvider = (p: Provider) => {
    setProvider(p);
    const first = MODELS.find((m) => m.provider === p);
    if (first) {
      setModel(first.id);
      setEffort(effortLevelsForModel(p, first.id)[0]?.value ?? null);
    }
  };

  const pickModel = (id: string) => {
    setModel(id);
    setEffort(effortLevelsForModel(provider, id)[0]?.value ?? null);
  };

  const status = STATUS_LABEL[experiment.status] ?? { text: experiment.status, color: "text-on-surface-variant" };

  return (
    <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onClick={dismiss}>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="experiment-resolution-title"
        className="w-[420px] bg-surface-container border border-outline-variant rounded-lg flex flex-col shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 px-4 pt-3.5 pb-3 border-b border-outline-variant shrink-0">
          <FontAwesomeIcon icon={faFlask} className={`${iconClass.lg} shrink-0 text-on-surface-variant`} />
          <span id="experiment-resolution-title" className="flex-1 text-[13px] font-medium text-on-surface truncate">
            {experiment.name}
          </span>
          <span className={`text-[11px] uppercase tracking-wide ${status.color}`}>{status.text}</span>
        </div>

        <div className="px-4 pt-3.5 pb-4 flex flex-col gap-4">
          <p className="text-[12px] text-on-surface-variant">
            This run finished without resuming a chat. Discuss the result, or mark it resolved.
          </p>

          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <select
                value={provider}
                onChange={(e) => pickProvider(e.target.value as Provider)}
                disabled={busy}
                className="flex-1 bg-surface-container-high border border-outline-variant rounded text-[12px] text-on-surface px-2 py-1.5 outline-none disabled:opacity-50"
              >
                <option value="claude">Claude Code</option>
                <option value="codex">Codex</option>
              </select>
              <select
                value={model}
                onChange={(e) => pickModel(e.target.value)}
                disabled={busy}
                className="flex-1 bg-surface-container-high border border-outline-variant rounded text-[12px] text-on-surface px-2 py-1.5 outline-none disabled:opacity-50"
              >
                {models.map((m) => (
                  <option key={m.id} value={m.id}>{m.label}</option>
                ))}
              </select>
              {effortLevels.length > 0 && (
                <select
                  value={effort ?? ""}
                  onChange={(e) => setEffort(e.target.value as EffortLevel)}
                  disabled={busy}
                  className="w-[84px] bg-surface-container-high border border-outline-variant rounded text-[12px] text-on-surface px-2 py-1.5 outline-none disabled:opacity-50"
                >
                  {effortLevels.map((l) => (
                    <option key={l.value} value={l.value}>{l.label}</option>
                  ))}
                </select>
              )}
            </div>
            <button
              onClick={() => onOpenNewChat(provider, model, effort)}
              disabled={busy}
              className="border border-outline-variant text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors disabled:opacity-50"
            >
              Open in new chat
            </button>
          </div>

          {existingChatTabs.length > 0 && (
            <div className="flex flex-col gap-2 pt-1 border-t border-outline-variant/50">
              <div className="flex items-center gap-2 pt-2">
                <select
                  value={existingTabId}
                  onChange={(e) => setExistingTabId(e.target.value)}
                  disabled={busy}
                  className="flex-1 bg-surface-container-high border border-outline-variant rounded text-[12px] text-on-surface px-2 py-1.5 outline-none disabled:opacity-50"
                >
                  {existingChatTabs.map((t) => (
                    <option key={t.id} value={t.id}>{t.label}</option>
                  ))}
                </select>
                <button
                  onClick={() => onOpenExistingChat(existingTabId)}
                  disabled={busy}
                  className="border border-outline-variant text-on-surface px-3 py-1.5 rounded text-[12px] hover:bg-surface-container-high transition-colors disabled:opacity-50 shrink-0"
                >
                  Open here
                </button>
              </div>
            </div>
          )}

          <div className="flex items-center justify-between pt-1">
            <button
              ref={closeRef}
              onClick={dismiss}
              disabled={busy}
              className="text-on-surface-variant hover:text-on-surface text-[12px] transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={onResolve}
              disabled={busy}
              className="border border-info/40 text-info px-3 py-1.5 rounded text-[12px] hover:bg-info/10 transition-colors disabled:opacity-50"
            >
              Resolve without chat
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
