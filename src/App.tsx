import { useCallback, useEffect } from "react";
import { Layout } from "./components/Layout";
import { SetupScreen } from "./components/setup/SetupScreen";
import { useWorkspace } from "./state/workspace";
import { ipc } from "./ipc";

export default function App() {
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);

  const refreshSetup = useCallback(async () => {
    const status = await ipc.setupStatus();
    setPhase({ kind: "setup", status });
  }, [setPhase]);

  useEffect(() => {
    refreshSetup();
  }, [refreshSetup]);

  if (phase.kind === "loading") {
    return <div className="app-loading">Loading…</div>;
  }
  if (phase.kind === "setup") {
    return <SetupScreen status={phase.status} onRefresh={refreshSetup} />;
  }
  return <Layout />;
}
