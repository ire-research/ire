import { useState } from "react";
import { ipc, pickDirectory, type SetupStatus } from "../../ipc";
import { useWorkspace } from "../../state/workspace";
import { useChatOptions, EFFORT_LEVELS } from "../../state/chatOptions";
import type { EffortLevel } from "../../types";
import { Icon } from "../Icon";

interface Props {
  status: SetupStatus;
  onRefresh: () => Promise<void>;
}

export function SetupScreen({ status, onRefresh }: Props) {
  const setPhase = useWorkspace((s) => s.setPhase);
  const hydrateFromPersisted = useWorkspace((s) => s.hydrateFromPersisted);
  const pushRecentWorkspace = useWorkspace((s) => s.pushRecentWorkspace);
  const recentWorkspaces = useWorkspace((s) => s.recentWorkspaces);
  const setEffort = useChatOptions((s) => s.setEffort);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const applyPersisted = (persisted: Parameters<typeof hydrateFromPersisted>[0]) => {
    hydrateFromPersisted(persisted);
    if (persisted.effort && EFFORT_LEVELS.some((e) => e.value === persisted.effort)) {
      setEffort(persisted.effort as EffortLevel);
    }
  };

  const binaryFound = status.binary.kind === "found";

  const openWorkspace = async (path: string) => {
    setError(null);
    setBusy(true);
    try {
      const workspace = await ipc.openWorkspace(path);
      pushRecentWorkspace(path);
      const persisted = await ipc.readWorkspaceState().catch(() => null);
      if (persisted) applyPersisted(persisted);
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
      pushRecentWorkspace(path);
      const persisted = await ipc.readWorkspaceState().catch(() => null);
      if (persisted) applyPersisted(persisted);
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
                  <button
                    key={path}
                    onClick={() => openWorkspace(path)}
                    disabled={busy}
                    className={`flex items-center justify-between w-full px-4 py-3 text-left transition-colors group ${
                      isActive
                        ? "bg-surface-container-low border-l-2 border-l-primary hover:bg-surface-container-highest"
                        : "border-l-2 border-l-transparent hover:bg-surface-container-low"
                    } ${!isLast ? "border-b border-b-outline-variant" : ""}`}
                  >
                    <div className="flex flex-col gap-0.5 min-w-0">
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
                    </div>
                    <Icon name="chevron_right" className="w-[16px] h-[16px] text-outline-variant group-hover:text-on-surface transition-colors shrink-0 ml-3" />
                  </button>
                );
              })
            )}
          </div>
        </div>

        {/* Action buttons */}
        <div className="flex gap-3">
          <button
            onClick={() => handlePick("open")}
            disabled={busy}
            className={`flex-1 h-9 border border-outline-variant rounded text-[14px] font-medium text-on-surface hover:bg-surface-container-low hover:border-outline transition-colors flex items-center justify-center gap-2 ${
              busy ? "opacity-50 cursor-not-allowed" : ""
            }`}
          >
            <Icon name="folder_open" className="w-[16px] h-[16px] text-on-surface-variant" />
            Open folder…
          </button>
          <button
            onClick={() => handlePick("init")}
            disabled={busy}
            className={`flex-1 h-9 border border-outline-variant rounded text-[14px] font-medium text-on-surface hover:bg-surface-container-low hover:border-outline transition-colors flex items-center justify-center gap-2 ${
              busy ? "opacity-50 cursor-not-allowed" : ""
            }`}
          >
            <Icon name="add" className="w-[16px] h-[16px] text-on-surface-variant" />
            New workspace…
          </button>
        </div>

        {/* Divider */}
        <div className="w-full h-px bg-outline-variant"></div>

        {/* Status row */}
        <div className="flex items-center">
          <div className="flex items-center gap-2">
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                binaryFound ? "bg-ok" : "bg-error"
              }`}
            ></span>
            <span className="font-mono text-[11px] text-on-surface-variant">
              {binaryFound ? (
                <>claude-code · authenticated</>
              ) : (
                <div className="flex items-center gap-2">
                  <span>claude-code · not found</span>
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
