import { create } from "zustand";
import type {
  ExperimentRow,
  IdeaItem,
  ResourceItem,
  WorkspaceEvent,
} from "../types";

export type PaneKey = "focus" | "resources" | "experiments" | "notes" | "ideas";

/** Snapshot of workspaceData captured BEFORE `apply(event)` runs, so we can
 *  diff incoming items against the previous list to decide new vs changed. */
interface PriorSnapshot {
  resources: ResourceItem[];
  experiments: ExperimentRow[];
  ideas: IdeaItem[];
}

/** Per-row TTL for the .new / .change overlay. Must outlast the longest CSS
 *  animation in row-flash-* (item-rule, 1600ms). */
const ROW_TICK_TTL_MS = 1700;

interface PaneSignalsStore {
  /** Monotonic pulse counter per pane. Hooks fire when this increments. */
  pulse: Record<PaneKey, number>;
  /** Per-id tick for new-row overlays; increments restart the animation. */
  newTicks: Record<string, number>;
  /** Per-id tick for in-place change overlays. */
  changeTicks: Record<string, number>;
  handle: (event: WorkspaceEvent, prior: PriorSnapshot) => void;
}

export const usePaneSignals = create<PaneSignalsStore>((set) => {
  const bump = (pane: PaneKey) =>
    set((s) => ({ pulse: { ...s.pulse, [pane]: s.pulse[pane] + 1 } }));

  const markRow = (id: string, kind: "new" | "change") => {
    const key = kind === "new" ? "newTicks" : "changeTicks";
    set((s) => ({ [key]: { ...s[key], [id]: (s[key][id] ?? 0) + 1 } }) as Partial<PaneSignalsStore>);
    setTimeout(() => {
      set((s) => {
        const next = { ...s[key] };
        delete next[id];
        return { [key]: next } as Partial<PaneSignalsStore>;
      });
    }, ROW_TICK_TTL_MS);
  };

  return {
    pulse: { focus: 0, resources: 0, experiments: 0, notes: 0, ideas: 0 },
    newTicks: {},
    changeTicks: {},

    handle: (event, prior) => {
      // Only animate live changes — the hydrate burst on workspace open
      // would otherwise flash everything at once.
      if (event.source !== "mutation") return;

      switch (event.kind) {
        case "pulse-changed":
          bump("focus");
          return;
        case "notes-changed":
          bump("notes");
          return;
        case "ideas-changed": {
          const prevActive = prior.ideas.filter((i) => !i.trashed);
          const prevIds = new Set(prevActive.map((i) => i.id));
          const newOnes = event.ideas.filter((i) => !i.trashed && !prevIds.has(i.id));
          const nextActive = event.ideas.filter((i) => !i.trashed);
          if (newOnes.length === 0 && nextActive.length === prevActive.length) return;
          bump("ideas");
          newOnes.forEach((i) => markRow(i.id, "new"));
          return;
        }
        case "resource-changed": {
          const existed = prior.resources.some((r) => r.resource_id === event.resource.resource_id);
          bump("resources");
          markRow(event.resource.resource_id, existed ? "change" : "new");
          return;
        }
        case "resource-deleted":
          bump("resources");
          return;
        case "experiment-changed": {
          const existed = prior.experiments.some((e) => e.uuid === event.experiment.uuid);
          bump("experiments");
          markRow(event.experiment.uuid, existed ? "change" : "new");
          return;
        }
        case "experiment-deleted":
          bump("experiments");
          return;
      }
    },
  };
});

/** Sum of pulses for a rail — drives the edge-trace animation. */
export const selectLeftRailPulse = (s: PaneSignalsStore) =>
  s.pulse.focus + s.pulse.resources + s.pulse.experiments;
export const selectRightRailPulse = (s: PaneSignalsStore) =>
  s.pulse.notes + s.pulse.ideas;
