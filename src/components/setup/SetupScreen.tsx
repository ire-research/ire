import { useState } from "react";
import { ipc, pickDirectory, type SetupStatus } from "../../ipc";
import { useWorkspace } from "../../state/workspace";

interface Props {
  status: SetupStatus;
  onRefresh: () => Promise<void>;
}

export function SetupScreen({ status, onRefresh }: Props) {
  const setPhase = useWorkspace((s) => s.setPhase);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const binaryFound = status.binary.kind === "found";

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
      </div>
    </div>
  );
}
