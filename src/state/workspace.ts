import { create } from "zustand";
import type { ChatMode } from "../types";
import type { SetupStatus, WorkspaceState as WorkspaceInfo } from "../ipc";

type Phase =
  | { kind: "loading" }
  | { kind: "setup"; status: SetupStatus }
  | { kind: "ready"; workspace: WorkspaceInfo };

interface WorkspaceStore {
  phase: Phase;
  mode: ChatMode;
  setMode: (mode: ChatMode) => void;
  setPhase: (phase: Phase) => void;
}

export const useWorkspace = create<WorkspaceStore>((set) => ({
  phase: { kind: "loading" },
  mode: "brainstorm",
  setMode: (mode) => set({ mode }),
  setPhase: (phase) => set({ phase }),
}));
