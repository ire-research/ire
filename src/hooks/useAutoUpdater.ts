import { useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { useToasts } from "../state/toasts";

/**
 * Checks for an app update once on launch. If one is found, it is
 * downloaded and installed in the background but not applied until the
 * user restarts the app — this avoids killing an in-progress experiment.
 */
export function useAutoUpdater() {
  useEffect(() => {
    check()
      .then(async (update) => {
        if (!update) return;
        await update.downloadAndInstall();
        useToasts.getState().push({
          kind: "success",
          scope: "updater",
          message: `Update to v${update.version} installed — restart ire to use it.`,
        });
      })
      .catch((e) => {
        // Not running inside a Tauri window (e.g. plain browser dev), or offline.
        console.error("Update check failed:", e);
      });
  }, []);
}
