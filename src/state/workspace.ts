import { create } from "zustand";
import type { ChatMode } from "../types";

interface WorkspaceState {
  workspaceName: string;
  mode: ChatMode;
  setMode: (mode: ChatMode) => void;
}

export const useWorkspace = create<WorkspaceState>((set) => ({
  workspaceName: "untitled-workspace",
  mode: "brainstorm",
  setMode: (mode) => set({ mode }),
}));
