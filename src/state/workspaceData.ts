import { create } from "zustand";
import type {
  ExperimentRow,
  FocusContent,
  IdeaItem,
  ResourceItem,
  WorkspaceEvent,
} from "../types";

interface WorkspaceDataStore {
  focus: FocusContent;
  notes: string;
  ideas: IdeaItem[];
  resources: ResourceItem[];
  experiments: ExperimentRow[];
  reset: () => void;
  apply: (event: WorkspaceEvent) => void;
}

const emptyFocus: FocusContent = { research_question: "", this_week: "" };

// The slice receives all its data — both the initial workspace burst and live
// mutations — through `apply(event)`. The Rust side emits `workspace-event`
// with `source: "hydrate"` for the open-workspace burst and `source: "mutation"`
// for everything else; the reducer treats them identically.
export const useWorkspaceData = create<WorkspaceDataStore>((set) => ({
  focus: emptyFocus,
  notes: "",
  ideas: [],
  resources: [],
  experiments: [],

  reset: () =>
    set({
      focus: emptyFocus,
      notes: "",
      ideas: [],
      resources: [],
      experiments: [],
    }),

  apply: (event) =>
    set((s) => {
      switch (event.kind) {
        case "focus-changed":
          return {
            focus: {
              research_question: event.research_question,
              this_week: event.this_week,
            },
          };
        case "notes-changed":
          return { notes: event.content };
        case "ideas-changed":
          return { ideas: event.ideas };
        case "resource-changed": {
          const incoming = event.resource;
          const idx = s.resources.findIndex((r) => r.path === incoming.path);
          if (idx === -1) return { resources: [incoming, ...s.resources] };
          const next = s.resources.slice();
          next[idx] = incoming;
          return { resources: next };
        }
        case "resource-deleted":
          return {
            resources: s.resources.filter((r) => r.path !== event.path),
          };
        case "experiment-changed": {
          const incoming = event.experiment;
          const idx = s.experiments.findIndex((e) => e.uuid === incoming.uuid);
          if (idx === -1) return { experiments: [incoming, ...s.experiments] };
          const next = s.experiments.slice();
          next[idx] = incoming;
          return { experiments: next };
        }
        case "experiment-deleted":
          return {
            experiments: s.experiments.filter((e) => e.uuid !== event.uuid),
          };
      }
    }),
}));

export const selectRunningCount = (s: WorkspaceDataStore) =>
  s.experiments.filter((e) => e.status === "running" || e.status === "starting").length;
