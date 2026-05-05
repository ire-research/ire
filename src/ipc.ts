import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { ChatMode, ResourceItem, TabCreatedPayload, TabStreamPayload, WikiFile } from "./types";

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

export const ipc = {
  setupStatus: (): Promise<SetupStatus> => invoke("setup_status"),
  openWorkspace: (path: string): Promise<WorkspaceState> =>
    invoke("open_workspace", { path }),
  initWorkspace: (path: string): Promise<WorkspaceState> =>
    invoke("init_workspace", { path }),
  closeWorkspace: (): Promise<void> => invoke("close_workspace"),
  readWikiFile: (path: string): Promise<WikiFile> =>
    invoke("read_wiki_file", { path }),
  saveNotes: (content: string): Promise<void> =>
    invoke("save_notes", { content }),
  saveIdeas: (content: string): Promise<void> =>
    invoke("save_ideas", { content }),
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
};

export function onWikiChanged(
  cb: (payload: { path: string }) => void
): Promise<() => void> {
  return listen<{ path: string }>("wiki-changed", (event) =>
    cb(event.payload)
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

export async function pickDirectory(title: string): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof result === "string" ? result : null;
}
