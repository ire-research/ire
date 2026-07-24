import { create } from "zustand";
import type { EffortLevel, Provider } from "../types";

export type { Provider };

export interface ModelEntry {
  id: string;
  label: string;
  provider: Provider;
}

export const MODELS: ModelEntry[] = [
  { id: "claude-sonnet-5",           label: "Sonnet 5",      provider: "claude" },
  { id: "claude-opus-4-8",           label: "Opus 4.8",      provider: "claude" },
  { id: "claude-fable-5",            label: "Fable 5",       provider: "claude" },
  { id: "claude-haiku-4-5-20251001", label: "Haiku 4.5",     provider: "claude" },
  { id: "gpt-5.5",                   label: "GPT-5.5",       provider: "codex" },
  { id: "gpt-5.4",                   label: "GPT-5.4",       provider: "codex" },
  { id: "gpt-5.4-mini",              label: "GPT-5.4-Mini",  provider: "codex" },
  { id: "gpt-5.3-codex",             label: "GPT-5.3-Codex", provider: "codex" },
];

export const CLAUDE_EFFORT_LEVELS: { value: EffortLevel; label: string }[] = [
  { value: "low",    label: "Low"   },
  { value: "medium", label: "Med"   },
  { value: "high",   label: "High"  },
  { value: "xhigh",  label: "XHigh" },
  { value: "max",    label: "Max"   },
];

export const CODEX_EFFORT_LEVELS: { value: EffortLevel; label: string }[] = [
  { value: "low",    label: "Low"   },
  { value: "medium", label: "Med"   },
  { value: "high",   label: "High"  },
  { value: "xhigh",  label: "XHigh" },
];

export const DEFAULT_CHAT_OPTIONS = {
  model: "claude-sonnet-5",
  provider: "claude" as Provider,
  effort: "low" as EffortLevel | null,
};

export function effortLevelsForModel(provider: Provider, model: string) {
  // OpenCode models each advertise their own variant vocabulary dynamically
  // (fetched via list_agent_models, not a static per-provider table like
  // Claude/Codex have) — v1 doesn't expose per-model variant selection in
  // the composer, so there's no effort picker for it here.
  if (provider === "opencode") return [];
  if (provider === "codex") return CODEX_EFFORT_LEVELS;
  if (model.includes("haiku")) return [];
  if (model.includes("opus")) return CLAUDE_EFFORT_LEVELS;
  return CLAUDE_EFFORT_LEVELS.filter((entry) => entry.value !== "xhigh");
}

export function defaultEffortForModel(provider: Provider, model: string): EffortLevel | null {
  return effortLevelsForModel(provider, model)[0]?.value ?? null;
}

function normalizedEffortForModel(provider: Provider, model: string, effort: string | null | undefined): EffortLevel | null {
  const levels = effortLevelsForModel(provider, model);
  if (levels.length === 0) return null;
  return levels.some((entry) => entry.value === effort)
    ? effort as EffortLevel
    : levels[0].value;
}

export function isValidChatOptions(model: string | null | undefined, provider: string | null | undefined, effort: string | null | undefined): model is string {
  // OpenCode's catalog is dynamic (fetched via list_agent_models), so it
  // can't be checked against the static MODELS list the way Claude/Codex
  // are — a non-empty model id and no effort selected is as far as this can
  // validate without a network/process round-trip.
  if (provider === "opencode") {
    return !!model && (effort === null || effort === undefined || effort === "");
  }
  if (provider !== "claude" && provider !== "codex") return false;
  if (!model) return false;
  if (!MODELS.some((entry) => entry.id === model && entry.provider === provider)) return false;
  const levels = effortLevelsForModel(provider, model);
  return levels.length === 0
    ? effort === null || effort === undefined || effort === ""
    : levels.some((entry) => entry.value === effort);
}

export function defaultModelForProvider(provider: Provider): string {
  // No static default exists for OpenCode's dynamic catalog — callers that
  // fall back to this (e.g. OpenCode becoming the only available provider)
  // get an empty model id and must prompt the user to pick one explicitly.
  if (provider === "opencode") return "";
  return MODELS.find((entry) => entry.provider === provider)?.id ?? DEFAULT_CHAT_OPTIONS.model;
}

/** Smallest/cheapest model per provider — used for background chat-title generation. */
export function lightweightModelForProvider(provider: Provider): string {
  if (provider === "opencode") return "";
  return provider === "codex" ? "gpt-5.4-mini" : "claude-haiku-4-5-20251001";
}

export function optionsForAvailableProviders(
  model: string | null | undefined,
  provider: string | null | undefined,
  effort: string | null | undefined,
  availableProviders: Provider[],
) {
  if (provider === "opencode" && model && availableProviders.includes("opencode")) {
    return { model, provider: "opencode" as Provider, effort: null };
  }

  if (provider === "claude" || provider === "codex") {
    const validProvider = provider as Provider;
    if (
      model &&
      availableProviders.includes(validProvider) &&
      MODELS.some((entry) => entry.id === model && entry.provider === validProvider)
    ) {
      return {
        model,
        provider: validProvider,
        effort: normalizedEffortForModel(validProvider, model, effort),
      };
    }
  }

  const fallbackProvider = availableProviders.includes(DEFAULT_CHAT_OPTIONS.provider)
    ? DEFAULT_CHAT_OPTIONS.provider
    : availableProviders[0] ?? DEFAULT_CHAT_OPTIONS.provider;

  const fallbackModel = defaultModelForProvider(fallbackProvider);
  return {
    model: fallbackModel,
    provider: fallbackProvider,
    effort: defaultEffortForModel(fallbackProvider, fallbackModel),
  };
}

interface ChatOptionsState {
  model: string;
  provider: Provider;
  effort: EffortLevel | null;
  availableProviders: Provider[];
  setModel(model: string, provider: Provider): void;
  setEffort(e: EffortLevel): void;
  setOptions(options: { model: string; provider: Provider; effort: EffortLevel | null }): void;
  setAvailableProviders(providers: Provider[]): void;
}

export const useChatOptions = create<ChatOptionsState>((set) => ({
  ...DEFAULT_CHAT_OPTIONS,
  availableProviders: ["claude", "codex"],
  setModel: (model, provider) => set((state) => ({
    model,
    provider,
    effort: normalizedEffortForModel(
      provider,
      model,
      state.provider === provider ? state.effort : null,
    ),
  })),
  setEffort: (effort) => set({ effort }),
  setOptions: (options) => set(options),
  setAvailableProviders: (providers) => set((state) => {
    const availableProviders = Array.from(new Set(providers));
    if (availableProviders.length === 0 || availableProviders.includes(state.provider)) {
      return { availableProviders };
    }
    const provider = availableProviders[0];
    const model = defaultModelForProvider(provider);
    return {
      availableProviders,
      model,
      provider,
      effort: defaultEffortForModel(provider, model),
    };
  }),
}));
