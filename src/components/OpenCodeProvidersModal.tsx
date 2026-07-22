import { useEffect, useMemo, useRef, useState } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faAnglesRight, faCheck, faChevronDown, faMagnifyingGlass, faXmark, iconClass } from "../icons";
import { ipc, type UserConfig } from "../ipc";
import type { ModelInfo } from "../types";

interface Props {
  onClose: () => void;
}

type PanelState =
  | { kind: "not_installed" }
  | { kind: "no_credentials" }
  | { kind: "error"; message: string }
  | { kind: "configured"; models: ModelInfo[] };

const OPENCODE_LOGIN_COMMAND = "opencode auth login";

export function OpenCodeProvidersModal({ onClose }: Props) {
  const [state, setState] = useState<PanelState | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [howExpanded, setHowExpanded] = useState(true);
  const [howManuallyToggled, setHowManuallyToggled] = useState(false);
  const [search, setSearch] = useState("");
  const [pinned, setPinned] = useState<string[]>([]);
  const [copied, setCopied] = useState(false);
  const hasLoadedOnMount = useRef(false);

  const load = async () => {
    setRefreshing(true);
    try {
      const [setup, models, config] = await Promise.all([
        ipc.setupStatus(),
        ipc.listAgentModels(),
        ipc.readUserConfig().catch((): UserConfig => ({})),
      ]);
      setPinned(config.pinned_opencode_models ?? []);

      const readiness = setup.providers.find((p) => p.provider === "opencode");
      if (!readiness || readiness.binary.kind === "missing") {
        setState({ kind: "not_installed" });
        return;
      }
      if (readiness.binary.kind === "logged_out") {
        setState({ kind: "no_credentials" });
        return;
      }
      const capabilities = models.find((m) => m.provider === "opencode");
      if (capabilities?.catalog.status === "error") {
        setState({ kind: "error", message: capabilities.catalog.message });
        return;
      }
      setState({ kind: "configured", models: capabilities?.catalog.models ?? [] });
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    // Guard against React.StrictMode's dev-only double-invoke of effects,
    // which would otherwise fire the opencode CLI round-trip twice on every
    // open (same pattern as useAutoUpdater.ts).
    if (hasLoadedOnMount.current) return;
    hasLoadedOnMount.current = true;
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (howManuallyToggled) return;
    setHowExpanded(state?.kind !== "configured");
  }, [state?.kind, howManuallyToggled]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const togglePin = async (modelId: string) => {
    const next = pinned.includes(modelId)
      ? pinned.filter((id) => id !== modelId)
      : [...pinned, modelId];
    setPinned(next);
    const config = await ipc.readUserConfig().catch(() => ({}));
    await ipc.saveUserConfig({ ...config, pinned_opencode_models: next }).catch(() => {});
  };

  const copyLoginCommand = async () => {
    try {
      await navigator.clipboard.writeText(OPENCODE_LOGIN_COMMAND);
    } catch {
      // clipboard access denied — nothing more we can do
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1200);
  };

  const groups = useMemo(() => {
    if (state?.kind !== "configured") return [];
    const q = search.trim().toLowerCase();
    const byProvider = new Map<string, ModelInfo[]>();
    for (const m of state.models) {
      if (q && !`${m.id} ${m.label}`.toLowerCase().includes(q)) continue;
      const providerKey = m.id.split("/")[0] || m.id;
      const list = byProvider.get(providerKey) ?? [];
      list.push(m);
      byProvider.set(providerKey, list);
    }
    return Array.from(byProvider.entries()).sort(([a], [b]) => a.localeCompare(b));
  }, [state, search]);

  const statusLine = (() => {
    if (!state) return { text: "Checking OpenCode…", tone: "muted" as const };
    switch (state.kind) {
      case "not_installed":
        return { text: "OpenCode not installed", tone: "error" as const };
      case "no_credentials":
        return { text: "OpenCode installed · no credentials detected", tone: "warn" as const };
      case "error":
        return { text: "Couldn't read OpenCode configuration", tone: "error" as const };
      case "configured":
        return {
          text: `OpenCode installed · credentials detected · ${state.models.length} model${state.models.length === 1 ? "" : "s"} available`,
          tone: "ok" as const,
        };
    }
  })();

  const dotClass =
    statusLine.tone === "ok"
      ? "bg-ok"
      : statusLine.tone === "warn"
        ? "bg-warn"
        : statusLine.tone === "error"
          ? "bg-error"
          : "bg-surface-container-highest border border-outline-variant";

  return (
    <div
      className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="w-[560px] max-h-[80vh] bg-surface-container border border-outline-variant rounded-lg flex flex-col shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="flex items-center gap-2 px-4 pt-3.5 pb-3 border-b border-outline-variant shrink-0">
          <FontAwesomeIcon icon={faAnglesRight} className={`${iconClass.lg} shrink-0 text-on-surface-variant`} />
          <span className="flex-1 text-[13px] font-medium text-on-surface">OpenCode Providers</span>
          <button onClick={onClose} className="app-icon-button w-6 h-6" aria-label="Close">
            <FontAwesomeIcon icon={faXmark} className={iconClass.md} />
          </button>
        </div>

        <div className="overflow-y-auto flex-1 min-h-0">
          {/* Intro */}
          <div className="px-4 pt-3.5 pb-3 border-b border-outline-variant">
            <p className="text-[13px] font-medium text-on-surface">Use providers configured in OpenCode</p>
            <p className="text-[11px] text-on-surface-variant mt-1 leading-relaxed">
              IRE uses OpenCode as a local provider gateway. Provider credentials remain on this
              device in OpenCode; IRE never receives or stores API keys.
            </p>
          </div>

          {/* Status + refresh */}
          <div className="px-4 pt-3 pb-3 flex items-center justify-between gap-3 border-b border-outline-variant">
            <div className="flex items-center gap-2 min-w-0">
              <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${dotClass}`} />
              <span className={`mono text-[11px] ${statusLine.tone === "error" ? "text-error" : "text-on-surface-variant"}`}>
                {statusLine.text}
              </span>
            </div>
            <button
              onClick={load}
              disabled={refreshing}
              className="flex items-center gap-1.5 border border-outline-variant text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high px-2.5 py-1.5 rounded text-[11px] transition-colors shrink-0 disabled:opacity-50"
            >
              {refreshing ? "Refreshing…" : "Refresh providers"}
            </button>
          </div>

          {/* How it works */}
          <div className="px-4 pt-3 pb-1">
            <button
              onClick={() => { setHowManuallyToggled(true); setHowExpanded((v) => !v); }}
              className="w-full flex items-center justify-between text-[11px] font-medium text-on-surface-variant hover:text-on-surface transition-colors"
            >
              <span>How it works</span>
              <FontAwesomeIcon
                icon={faChevronDown}
                className={`${iconClass.sm} transition-transform ${howExpanded ? "" : "-rotate-90"}`}
              />
            </button>
            {howExpanded && (
              <div className="mt-2.5 pb-3 flex flex-col gap-2.5">
                <div className="flex gap-2.5">
                  <span className="mono text-[10px] text-on-surface-variant/50 w-3.5 shrink-0 pt-0.5">1</span>
                  <p className="text-[12px] text-on-surface-variant">Install OpenCode</p>
                </div>
                <div className="flex gap-2.5">
                  <span className="mono text-[10px] text-on-surface-variant/50 w-3.5 shrink-0 pt-0.5">2</span>
                  <div className="flex-1 min-w-0">
                    <p className="text-[12px] text-on-surface-variant">Connect providers in OpenCode</p>
                    <div className="mt-1.5 flex items-center gap-2 bg-surface-container-low border border-outline-variant rounded px-2.5 py-1.5">
                      <span className="mono text-[11.5px] text-on-surface flex-1">{OPENCODE_LOGIN_COMMAND}</span>
                      <button
                        onClick={copyLoginCommand}
                        className="app-icon-button w-5 h-5 shrink-0"
                        aria-label="Copy command"
                        title="Copy command"
                      >
                        <FontAwesomeIcon icon={copied ? faCheck : faAnglesRight} className={iconClass.sm} />
                      </button>
                    </div>
                    <p className="text-[10.5px] text-on-surface-variant/70 mt-1.5 leading-relaxed">
                      This runs in your terminal. Provider sign-in and API-key setup happen there — not in IRE.
                    </p>
                    <p className="text-[10.5px] text-on-surface-variant/55 mt-1 leading-relaxed">
                      Covers OpenCode's supported provider sign-ins (Anthropic, OpenAI, Google, and
                      others). Custom or OpenAI-compatible endpoints are added directly in
                      OpenCode's own configuration — not through this command.
                    </p>
                  </div>
                </div>
                <div className="flex gap-2.5">
                  <span className="mono text-[10px] text-on-surface-variant/50 w-3.5 shrink-0 pt-0.5">3</span>
                  <div>
                    <p className="text-[12px] text-on-surface-variant">Refresh IRE and select a model</p>
                    <p className="text-[10.5px] text-on-surface-variant/55 mt-1">
                      After signing in, return here and refresh.
                    </p>
                  </div>
                </div>
              </div>
            )}
          </div>

          {state?.kind === "no_credentials" && (
            <div className="flex flex-col px-4 pb-3">
              <p className="text-[11px] text-on-surface-variant/70">
                OpenCode is installed, but no provider credentials were detected yet. Run{" "}
                <span className="mono text-on-surface-variant">{OPENCODE_LOGIN_COMMAND}</span> in
                your terminal, then refresh.
              </p>
            </div>
          )}

          {state?.kind === "error" && (
            <div className="flex flex-col px-4 pb-3 gap-1.5">
              <div className="bg-error-container/20 border border-error/30 rounded px-2.5 py-2">
                <p className="mono text-[11px] text-error">{state.message}</p>
              </div>
              <p className="text-[11px] text-on-surface-variant/70">Check the configuration, then try again.</p>
            </div>
          )}

          {state?.kind === "configured" && (
            <div className="flex flex-col">
              <div className="px-4 pt-1 pb-2.5">
                <div className="relative">
                  <FontAwesomeIcon
                    icon={faMagnifyingGlass}
                    className={`${iconClass.sm} absolute left-2.5 top-1/2 -translate-y-1/2 text-on-surface-variant/60`}
                  />
                  <input
                    type="text"
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    placeholder="Search providers or models…"
                    className="w-full bg-surface-container-low border border-outline-variant rounded text-[12px] text-on-surface pl-8 pr-2.5 py-1.5 focus:border-outline placeholder-on-surface-variant/45 outline-none"
                  />
                </div>
                <p className="text-[10.5px] text-on-surface-variant/55 mt-1.5">
                  Providers and models shown here are discovered automatically from your OpenCode configuration.
                </p>
              </div>

              <div className="max-h-[280px] overflow-y-auto px-1 pb-2">
                {groups.length === 0 ? (
                  <div className="px-4 py-8 text-center">
                    <p className="text-[12px] text-on-surface-variant">No models match your search.</p>
                    <p className="text-[11px] text-on-surface-variant/60 mt-1">Try a different provider or model name.</p>
                  </div>
                ) : (
                  groups.map(([providerKey, models]) => (
                    <div key={providerKey} className="py-2 border-t border-outline-variant/40 first:border-t-0">
                      <div className="px-3 pb-1 text-[10px] font-medium uppercase tracking-normal text-on-surface-variant/60">
                        {providerKey}
                      </div>
                      {models.map((m) => {
                        const isPinned = pinned.includes(m.id);
                        return (
                          <div
                            key={m.id}
                            className="w-full flex items-center gap-2.5 px-3 py-1.5 rounded hover:bg-surface-container-highest transition-colors group"
                          >
                            <button
                              onClick={() => togglePin(m.id)}
                              className={`shrink-0 w-4 h-4 flex items-center justify-center text-[12px] leading-none transition-colors ${
                                isPinned ? "text-primary" : "text-on-surface-variant/30 hover:text-on-surface-variant"
                              }`}
                              aria-pressed={isPinned}
                              aria-label={`${isPinned ? "Unpin" : "Pin"} ${m.label}`}
                              title={isPinned ? "Unpin" : "Pin"}
                            >
                              {isPinned ? "★" : "☆"}
                            </button>
                            <div className="flex-1 min-w-0">
                              <p className="text-[12px] text-on-surface truncate">{m.label}</p>
                              <p className="mono text-[10.5px] text-on-surface-variant/60 truncate">{m.id}</p>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  ))
                )}
              </div>

              <p className="text-[10.5px] text-on-surface-variant/50 px-4 pt-1 pb-3 leading-relaxed">
                Favorites (★) are saved as an IRE preference, global to your account across all
                workspaces. Provider credentials and model definitions stay owned by OpenCode.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
