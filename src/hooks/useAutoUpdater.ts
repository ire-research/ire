import { useEffect, useRef } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { useToasts } from "../state/toasts";

const RELEASES_URL = "https://github.com/ire-research/ire/releases/tag";

/**
 * Checks for an app update once on launch. If one is found, a toast offers
 * a Download button — the update is only fetched and installed once the
 * user clicks it, and isn't applied until the user restarts the app, so an
 * in-progress experiment is never interrupted.
 */
export function useAutoUpdater() {
  const hasChecked = useRef(false);

  useEffect(() => {
    // Guard against React.StrictMode's dev-only double-invoke of effects,
    // which would otherwise check twice and offer the download twice.
    if (hasChecked.current) return;
    hasChecked.current = true;

    check()
      .then((update) => {
        if (!update) return;
        const link = { label: `v${update.version}`, url: `${RELEASES_URL}/v${update.version}` };

        useToasts.getState().push({
          kind: "info",
          scope: "updater",
          message: "Update available:",
          link,
          persistent: true,
          action: {
            label: "Download",
            onClick: (id) => {
              useToasts.getState().dismiss(id);
              useToasts.getState().push({
                kind: "info",
                scope: "updater",
                message: "Downloading update...",
                persistent: true,
              });
              update
                .downloadAndInstall()
                .then(() => {
                  useToasts.getState().push({
                    kind: "success",
                    scope: "updater",
                    message: "Update installed — restart ire to use it.",
                    link,
                    persistent: true,
                  });
                })
                .catch((e) => console.error("Update download failed:", e));
            },
          },
        });
      })
      .catch((e) => {
        // Not running inside a Tauri window (e.g. plain browser dev), or offline.
        console.error("Update check failed:", e);
      });
  }, []);
}
