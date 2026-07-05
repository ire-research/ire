import { useEffect, useState } from "react";
import { ipc } from "../ipc";

export function SettingsModal() {
  const [analyticsEnabled, setAnalyticsEnabled] = useState(false);

  useEffect(() => {
    ipc.readUserConfig().then((config) => {
      setAnalyticsEnabled(config.analytics_enabled === true);
    }).catch(() => {});
  }, []);

  const toggleAnalytics = async () => {
    const next = !analyticsEnabled;
    setAnalyticsEnabled(next);
    const config = await ipc.readUserConfig().catch(() => ({}));
    await ipc.saveUserConfig({ ...config, analytics_enabled: next }).catch(() => {});
  };

  return (
    <div className="absolute top-full right-0 mt-1.5 w-[280px] bg-surface-container-high border border-outline-variant rounded-lg shadow-lg z-50 overflow-hidden">
      <div className="px-3 py-2.5 flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          <p className="text-[12px] font-medium text-on-surface">Anonymous usage analytics</p>
          <p className="text-[11px] text-on-surface-variant mt-0.5">
            Anonymous, minimal data (app launches, session length) helps us understand usage and improve IRE. No file paths or chat content is ever sent.
          </p>
        </div>
        <button
          role="switch"
          aria-checked={analyticsEnabled}
          onClick={toggleAnalytics}
          className={`shrink-0 w-8 h-[18px] rounded-full transition-colors relative ${analyticsEnabled ? "bg-primary" : "bg-surface-container-highest border border-outline-variant"}`}
        >
          <span
            className={`absolute left-0 top-1/2 -translate-y-1/2 w-3.5 h-3.5 rounded-full transition-transform ${analyticsEnabled ? "translate-x-[16px] bg-white" : "translate-x-[2px] bg-on-surface-variant"}`}
          />
        </button>
      </div>
    </div>
  );
}
