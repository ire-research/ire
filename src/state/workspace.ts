import { create } from "zustand";
import type { ChatMode } from "../types";
import type {
  PanelLayouts,
  PersistedWorkspace,
  SetupStatus,
  WorkspaceState as WorkspaceInfo,
} from "../ipc";

type Phase =
  | { kind: "loading" }
  | { kind: "setup"; status: SetupStatus }
  | { kind: "ready"; workspace: WorkspaceInfo };

type Theme = "dark" | "light";

interface WorkspaceStore {
  phase: Phase;
  mode: ChatMode;
  theme: Theme;
  panelLayout: PanelLayouts;
  setMode: (mode: ChatMode) => void;
  setPhase: (phase: Phase) => void;
  toggleTheme: () => void;
  setGroupLayout: (groupId: string, layout: Record<string, number>) => void;
  hydrateFromPersisted: (state: PersistedWorkspace) => void;
  toPersisted: () => PersistedWorkspace;
}

export const useWorkspace = create<WorkspaceStore>((set, get) => ({
  phase: { kind: "loading" },
  mode: "brainstorm",
  theme: "dark",
  panelLayout: {},
  setMode: (mode) => set({ mode }),
  setPhase: (phase) => set({ phase }),
  toggleTheme: () =>
    set((s) => ({ theme: s.theme === "dark" ? "light" : "dark" })),
  setGroupLayout: (groupId, layout) =>
    set((s) => ({
      panelLayout: {
        ...s.panelLayout,
        groups: { ...(s.panelLayout.groups ?? {}), [groupId]: layout },
      },
    })),
  hydrateFromPersisted: (state) => {
    set({
      theme: state.theme === "light" ? "light" : "dark",
      panelLayout: state.panel_layout ?? {},
    });
  },
  toPersisted: () => {
    const { theme, panelLayout } = get();
    return {
      version: 1,
      theme,
      panel_layout: panelLayout,
      last_opened: new Date().toISOString(),
    };
  },
}));
