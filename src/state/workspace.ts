import { create } from "zustand";
import type {
  PanelLayouts,
  PersistedWorkspace,
  SetupStatus,
  UserConfig,
  WorkspaceState as WorkspaceInfo,
} from "../ipc";
import { useChatOptions } from "./chatOptions";
import { useChat } from "./chat";

type Phase =
  | { kind: "loading" }
  | { kind: "setup"; status: SetupStatus }
  | { kind: "ready"; workspace: WorkspaceInfo };

interface WorkspaceStore {
  phase: Phase;
  panelLayout: PanelLayouts;
  recentWorkspaces: string[];
  setPhase: (phase: Phase) => void;
  setGroupLayout: (groupId: string, layout: Record<string, number>) => void;
  setPanelCollapsed: (panelId: "left" | "right", collapsed: boolean) => void;
  setRecentWorkspaces: (paths: string[]) => void;
  pushRecentWorkspace: (path: string) => void;
  hydrateFromPersisted: (state: PersistedWorkspace) => void;
  hydrateFromUserConfig: (config: UserConfig) => void;
  toPersisted: () => PersistedWorkspace;
}

export const useWorkspace = create<WorkspaceStore>((set, get) => ({
  phase: { kind: "loading" },
  panelLayout: {},
  recentWorkspaces: [],
  setPhase: (phase) => set({ phase }),
  setGroupLayout: (groupId, layout) =>
    set((s) => ({
      panelLayout: {
        ...s.panelLayout,
        groups: { ...(s.panelLayout.groups ?? {}), [groupId]: layout },
      },
    })),
  setPanelCollapsed: (panelId, collapsed) =>
    set((s) => ({
      panelLayout: {
        ...s.panelLayout,
        collapsed: { ...(s.panelLayout.collapsed ?? {}), [panelId]: collapsed },
      },
    })),
  setRecentWorkspaces: (paths) => set({ recentWorkspaces: paths }),
  pushRecentWorkspace: (path) =>
    set((s) => {
      const filtered = s.recentWorkspaces.filter((p) => p !== path);
      return { recentWorkspaces: [path, ...filtered].slice(0, 10) };
    }),
  hydrateFromPersisted: (state) => {
    const panelLayout = state.panel_layout ?? {};
    const bodyLayout = panelLayout.groups?.body;
    set({
      panelLayout: {
        ...panelLayout,
        collapsed: {
          left: panelLayout.collapsed?.left ?? bodyLayout?.left === 0,
          right: panelLayout.collapsed?.right ?? bodyLayout?.right === 0,
        },
      },
    });
    if (Array.isArray(state.tabs)) {
      useChat.getState().restorePersistedTabs(
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        state.tabs as any[],
        state.active_tab_id ?? undefined,
      );
    }
  },
  hydrateFromUserConfig: (config) => {
    set({
      recentWorkspaces: config.recent_workspaces ?? [],
    });
  },
  toPersisted: () => {
    const { panelLayout } = get();
    const { model, provider, effort } = useChatOptions.getState();
    const { tabs, activeTabId } = useChat.getState();
    const tabsToSave = tabs.map((tab) => ({
      ...tab,
      isStreaming: false,
      messages: tab.messages.map((message) =>
        message.role === "assistant" ? { ...message, isStreaming: false } : message
      ),
    }));
    return {
      version: 1,
      panel_layout: panelLayout,
      model,
      provider,
      last_opened: new Date().toISOString(),
      effort: effort ?? undefined,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      tabs: tabsToSave as any[],
      active_tab_id: activeTabId,
    };
  },
}));
