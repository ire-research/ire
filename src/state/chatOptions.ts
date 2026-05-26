import { create } from "zustand";
import type { EffortLevel } from "../types";

export type Provider = "claude" | "codex";

export interface ModelEntry {
  id: string;
  label: string;
  provider: Provider;
}

export const MODELS: ModelEntry[] = [
  { id: "claude-opus-4-7",           label: "Opus 4.7",      provider: "claude" },
  { id: "claude-sonnet-4-6",         label: "Sonnet 4.6",    provider: "claude" },
  { id: "claude-haiku-4-5-20251001", label: "Haiku 4.5",     provider: "claude" },
  { id: "gpt-5.5",                   label: "GPT-5.5",       provider: "codex" },
  { id: "gpt-5.4",                   label: "GPT-5.4",       provider: "codex" },
  { id: "gpt-5.4-mini",              label: "GPT-5.4-Mini",  provider: "codex" },
  { id: "gpt-5.3-codex",             label: "GPT-5.3-Codex", provider: "codex" },
  { id: "gpt-5.2",                   label: "GPT-5.2",       provider: "codex" },
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

export const EFFORT_LEVELS = CLAUDE_EFFORT_LEVELS;

export const DEFAULT_CHAT_OPTIONS = {
  model: "claude-sonnet-4-6",
  provider: "claude" as Provider,
  effort: "low" as EffortLevel,
};

export function effortLevelsForProvider(provider: Provider) {
  return provider === "codex" ? CODEX_EFFORT_LEVELS : CLAUDE_EFFORT_LEVELS;
}

export function isValidChatOptions(model: string | null | undefined, provider: string | null | undefined, effort: string | null | undefined): model is string {
  if (provider !== "claude" && provider !== "codex") return false;
  if (!MODELS.some((entry) => entry.id === model && entry.provider === provider)) return false;
  return effortLevelsForProvider(provider).some((entry) => entry.value === effort);
}

export function defaultModelForProvider(provider: Provider): string {
  return MODELS.find((entry) => entry.provider === provider)?.id ?? DEFAULT_CHAT_OPTIONS.model;
}

export function optionsForAvailableProviders(
  model: string | null | undefined,
  provider: string | null | undefined,
  effort: string | null | undefined,
  availableProviders: Provider[],
) {
  if (isValidChatOptions(model, provider, effort)) {
    const validProvider = provider as Provider;
    if (!availableProviders.includes(validProvider)) {
      return optionsForAvailableProviders(null, null, null, availableProviders);
    }
    return {
      model,
      provider: validProvider,
      effort: effort as EffortLevel,
    };
  }

  const fallbackProvider = availableProviders.includes(DEFAULT_CHAT_OPTIONS.provider)
    ? DEFAULT_CHAT_OPTIONS.provider
    : availableProviders[0] ?? DEFAULT_CHAT_OPTIONS.provider;

  return {
    model: defaultModelForProvider(fallbackProvider),
    provider: fallbackProvider,
    effort: DEFAULT_CHAT_OPTIONS.effort,
  };
}

interface ChatOptionsState {
  model: string;
  provider: Provider;
  effort: EffortLevel;
  availableProviders: Provider[];
  setModel(model: string, provider: Provider): void;
  setEffort(e: EffortLevel): void;
  setOptions(options: { model: string; provider: Provider; effort: EffortLevel }): void;
  setAvailableProviders(providers: Provider[]): void;
}

export const useChatOptions = create<ChatOptionsState>((set) => ({
  ...DEFAULT_CHAT_OPTIONS,
  availableProviders: ["claude", "codex"],
  setModel: (model, provider) => set((state) => ({
    model,
    provider,
    effort: state.provider === provider ? state.effort : "low",
  })),
  setEffort: (effort) => set({ effort }),
  setOptions: (options) => set(options),
  setAvailableProviders: (providers) => set((state) => {
    const availableProviders = Array.from(new Set(providers));
    if (availableProviders.length === 0) return state;
    if (availableProviders.includes(state.provider)) {
      return { availableProviders };
    }
    const provider = availableProviders[0];
    return {
      availableProviders,
      model: defaultModelForProvider(provider),
      provider,
      effort: DEFAULT_CHAT_OPTIONS.effort,
    };
  }),
}));
