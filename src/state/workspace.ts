import { create } from "zustand";
import type {
  PanelLayouts,
  PersistedWorkspace,
  SetupStatus,
  UserConfig,
  WorkspaceState as WorkspaceInfo,
} from "../ipc";
import { ipc } from "../ipc";
import type { ChatMessage, Tab } from "../types";
import { useChatOptions } from "./chatOptions";
import { useChat } from "./chat";
import { savePersisted } from "./persistedStore";

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
  hydrateFromPersisted: (state: PersistedWorkspace) => Promise<void>;
  hydrateFromUserConfig: (config: UserConfig) => void;
  toPersisted: () => PersistedWorkspace;
  persist: () => Promise<void>;
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
  hydrateFromPersisted: async (state) => {
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
      // workspace.json no longer stores messages — hydrate them from chat_sessions
      // (the durable store) by historySessionUuid.
      const tabs = state.tabs as Tab[];
      const hydrated = await Promise.all(
        tabs.map(async (t) => {
          if (t.kind === "chat" && t.historySessionUuid) {
            const json = await ipc.chatHistoryGet(t.historySessionUuid).catch(() => null);
            const messages: ChatMessage[] = json ? JSON.parse(json) : [];
            return { ...t, messages };
          }
          return { ...t, messages: t.messages ?? [] };
        }),
      );
      useChat.getState().restorePersistedTabs(hydrated, state.active_tab_id ?? undefined);
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
    // Messages live in chat_sessions (the durable store), not here — persist only
    // small UI metadata so there is a single source of truth for chat content.
    const tabsToSave = tabs.map(({ messages: _messages, ...rest }) => ({
      ...rest,
      isStreaming: false,
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
  persist: async () => {
    const { phase } = get();
    if (phase.kind !== "ready") return;
    await savePersisted(phase.workspace.path, get().toPersisted());
  },
}));
