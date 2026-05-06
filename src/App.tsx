import { useCallback, useEffect } from "react";
import { Layout } from "./components/Layout";
import { SetupScreen } from "./components/setup/SetupScreen";
import { ToastStack } from "./components/ToastStack";
import { useWorkspace } from "./state/workspace";
import { ipc, onBackendError } from "./ipc";
import { useToasts } from "./state/toasts";

export default function App() {
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);
  const theme = useWorkspace((s) => s.theme);
  const hydrateFromUserConfig = useWorkspace((s) => s.hydrateFromUserConfig);

  useEffect(() => {
    const unlisten = onBackendError(({ scope, message }) => {
      useToasts.getState().push({ kind: "error", scope, message });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Apply theme to DOM globally so SetupScreen also respects user preference.
  useEffect(() => {
    if (theme === "light") {
      document.documentElement.dataset.theme = "light";
    } else {
      delete document.documentElement.dataset.theme;
    }
  }, [theme]);

  const refreshSetup = useCallback(async () => {
    try {
      const [status, config] = await Promise.all([
        ipc.setupStatus(),
        ipc.readUserConfig().catch(() => null),
      ]);
      if (config) hydrateFromUserConfig(config);
      setPhase({ kind: "setup", status });
    } catch (e) {
      // Not running inside a Tauri window (e.g. plain browser dev).
      setPhase({
        kind: "setup",
        status: { binary: { kind: "missing" } },
      });
    }
  }, [setPhase, hydrateFromUserConfig]);

  useEffect(() => {
    refreshSetup();
  }, [refreshSetup]);

  if (phase.kind === "loading") {
    return (
      <>
        <div className="app-loading">Loading…</div>
        <ToastStack />
      </>
    );
  }
  if (phase.kind === "setup") {
    return (
      <>
        <SetupScreen status={phase.status} onRefresh={refreshSetup} />
        <ToastStack />
      </>
    );
  }
  return (
    <>
      <Layout />
      <ToastStack />
    </>
  );
}
