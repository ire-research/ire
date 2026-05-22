import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ChatOptions,
  ExperimentLogLinePayload,
  ExperimentRow,
  ExperimentStartingPayload,
  ExperimentStatusPayload,
  IdeaItem,
  PulseContent,
  ResourceItem,
  SystemStatus,
  TabCreatedPayload,
  TabStreamPayload,
  WikiFile,
} from "./types";

export type BinaryStatus =
  | { kind: "found"; path: string; version: string | null }
  | { kind: "missing" };

export interface SetupStatus {
  binary: BinaryStatus;
}

export interface WorkspaceState {
  path: string;
  name: string;
}

export interface PersistedWorkspace {
  version: number;
  panel_layout?: PanelLayouts | null;
  last_opened?: string | null;
  effort?: string | null;
}

export interface UserConfig {
  theme?: "dark" | "light" | null;
  recent_workspaces?: string[];
}

export interface PanelLayouts {
  /** Map of panel-group id → react-resizable-panels Layout (panel id → percentage). */
  groups?: Record<string, Record<string, number>>;
}

export type ResourceSourceInput =
  | { kind: "url"; url: string }
  | { kind: "local_file"; path: string };

export const ipc = {
  setupStatus: (): Promise<SetupStatus> => invoke("setup_status"),
  openWorkspace: (path: string): Promise<WorkspaceState> =>
    invoke("open_workspace", { path }),
  initWorkspace: (path: string): Promise<WorkspaceState> =>
    invoke("init_workspace", { path }),
  closeWorkspace: (): Promise<void> => invoke("close_workspace"),
  readWorkspaceState: (): Promise<PersistedWorkspace> =>
    invoke("read_workspace_state"),
  saveWorkspaceState: (state: PersistedWorkspace): Promise<void> =>
    invoke("save_workspace_state", { state }),
  readWikiFile: (path: string): Promise<WikiFile> =>
    invoke("read_wiki_file", { path }),
  saveNotes: (content: string): Promise<void> =>
    invoke("save_notes", { content }),
  readPulse: (): Promise<PulseContent> => invoke("read_pulse"),
  savePulseField: (field: "research_question" | "this_week", content: string): Promise<void> =>
    invoke("save_pulse_field", { field, content }),
  readIdeas: (): Promise<IdeaItem[]> => invoke("read_ideas"),
  saveIdeasJson: (ideas: IdeaItem[]): Promise<void> => invoke("save_ideas_json", { ideas }),
  getSystemStatus: (): Promise<SystemStatus> => invoke("get_system_status"),
  chatSend: (tabId: string, message: string, options: ChatOptions): Promise<void> =>
    invoke("chat_send", { tabId, message, options }),
  chatCancel: (tabId: string): Promise<void> =>
    invoke("chat_cancel", { tabId }),
  chatResetSession: (tabId: string): Promise<void> =>
    invoke("chat_reset_session", { tabId }),
  submitResource: (url: string): Promise<string> =>
    invoke("submit_resource", { url }),
  submitLocalResource: (path: string): Promise<string> =>
    invoke("submit_local_resource", { path }),
  submitResources: (sources: ResourceSourceInput[]): Promise<string> =>
    invoke("submit_resources", { sources }),
  discardResource: (resourceId: string): Promise<void> =>
    invoke("discard_resource", { resourceId }),
  indexResource: (resourceId: string): Promise<void> =>
    invoke("index_resource", { resourceId }),
  listResources: (): Promise<ResourceItem[]> =>
    invoke("list_resources"),
  getResourceConfirmPrompt: (): Promise<string> =>
    invoke("get_resource_confirm_prompt"),
  saveWikiFile: (path: string, content: string): Promise<void> =>
    invoke("save_wiki_file", { path, content }),
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
};

export function onWikiChanged(
  cb: (payload: { path: string }) => void
): Promise<() => void> {
  return listen<{ path: string }>("wiki-changed", (event) =>
    cb(event.payload)
  );
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
