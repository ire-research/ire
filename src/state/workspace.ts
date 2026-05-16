import { create } from "zustand";
import type { ChatMode } from "../types";
import type {
  PanelLayouts,
  PersistedWorkspace,
  SetupStatus,
  UserConfig,
  WorkspaceState as WorkspaceInfo,
} from "../ipc";

type Phase =
  | { kind: "loading" }
  | { kind: "setup"; status: SetupStatus }
  | { kind: "ready"; workspace: WorkspaceInfo };

interface WorkspaceStore {
  phase: Phase;
  mode: ChatMode;
  panelLayout: PanelLayouts;
  recentWorkspaces: string[];
  setMode: (mode: ChatMode) => void;
  setPhase: (phase: Phase) => void;
  setGroupLayout: (groupId: string, layout: Record<string, number>) => void;
  setRecentWorkspaces: (paths: string[]) => void;
  pushRecentWorkspace: (path: string) => void;
  hydrateFromPersisted: (state: PersistedWorkspace) => void;
  hydrateFromUserConfig: (config: UserConfig) => void;
  toPersisted: () => PersistedWorkspace;
}

export const useWorkspace = create<WorkspaceStore>((set, get) => ({
  phase: { kind: "loading" },
  mode: "brainstorm",
  panelLayout: {},
  recentWorkspaces: [],
  setMode: (mode) => set({ mode }),
  setPhase: (phase) => set({ phase }),
  setGroupLayout: (groupId, layout) =>
    set((s) => ({
      panelLayout: {
        ...s.panelLayout,
        groups: { ...(s.panelLayout.groups ?? {}), [groupId]: layout },
      },
    })),
  setRecentWorkspaces: (paths) => set({ recentWorkspaces: paths }),
  pushRecentWorkspace: (path) =>
    set((s) => {
      const filtered = s.recentWorkspaces.filter((p) => p !== path);
      return { recentWorkspaces: [path, ...filtered].slice(0, 10) };
    }),
  hydrateFromPersisted: (state) => {
    set({ panelLayout: state.panel_layout ?? {} });
  },
  hydrateFromUserConfig: (config) => {
    set({
      recentWorkspaces: config.recent_workspaces ?? [],
    });
  },
  toPersisted: () => {
    const { panelLayout } = get();
    return {
      version: 1,
      panel_layout: panelLayout,
      last_opened: new Date().toISOString(),
    };
  },
}));
