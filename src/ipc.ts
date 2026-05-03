import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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
};

export async function pickDirectory(title: string): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title,
  });
  return typeof result === "string" ? result : null;
}
