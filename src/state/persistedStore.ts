import { load } from "@tauri-apps/plugin-store";
import type { PersistedWorkspace } from "../ipc";

// UI/session state (model, provider, effort, panel layout, tabs) lives in the
// Tauri plugin-store's app-data dir — not in the git-tracked workspace `.ire/`.
// One shared store file, keyed by workspace path, so reopening any recent
// workspace restores its own state.
const STORE = "workspace-state.json";

export async function loadPersisted(path: string): Promise<PersistedWorkspace | null> {
  const store = await load(STORE);
  return (await store.get<PersistedWorkspace>(path)) ?? null;
}

export async function savePersisted(path: string, state: PersistedWorkspace): Promise<void> {
  const store = await load(STORE);
  await store.set(path, state);
  await store.save();
}
