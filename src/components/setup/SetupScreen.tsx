import { useState } from "react";
import { ipc, pickDirectory, type SetupStatus } from "../../ipc";
import { useWorkspace } from "../../state/workspace";
import { useChatOptions, EFFORT_LEVELS } from "../../state/chatOptions";
import type { EffortLevel } from "../../types";

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
    <div className="setup">
      <div className="setup__card">
        <h1>Integrated Research Environment</h1>

        <section className="setup__step">
          <h2>1. Claude Code binary</h2>
          {status.binary.kind === "found" ? (
            <div className="setup__ok">
              <strong>Found:</strong> <code>{status.binary.path}</code>
              {status.binary.version && (
                <div className="setup__hint">{status.binary.version}</div>
              )}
            </div>
          ) : (
            <div className="setup__warn">
              <p>Claude Code CLI not found.</p>
              <p>
                Install with{" "}
                <code>npm install -g @anthropic-ai/claude-code</code> or follow{" "}
                <a
                  href="https://docs.claude.com/en/docs/claude-code/quickstart"
                  target="_blank"
                  rel="noreferrer"
                >
                  the setup guide
                </a>
                .
              </p>
              <button onClick={onRefresh}>Retry</button>
            </div>
          )}
        </section>

        <section className="setup__step">
          <h2>2. Choose a workspace</h2>
          <div className="setup__actions">
            <button
              disabled={!binaryFound || busy}
              onClick={() => handlePick("open")}
            >
              Open existing
            </button>
            <button
              disabled={!binaryFound || busy}
              onClick={() => handlePick("init")}
            >
              Initialize new
            </button>
          </div>
          {error && <div className="setup__error">{error}</div>}
        </section>

        {recentWorkspaces.length > 0 && (
          <section className="setup__step">
            <h2>Recent workspaces</h2>
            <ul className="setup__recents">
              {recentWorkspaces.map((path) => {
                const name = path.split("/").filter(Boolean).pop() ?? path;
                return (
                  <li key={path}>
                    <button
                      className="setup__recent-item"
                      disabled={!binaryFound || busy}
                      onClick={() => openWorkspace(path)}
                    >
                      <span className="setup__recent-name">{name}</span>
                      <span className="setup__recent-path">{path}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}
