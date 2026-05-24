import { useState } from "react";
import { Icon } from "../Icon";
import { AddResourceModal } from "../AddResourceModal";

interface Props {
  resources: Array<{ resourceId: string; label: string; wikiPath: string }>;
  onOpen: (label: string, wikiPath: string) => void;
}

export function ResourcesSection({ resources, onOpen }: Props) {
  const [modalOpen, setModalOpen] = useState(false);

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2 text-on-surface-variant text-[14px]">
        <Icon name="description" className="w-[16px] h-[16px] shrink-0" />
        <span className="flex-1">Resources</span>
        <button
          onClick={() => setModalOpen(true)}
          title="Add resource"
          className="w-5 h-5 flex items-center justify-center rounded text-[16px] text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface transition-colors"
        >
          +
        </button>
      </div>
      <div className="space-y-0.5">
        {resources.length > 0 ? (
          resources.map((resource) => (
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
