import { Icon } from "../Icon";

interface Props {
  resources: Array<{ resourceId: string; label: string; wikiPath: string }>;
  onOpen: (label: string, wikiPath: string) => void;
}

export function ResourcesSection({ resources, onOpen }: Props) {
  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="flex items-center gap-2 py-1 mb-2 text-on-surface-variant text-[14px]">
        <Icon name="description" className="w-[16px] h-[16px] shrink-0" />
        Resources
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
    </div>
  );
}
