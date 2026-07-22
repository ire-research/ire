import { useState } from "react";
import { ipc, pickDirectory, type BinaryStatus, type SetupStatus } from "../../ipc";
import { useWorkspace } from "../../state/workspace";
import { loadPersisted } from "../../state/persistedStore";
import {
  optionsForAvailableProviders,
  useChatOptions,
  type Provider,
} from "../../state/chatOptions";
import { PROVIDER_LABELS } from "../../types";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faTrash, faChevronRight, faFolderOpen, iconClass } from "../../icons";

interface Props {
  status: SetupStatus;
  onRefresh: () => Promise<void>;
}

export function SetupScreen({ status, onRefresh }: Props) {
  const setPhase = useWorkspace((s) => s.setPhase);
  const hydrateFromPersisted = useWorkspace((s) => s.hydrateFromPersisted);
  const pushRecentWorkspace = useWorkspace((s) => s.pushRecentWorkspace);
  const recentWorkspaces = useWorkspace((s) => s.recentWorkspaces);
  const setRecentWorkspaces = useWorkspace((s) => s.setRecentWorkspaces);
  const setOptions = useChatOptions((s) => s.setOptions);
  const setAvailableProviders = useChatOptions((s) => s.setAvailableProviders);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const applyPersisted = (
    persisted: Parameters<typeof hydrateFromPersisted>[0],
    availableProviders: Provider[],
  ) => {
    void hydrateFromPersisted(persisted).catch((e) => setError(String(e)));
    setOptions(optionsForAvailableProviders(
      persisted.model,
      persisted.provider,
      persisted.effort,
      availableProviders,
    ));
  };

  const removeRecentWorkspace = async (path: string) => {
    setError(null);
    const previous = recentWorkspaces;
    const updated = recentWorkspaces.filter((p) => p !== path);
    setRecentWorkspaces(updated);
    try {
      const config = await ipc.readUserConfig().catch(() => ({ recent_workspaces: previous }));
      await ipc.saveUserConfig({ ...config, recent_workspaces: updated });
    } catch (e) {
      setRecentWorkspaces(previous);
      setError(String(e));
    }
  };

  const availableProviders: Provider[] = status.providers
    .filter((p) => p.binary.kind === "ready")
    .map((p) => p.provider);
  const canOpenWorkspace = availableProviders.length > 0;

  const openWorkspace = async (path: string) => {
    setError(null);
    setBusy(true);
    try {
      const workspace = await ipc.openWorkspace(path);
      setAvailableProviders(availableProviders);
      pushRecentWorkspace(path);
      const persisted = await loadPersisted(path).catch(() => null);
      if (persisted) {
        applyPersisted(persisted, availableProviders);
      } else {
        setOptions(optionsForAvailableProviders(null, null, null, availableProviders));
      }
      setPhase({ kind: "ready", workspace });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handlePick = async () => {
    setError(null);
    const path = await pickDirectory("Open workspace");
    if (!path) return;
    setBusy(true);
    try {
      const workspace = await ipc.openWorkspace(path);
      setAvailableProviders(availableProviders);
      pushRecentWorkspace(path);
      const persisted = await loadPersisted(path).catch(() => null);
      if (persisted) {
        applyPersisted(persisted, availableProviders);
      } else {
        setOptions(optionsForAvailableProviders(null, null, null, availableProviders));
      }
      setPhase({ kind: "ready", workspace });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="flex-1 flex flex-col items-center justify-center overflow-y-auto px-6 py-10 bg-background text-on-surface h-screen">
      <div className="w-full max-w-[520px] flex flex-col gap-6">
        {/* Title */}
        <div className="flex flex-col gap-2">
          <h1 className="text-[22px] font-semibold text-on-surface tracking-tight leading-snug">
            Open a workspace.
          </h1>
          <p className="text-[14px] text-on-surface-variant leading-relaxed">
            Pick any folder — if it's new, IRE will initialize{" "}
            <code className="font-mono text-[12px] px-1 py-0.5 bg-surface-container border border-outline-variant rounded">
              .ire/
            </code>
            {" "}automatically. Each workspace should map 1:1 to a Git repository.
          </p>
        </div>

        {/* Recent workspaces */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center">
            <span className="text-[12px] font-medium text-on-surface-variant">Recent</span>
          </div>
          <div className="flex flex-col border border-outline-variant rounded overflow-hidden">
            {recentWorkspaces.length === 0 ? (
              <div className="text-[13px] text-on-surface-variant px-4 py-3">
                No recent workspaces
              </div>
            ) : (
              recentWorkspaces.slice(0, 5).map((path, index) => {
                const name = path.split("/").filter(Boolean).pop() ?? path;
                const isActive = index === 0;
                const isLast = index === Math.min(recentWorkspaces.length, 5) - 1;
                return (
                  <div
                    key={path}
                    className={`flex items-center justify-between w-full px-4 py-3 text-left transition-colors group ${
                      isActive
                        ? "bg-surface-container-low border-l-2 border-l-primary hover:bg-surface-container-highest"
                        : "border-l-2 border-l-transparent hover:bg-surface-container-low"
                    } ${!isLast ? "border-b border-b-outline-variant" : ""} ${
                      busy || !canOpenWorkspace ? "opacity-50 cursor-not-allowed" : ""
                    }`}
                  >
                    <button
                      onClick={() => openWorkspace(path)}
                      disabled={busy || !canOpenWorkspace}
                      className="flex flex-col gap-0.5 min-w-0 flex-1 text-left disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      <span
                        className={`text-[14px] text-on-surface truncate ${
                          isActive ? "font-semibold" : "font-medium"
                        }`}
                      >
                        {name}
                      </span>
                      <span className="font-mono text-[11px] text-on-surface-variant group-hover:text-on-surface transition-colors truncate">
                        {path}
                      </span>
                    </button>
                    <button
                      onClick={() => removeRecentWorkspace(path)}
                      disabled={busy}
                      title="Remove from recent"
                      aria-label={`Remove ${name} from recent workspaces`}
                      className="app-danger-icon-button opacity-0 group-hover:opacity-100 focus:opacity-100 transition-opacity disabled:cursor-not-allowed disabled:opacity-40 shrink-0 ml-3 p-1"
                    >
                      <FontAwesomeIcon icon={faTrash} className={iconClass.md} />
                    </button>
                    <FontAwesomeIcon icon={faChevronRight} className={`${iconClass.lg} text-outline-variant group-hover:text-on-surface transition-colors shrink-0 ml-1`} />
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* Action button */}
        <button
          onClick={handlePick}
          disabled={busy || !canOpenWorkspace}
          className={`w-full h-9 border border-outline-variant rounded text-[14px] font-medium text-on-surface hover:bg-surface-container-low hover:border-outline transition-colors flex items-center justify-center gap-2 ${
            busy || !canOpenWorkspace ? "opacity-50 cursor-not-allowed" : ""
          }`}
        >
          <FontAwesomeIcon icon={faFolderOpen} className={`${iconClass.lg} text-on-surface-variant`} />
          Open workspace…
        </button>

        {/* Divider */}
        <div className="w-full h-px bg-outline-variant"></div>

        {/* Status rows */}
        <div className="flex flex-col gap-2">
          {status.providers.map((p) => (
            <BinaryRow
              key={p.provider}
              label={PROVIDER_LABELS[p.provider]}
              status={p.binary}
              busy={busy}
              onRefresh={onRefresh}
            />
          ))}
          {!canOpenWorkspace && (
            <div className="font-mono text-[11px] text-error">
              install claude-code or codex to continue
            </div>
          )}
        </div>

        {/* Error display */}
        {error && (
          <div className="text-[12px] text-error mt-1">
            {error}
          </div>
        )}
      </div>
    </main>
  );
}

function BinaryRow({
  label,
  status,
  busy,
  onRefresh,
}: {
  label: string;
  status: BinaryStatus;
  busy: boolean;
  onRefresh: () => Promise<void>;
}) {
  const ready = status.kind === "ready";
  const statusText =
    status.kind === "logged_out" ? "not logged in" : "not found";
  return (
    <div className="flex items-center gap-2">
      <span className={`w-1.5 h-1.5 rounded-full ${ready ? "bg-ok" : "bg-error"}`} />
      <span className="font-mono text-[11px] text-on-surface-variant">
        {ready ? (
          <>{label} · ready</>
        ) : (
          <div className="flex items-center gap-2">
            <span>{label} · {statusText}</span>
            <button
              onClick={onRefresh}
              disabled={busy}
              className="text-on-surface-variant hover:text-on-surface transition-colors underline"
            >
              retry
            </button>
          </div>
        )}
      </span>
    </div>
  );
}
