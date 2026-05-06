import { create } from "zustand";
import type { EffortLevel } from "../types";

export const MODELS = [
  { id: "claude-haiku-4-5-20251001", label: "Haiku 4.5" },
  { id: "claude-sonnet-4-6",         label: "Sonnet 4.6" },
  { id: "claude-opus-4-7",           label: "Opus 4.7"   },
];

export const EFFORT_LEVELS: { value: EffortLevel; label: string }[] = [
  { value: "low",    label: "Low"   },
  { value: "medium", label: "Med"   },
  { value: "high",   label: "High"  },
  { value: "xhigh",  label: "XHigh" },
  { value: "max",    label: "Max"   },
];

interface ChatOptionsState {
  model: string;
  effort: EffortLevel;
  setModel(m: string): void;
  setEffort(e: EffortLevel): void;
}

export const useChatOptions = create<ChatOptionsState>((set) => ({
  model: "claude-haiku-4-5-20251001",
  effort: "low",
  setModel: (model) => set({ model }),
  setEffort: (effort) => set({ effort }),
}));
