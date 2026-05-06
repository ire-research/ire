import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ChatMode,
  ExperimentLogLinePayload,
  ExperimentRow,
  ExperimentStartingPayload,
  ExperimentStatusPayload,
  ResourceItem,
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
  theme?: "dark" | "light" | null;
  panel_layout?: PanelLayouts | null;
  last_opened?: string | null;
}

export interface PanelLayouts {
  /** Map of panel-group id → react-resizable-panels Layout (panel id → percentage). */
  groups?: Record<string, Record<string, number>>;
}

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
  saveIdeas: (content: string): Promise<void> =>
    invoke("save_ideas", { content }),
  updatePulseFocus: (focus: string): Promise<void> =>
    invoke("update_pulse_focus", { focus }),
  chatSend: (tabId: string, message: string, mode: ChatMode): Promise<void> =>
    invoke("chat_send", { tabId, message, mode }),
  chatCancel: (tabId: string): Promise<void> =>
    invoke("chat_cancel", { tabId }),
  chatResetSession: (tabId: string): Promise<void> =>
    invoke("chat_reset_session", { tabId }),
  submitResource: (url: string): Promise<string> =>
    invoke("submit_resource", { url }),
  discardResource: (resourceId: string): Promise<void> =>
    invoke("discard_resource", { resourceId }),
  indexResource: (resourceId: string): Promise<void> =>
    invoke("index_resource", { resourceId }),
  listResources: (): Promise<ResourceItem[]> =>
    invoke("list_resources"),
  getResourceConfirmPrompt: (): Promise<string> =>
    invoke("get_resource_confirm_prompt"),
  experimentList: (limit?: number): Promise<ExperimentRow[]> =>
    invoke("experiment_list", { limit }),
  experimentLogs: (uuid: string, kb?: number): Promise<{ stdout: string; stderr: string }> =>
    invoke("experiment_logs", { uuid, kb }),
  experimentCancel: (uuid: string): Promise<void> =>
    invoke("experiment_cancel", { uuid }),
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
