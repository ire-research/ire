import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { WikiFile } from "./types";

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
};

export function onWikiChanged(
  cb: (payload: { path: string }) => void
): Promise<() => void> {
  return listen<{ path: string }>("wiki-changed", (event) =>
    cb(event.payload)
  );
}

export async function pickDirectory(title: string): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof result === "string" ? result : null;
}
