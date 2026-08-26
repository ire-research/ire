import { useEffect } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useToasts, type ToastKind } from "../state/toasts";

const AUTO_DISMISS_MS = 5000;

const DOT_COLOR: Record<ToastKind, string> = {
  error: "bg-error",
  success: "bg-ok",
  info: "bg-primary",
};

export function ToastStack() {
  const toasts = useToasts((s) => s.toasts);
  const dismiss = useToasts((s) => s.dismiss);

  useEffect(() => {
    const pending = toasts.filter((t) => !t.persistent);
    if (pending.length === 0) return;
    const timers = pending.map((t) =>
      setTimeout(() => dismiss(t.id), AUTO_DISMISS_MS),
    );
    return () => timers.forEach(clearTimeout);
  }, [toasts, dismiss]);

  if (toasts.length === 0) return null;

  const banners = toasts.filter((t) => t.variant === "banner");
  const cards = toasts.filter((t) => t.variant !== "banner");

  return (
    <>
      {banners.map((t) => (
        <div
          key={t.id}
          role="status"
          aria-live="polite"
          className="fixed bottom-0 inset-x-0 z-50 flex items-center justify-between gap-3 w-full bg-[#4c7ae0] text-[#0a0a0a] px-4 py-1.5 text-[12px] shadow-lg shadow-black/40"
        >
          <span className="leading-snug">
            {t.message}
            {t.link && (
              <>
                {" "}
                <button
                  className="underline hover:opacity-70"
                  onClick={() => openUrl(t.link!.url).catch((e) => console.error("Failed to open URL:", e))}
                >
                  {t.link.label}
                </button>
              </>
            )}
          </span>
          <div className="flex items-center gap-3 shrink-0">
            {t.action && (
              <button
                className="border border-black/30 rounded px-2.5 py-0.5 text-[11px] font-medium hover:bg-black/10 transition-colors"
                onClick={() => t.action!.onClick(t.id)}
              >
                {t.action.label}
              </button>
            )}
            <button
              className="text-black/60 hover:text-black leading-none"
              onClick={() => dismiss(t.id)}
              aria-label="Dismiss"
            >
              ×
            </button>
          </div>
        </div>
      ))}
      {cards.length > 0 && (
        <div
          className="fixed bottom-4 right-4 z-50 flex flex-col gap-2"
          role="status"
          aria-live="polite"
        >
          {cards.map((t) => (
            <div
              key={t.id}
              className="flex items-start gap-2 w-[320px] max-w-[90vw] bg-surface-container-high border border-outline-variant rounded-lg shadow-lg shadow-black/40 px-3 py-2.5 text-[12px] text-on-surface"
            >
              <span
                className={`mt-1 h-1.5 w-1.5 shrink-0 rounded-full ${DOT_COLOR[t.kind]}`}
              />
              <div className="min-w-0 flex-1">
                {t.scope && (
                  <div className="text-[10px] uppercase tracking-wide text-on-surface-variant">
                    {t.scope}
                  </div>
                )}
                <div className="leading-snug">
                  {t.message}
                  {t.link && (
                    <>
                      {" "}
                      <button
                        className="underline text-on-surface hover:text-primary"
                        onClick={() => openUrl(t.link!.url).catch((e) => console.error("Failed to open URL:", e))}
                      >
                        {t.link.label}
                      </button>
                    </>
                  )}
                </div>
                {t.action && (
                  <button
                    className="mt-1.5 border border-outline-variant rounded px-2 py-0.5 text-[11px] text-on-surface hover:bg-surface-container-highest transition-colors"
                    onClick={() => t.action!.onClick(t.id)}
                  >
                    {t.action.label}
                  </button>
                )}
              </div>
              <button
                className="shrink-0 text-on-surface-variant hover:text-on-surface leading-none"
                onClick={() => dismiss(t.id)}
                aria-label="Dismiss"
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
