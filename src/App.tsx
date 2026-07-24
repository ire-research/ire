import { useCallback, useEffect, useState } from "react";
import { Layout } from "./components/Layout";
import { SetupScreen } from "./components/setup/SetupScreen";
import { ToastStack } from "./components/ToastStack";
import { AnalyticsConsentModal } from "./components/AnalyticsConsentModal";
import { useWorkspace } from "./state/workspace";
import { useWorkspaceData } from "./state/workspaceData";
import { ipc, onBackendError, onWorkspaceEvent } from "./ipc";
import { useToasts } from "./state/toasts";
import { useFeedbackModal } from "./state/feedbackModal";
import { useAutoUpdater } from "./hooks/useAutoUpdater";

export default function App() {
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);
  const hydrateFromUserConfig = useWorkspace((s) => s.hydrateFromUserConfig);
  const [needsAnalyticsConsent, setNeedsAnalyticsConsent] = useState(false);

  useAutoUpdater();

  useEffect(() => {
    const unlisten = onBackendError(({ scope, message }) => {
      // Resume-id persistence failures are silent otherwise (the turn itself
      // still succeeds) — surface a report-able toast so it doesn't go unnoticed.
      if (scope === "resume id") {
        useToasts.getState().push({
          kind: "error",
          scope,
          message,
          persistent: true,
          action: {
            label: "Report",
            onClick: (id) => {
              useToasts.getState().dismiss(id);
              useFeedbackModal.getState().openWith(message);
            },
          },
        });
        return;
      }
      useToasts.getState().push({ kind: "error", scope, message });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Reset slice on workspace close. The initial state for a newly-opened
  // workspace arrives via the `workspace-event` channel (Rust emits a `hydrate`
  // burst at the end of `open_workspace`), so there is no
  // explicit hydrate IPC call from the frontend.
  useEffect(() => {
    if (phase.kind !== "ready") {
      useWorkspaceData.getState().reset();
    }
  }, [phase.kind]);

  // Single subscriber for workspace-event — dispatches every variant into the slice.
  // The cancelled flag handles Strict Mode unmount before listen() resolves.
  useEffect(() => {
    let cancelled = false;
    const p = onWorkspaceEvent((event) => useWorkspaceData.getState().apply(event));
    let unlisten: (() => void) | null = null;
    p.then((fn) => { if (cancelled) fn(); else unlisten = fn; });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const refreshSetup = useCallback(async () => {
    try {
      const [status, config] = await Promise.all([
        ipc.setupStatus(),
        ipc.readUserConfig().catch(() => null),
      ]);
      if (config) {
        hydrateFromUserConfig(config);
        setNeedsAnalyticsConsent(config.analytics_enabled == null);
      }
      setPhase({ kind: "setup", status });
    } catch (e) {
      // Not running inside a Tauri window (e.g. plain browser dev).
      setPhase({
        kind: "setup",
        status: { providers: [] },
      });
    }
  }, [setPhase, hydrateFromUserConfig]);

  useEffect(() => {
    refreshSetup();
  }, [refreshSetup]);

  const handleAnalyticsConsent = async (enabled: boolean) => {
    setNeedsAnalyticsConsent(false);
    const config = await ipc.readUserConfig().catch(() => ({}));
    await ipc.saveUserConfig({ ...config, analytics_enabled: enabled }).catch(() => {});
  };

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
        {needsAnalyticsConsent && <AnalyticsConsentModal onAnswer={handleAnalyticsConsent} />}
      </>
    );
  }
  return (
    <>
      <Layout />
      {needsAnalyticsConsent && <AnalyticsConsentModal onAnswer={handleAnalyticsConsent} />}
      <ToastStack />
    </>
  );
}
