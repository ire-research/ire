import { create } from "zustand";
import type { ChatMode } from "../types";
import type { SetupStatus, WorkspaceState as WorkspaceInfo } from "../ipc";

type Phase =
  | { kind: "loading" }
  | { kind: "setup"; status: SetupStatus }
  | { kind: "ready"; workspace: WorkspaceInfo };

type Theme = "dark" | "light";

interface WorkspaceStore {
  phase: Phase;
  mode: ChatMode;
  theme: Theme;
  setMode: (mode: ChatMode) => void;
  setPhase: (phase: Phase) => void;
  toggleTheme: () => void;
}

export const useWorkspace = create<WorkspaceStore>((set) => ({
  phase: { kind: "loading" },
  mode: "brainstorm",
  theme: "dark",
  setMode: (mode) => set({ mode }),
  setPhase: (phase) => set({ phase }),
  toggleTheme: () =>
    set((s) => ({ theme: s.theme === "dark" ? "light" : "dark" })),
}));
