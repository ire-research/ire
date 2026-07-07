import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AskAnswer,
  BinaryStatus,
  ChatOptions,
  ExperimentLogLinePayload,
  ExperimentRow,
  ExperimentStartingPayload,
  ExperimentStatusPayload,
  IdeaItem,
  ResourcePendingPayload,
  SystemInfo,
  SystemMetrics,
  TabCreatedPayload,
  TabStreamPayload,
  WikiFile,
  WorkspaceEvent,
} from "./types";

export type { BinaryStatus };

export interface SetupStatus {
  claude_binary: BinaryStatus;
  codex_binary: BinaryStatus;
}

export interface WorkspaceState {
  path: string;
  name: string;
}

export interface PersistedWorkspace {
  version: number;
  panel_layout?: PanelLayouts | null;
  model?: string | null;
  provider?: "claude" | "codex" | null;
  last_opened?: string | null;
  effort?: string | null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  tabs?: any[] | null;
  active_tab_id?: string | null;
}

export interface UserConfig {
  theme?: "dark" | "light" | null;
  recent_workspaces?: string[];
  analytics_enabled?: boolean | null;
}

export interface PanelLayouts {
  /** Map of panel-group id → react-resizable-panels Layout (panel id → percentage). */
  groups?: Record<string, Record<string, number>>;
  /** Independent sidebar collapsed state, persisted alongside group sizes. */
  collapsed?: {
    left?: boolean;
    right?: boolean;
  };
}

export type ResourceSourceInput =
  | { kind: "url"; url: string }
  | { kind: "local_file"; path: string };

export const ipc = {
  setupStatus: (): Promise<SetupStatus> => invoke("setup_status"),
  openWorkspace: (path: string): Promise<WorkspaceState> =>
    invoke("open_workspace", { path }),
  closeWorkspace: (): Promise<void> => invoke("close_workspace"),
  readResource: (path: string): Promise<WikiFile> =>
    invoke("read_resource", { path }),
  saveNotes: (content: string): Promise<void> =>
    invoke("save_notes", { content }),
  saveFocusField: (field: "research_question" | "this_week", content: string): Promise<void> =>
    invoke("save_focus_field", { field, content }),
  saveIdeas: (ideas: IdeaItem[]): Promise<void> => invoke("save_ideas", { ideas }),
  getSystemInfo: (): Promise<SystemInfo> => invoke("get_system_info"),
  getSystemMetrics: (): Promise<SystemMetrics> => invoke("get_system_metrics"),
  chatSend: (
    tabId: string,
    message: string,
    options: ChatOptions,
    sessionUuid: string,
    tabLabel: string,
    startedAt: string,
  ): Promise<void> =>
    invoke("chat_send", { tabId, message, options, sessionUuid, tabLabel, startedAt }),
  chatCancel: (tabId: string): Promise<void> =>
    invoke("chat_cancel", { tabId }),
  chatResetSession: (tabId: string): Promise<void> =>
    invoke("chat_reset_session", { tabId }),
  submitAskAnswer: (tabId: string, answers: AskAnswer[]): Promise<void> =>
    invoke("submit_ask_answer", { tabId, answers }),
  submitResource: (url: string, options: ChatOptions): Promise<string> =>
    invoke("submit_resource", { url, options }),
  submitLocalResource: (path: string, options: ChatOptions): Promise<string> =>
    invoke("submit_local_resource", { path, options }),
  submitResources: (sources: ResourceSourceInput[], options: ChatOptions): Promise<string> =>
    invoke("submit_resources", { sources, options }),
  discardResource: (resourceId: string): Promise<void> =>
    invoke("discard_resource", { resourceId }),
  readResourceDraft: (resourceId: string): Promise<string> =>
    invoke("read_resource_draft", { resourceId }),
  saveResourceDraft: (resourceId: string, content: string): Promise<void> =>
    invoke("save_resource_draft", { resourceId, content }),
  confirmResource: (resourceId: string): Promise<void> =>
    invoke("confirm_resource", { resourceId }),
  saveResource: (path: string, content: string): Promise<void> =>
    invoke("save_resource", { path, content }),
  experimentList: (limit?: number): Promise<ExperimentRow[]> =>
    invoke("experiment_list", { limit }),
  experimentLogs: (uuid: string, kb?: number): Promise<{ stdout: string; stderr: string }> =>
    invoke("experiment_logs", { uuid, kb }),
  experimentCancel: (uuid: string): Promise<void> =>
    invoke("experiment_cancel", { uuid }),
  experimentDelete: (uuid: string): Promise<void> =>
    invoke("experiment_delete", { uuid }),
  experimentRename: (uuid: string, name: string): Promise<void> =>
    invoke("experiment_rename", { uuid, name }),
  readUserConfig: (): Promise<UserConfig> => invoke("read_user_config"),
  saveUserConfig: (config: UserConfig): Promise<void> =>
    invoke("save_user_config", { config }),
  openInVscode: (path: string): Promise<void> =>
    invoke("open_in_vscode", { path }),
  generateChatTitle: (message: string, model: string, provider: string): Promise<string> =>
    invoke("generate_chat_title", { message, model, provider }),
  chatHistorySave: (
    tabLabel: string,
    provider: string,
    model: string,
    startedAt: string,
    messagesJson: string,
    sessionUuid?: string,
  ): Promise<void> =>
    invoke("chat_history_save", { sessionUuid, tabLabel, provider, model, startedAt, messagesJson }),
  chatHistoryList: (limit?: number): Promise<import("./types").ChatSessionSummary[]> =>
    invoke("chat_history_list", { limit }),
  chatHistoryGet: (sessionUuid: string): Promise<string | null> =>
    invoke("chat_history_get", { sessionUuid }),
  chatHistoryDelete: (sessionUuid: string): Promise<void> =>
    invoke("chat_history_delete", { sessionUuid }),
};

