import { useState } from "react";
import { useWorkspaceData } from "../../state/workspaceData";
import { Icon } from "../Icon";
import { AddResourceModal } from "../AddResourceModal";

interface Props {
  onOpen: (label: string, wikiPath: string) => void;
}

export function ResourcesSection({ onOpen }: Props) {
  const resources = useWorkspaceData((s) => s.resources);
  const [modalOpen, setModalOpen] = useState(false);
  const rail = resources
    .filter((r) => r.wiki_path)
    .map((r) => ({
      resourceId: r.resource_id,
      label: r.title ?? r.source_label,
      wikiPath: r.wiki_path!,
    }));

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="sticky top-0 z-10 flex items-center gap-2 py-1 mb-2 bg-surface-container-low text-on-surface-variant text-[14px]">
        <Icon name="description" className="w-[16px] h-[16px] shrink-0" />
        <span className="flex-1">Resources</span>
        <button
          onClick={() => setModalOpen(true)}
          title="Add resource"
          className="app-icon-button w-5 h-5 text-[16px]"
        >
          +
        </button>
      </div>
      <div className="space-y-0.5">
        {rail.length > 0 ? (
          rail.map((resource) => (
            <button
              key={resource.resourceId}
              onClick={() => onOpen(resource.label, resource.wikiPath)}
              className="w-full text-left px-2 py-1.5 rounded text-[14px] text-on-surface hover:bg-surface-container-high transition-colors truncate"
            >
              {resource.label}
            </button>
          ))
        ) : (
          <p className="text-[13px] text-on-surface-variant italic">no resources yet</p>
        )}
      </div>
      {modalOpen && <AddResourceModal onClose={() => setModalOpen(false)} />}
    </div>
  );
}
