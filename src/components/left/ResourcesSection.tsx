interface Props {
  resources: Array<{ label: string; wikiPath: string }>;
  onOpen: (label: string, wikiPath: string) => void;
}

export function ResourcesSection({ resources, onOpen }: Props) {
  if (resources.length === 0) {
    return <div />;
  }

  return (
    <div className="overflow-y-auto flex-1 py-1">
      <div className="flex items-center gap-2 px-4 py-2 text-on-surface-variant text-[14px]">
        <span className="material-symbols-outlined text-[16px] shrink-0">description</span>
        Resources
      </div>
      <div className="px-4 pb-1 space-y-0.5">
        {resources.map((resource) => (
          <button
            key={resource.wikiPath}
            onClick={() => onOpen(resource.label, resource.wikiPath)}
            className="w-full text-left px-2 py-1.5 rounded text-[14px] text-on-surface hover:bg-surface-container-high transition-colors truncate"
          >
            {resource.label}
          </button>
        ))}
      </div>
    </div>
  );
}