export function onWorkspaceEvent(
  cb: (event: WorkspaceEvent) => void
): Promise<() => void> {
  return listen<WorkspaceEvent>("workspace-event", (event) => cb(event.payload));
}

export function onBackendError(
  cb: (payload: { scope: string; message: string }) => void
): Promise<() => void> {
  return listen<{ scope: string; message: string }>("error", (event) =>
    cb(event.payload),
  );
}

export function onChatStream(
  cb: (payload: TabStreamPayload) => void
): Promise<() => void> {
  return listen<TabStreamPayload>("chat-stream", (event) => cb(event.payload));
}

export function onTabCreated(
  cb: (payload: TabCreatedPayload) => void
): Promise<() => void> {
  return listen<TabCreatedPayload>("tab-created", (event) => cb(event.payload));
}

export function onResourcePending(
  cb: (payload: ResourcePendingPayload) => void
): Promise<() => void> {
  return listen<ResourcePendingPayload>("resource-pending", (e) => cb(e.payload));
}

export function onExperimentStatus(
  cb: (payload: ExperimentStatusPayload) => void
): Promise<() => void> {
  return listen<ExperimentStatusPayload>("experiment-status", (e) => cb(e.payload));
}

export function onExperimentLogLine(
  cb: (payload: ExperimentLogLinePayload) => void
): Promise<() => void> {
  return listen<ExperimentLogLinePayload>("experiment-log-line", (e) => cb(e.payload));
}

export function onExperimentStarting(
  cb: (payload: ExperimentStartingPayload) => void
): Promise<() => void> {
  return listen<ExperimentStartingPayload>("experiment-starting", (e) => cb(e.payload));
}

export async function pickDirectory(title: string): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof result === "string" ? result : null;
}

export async function pickResourceFile(): Promise<string | null> {
  const result = await open({
    directory: false,
    multiple: false,
    title: "Choose resource file",
    filters: [
      {
        name: "Supported resources",
        extensions: ["txt", "md", "pdf", "docx"],
      },
    ],
  });
  return typeof result === "string" ? result : null;
}

export async function pickResourceFiles(): Promise<string[]> {
  const result = await open({
    directory: false,
    multiple: true,
    title: "Choose resource files",
    filters: [
      {
        name: "Supported resources",
        extensions: ["txt", "md", "pdf", "docx"],
      },
    ],
  });
  if (Array.isArray(result)) return result;
  return typeof result === "string" ? [result] : [];
}
