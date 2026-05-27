import { useState } from "react";
import { ipc, pickDirectory, type SetupStatus } from "../../ipc";
import { useWorkspace } from "../../state/workspace";
import {
  optionsForAvailableProviders,
  useChatOptions,
  type Provider,
} from "../../state/chatOptions";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faTrash, faChevronRight, faFolderOpen, faPlus, iconClass } from "../../icons";

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
    hydrateFromPersisted(persisted);
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

  const binaryFound = status.binary.kind === "found";
  const codexFound = status.codex_binary.kind === "found";
  const availableProviders: Provider[] = [
    ...(binaryFound ? (["claude"] as const) : []),
    ...(codexFound ? (["codex"] as const) : []),
  ];
  const canOpenWorkspace = binaryFound || codexFound;
  const providerBanner =
    binaryFound && !codexFound
      ? "claude code only"
      : codexFound && !binaryFound
        ? "codex only"
        : null;

  const openWorkspace = async (path: string) => {
    setError(null);
    setBusy(true);
    try {
      const workspace = await ipc.openWorkspace(path);
      setAvailableProviders(availableProviders);
      pushRecentWorkspace(path);
      const persisted = await ipc.readWorkspaceState().catch(() => null);
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

  const handlePick = async (kind: "open" | "init") => {
    setError(null);
    const path = await pickDirectory(
      kind === "open" ? "Open existing IRE workspace" : "Pick a directory to initialize",
    );
    if (!path) return;
    setBusy(true);
    try {
      const workspace =
        kind === "open" ? await ipc.openWorkspace(path) : await ipc.initWorkspace(path);
      setAvailableProviders(availableProviders);
      pushRecentWorkspace(path);
      const persisted = await ipc.readWorkspaceState().catch(() => null);
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
            Open or create a workspace.
          </h1>
          <p className="text-[14px] text-on-surface-variant leading-relaxed">
            Each workspace maps 1:1 to a Git repository. Your code, wiki, experiments, and Claude Code state live together in{" "}
            <code className="font-mono text-[12px] px-1 py-0.5 bg-surface-container border border-outline-variant rounded">
              .ire/
            </code>
            .
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

        {/* Action buttons */}
        <div className="flex gap-3">
          <button
            onClick={() => handlePick("open")}
            disabled={busy || !canOpenWorkspace}
            className={`flex-1 h-9 border border-outline-variant rounded text-[14px] font-medium text-on-surface hover:bg-surface-container-low hover:border-outline transition-colors flex items-center justify-center gap-2 ${
              busy || !canOpenWorkspace ? "opacity-50 cursor-not-allowed" : ""
            }`}
          >
            <FontAwesomeIcon icon={faFolderOpen} className={`${iconClass.lg} text-on-surface-variant`} />
            Open folder…
          </button>
          <button
            onClick={() => handlePick("init")}
            disabled={busy || !canOpenWorkspace}
            className={`flex-1 h-9 border border-outline-variant rounded text-[14px] font-medium text-on-surface hover:bg-surface-container-low hover:border-outline transition-colors flex items-center justify-center gap-2 ${
              busy || !canOpenWorkspace ? "opacity-50 cursor-not-allowed" : ""
            }`}
          >
            <FontAwesomeIcon icon={faPlus} className={`${iconClass.lg} text-on-surface-variant`} />
            New workspace…
          </button>
        </div>

        {/* Divider */}
        <div className="w-full h-px bg-outline-variant"></div>

        {/* Status rows */}
        <div className="flex flex-col gap-2">
          <BinaryRow label="claude-code" found={binaryFound} busy={busy} onRefresh={onRefresh} />
          <BinaryRow label="codex" found={codexFound} busy={busy} onRefresh={onRefresh} />
          {providerBanner && (
            <div className="font-mono text-[11px] text-on-surface-variant/70">
              {providerBanner}
            </div>
          )}
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
  found,
  busy,
  onRefresh,
}: {
  label: string;
  found: boolean;
  busy: boolean;
  onRefresh: () => Promise<void>;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className={`w-1.5 h-1.5 rounded-full ${found ? "bg-ok" : "bg-error"}`} />
      <span className="font-mono text-[11px] text-on-surface-variant">
        {found ? (
          <>{label} · found</>
        ) : (
          <div className="flex items-center gap-2">
            <span>{label} · not found</span>
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
