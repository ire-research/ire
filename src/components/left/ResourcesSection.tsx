import { useState } from "react";
import { useWorkspaceData } from "../../state/workspaceData";
import { usePaneSignals } from "../../state/paneSignals";
import { useTransientClass } from "../../hooks/useTransientClass";
import { Icon } from "../Icon";
import { AddResourceModal } from "../AddResourceModal";

interface Props {
  onOpen: (label: string, wikiPath: string) => void;
}

export function ResourcesSection({ onOpen }: Props) {
  const resources = useWorkspaceData((s) => s.resources);
  const signalPulse = usePaneSignals((s) => s.pulse.resources);
  const newTicks = usePaneSignals((s) => s.newTicks);
  const changeTicks = usePaneSignals((s) => s.changeTicks);
  const paneRef = useTransientClass<HTMLDivElement>(signalPulse, "pane-signal-active", 1200);
  const [modalOpen, setModalOpen] = useState(false);
  const rail = resources
    .filter((r) => r.wiki_path)
    .map((r) => ({
      resourceId: r.resource_id,
      label: r.title ?? r.source_label,
      wikiPath: r.wiki_path!,
    }));

  return (
    <div ref={paneRef} data-side="left" className="pane-signal px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2 text-on-surface-variant text-[14px]">
        <span className="pane-signal-icon shrink-0">
          <Icon name="description" className="w-[16px] h-[16px]" />
        </span>
        <span className="flex-1">Resources</span>
        <span className="pane-signal-dot" aria-hidden />
        <button
          onClick={() => setModalOpen(true)}
          title="Add resource"
          className="w-5 h-5 flex items-center justify-center rounded text-[16px] text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface transition-colors"
        >
          +
        </button>
      </div>
      <div className="space-y-0.5">
        {rail.length > 0 ? (
          rail.map((resource) => {
            const newTick = newTicks[resource.resourceId] ?? 0;
            const changeTick = changeTicks[resource.resourceId] ?? 0;
            return (
              <button
                key={resource.resourceId}
                onClick={() => onOpen(resource.label, resource.wikiPath)}
                className={`relative overflow-hidden w-full text-left px-2 py-1.5 rounded text-[14px] text-on-surface hover:bg-surface-container-high transition-colors truncate${newTick > 0 ? " row-enter" : ""}`}
              >
                {resource.label}
                {newTick > 0 && <span key={`n-${newTick}`} className="row-flash row-flash-new" aria-hidden />}
                {changeTick > 0 && <span key={`c-${changeTick}`} className="row-flash row-flash-change" aria-hidden />}
              </button>
            );
          })
        ) : (
          <p className="text-[13px] text-on-surface-variant italic">no resources yet</p>
        )}
      </div>
      {modalOpen && <AddResourceModal onClose={() => setModalOpen(false)} />}
    </div>
  );
}
