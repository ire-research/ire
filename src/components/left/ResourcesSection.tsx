import { useState, useRef, useEffect } from "react";
import { useWorkspaceData } from "../../state/workspaceData";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faFileLines, faTrash, iconClass } from "../../icons";
import { AddResourceModal } from "../AddResourceModal";
import { ipc } from "../../ipc";
import { toastError } from "../../state/toasts";

interface Props {
  onOpen: (label: string, wikiPath: string) => void;
}

export function ResourcesSection({ onOpen }: Props) {
  const resources = useWorkspaceData((s) => s.resources);
  const [modalOpen, setModalOpen] = useState(false);
  const [discardingId, setDiscardingId] = useState<string | null>(null);
  const [tooltip, setTooltip] = useState<{ label: string; x: number; y: number } | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const spanRefs = useRef<Map<string, HTMLSpanElement | null>>(new Map());

  useEffect(() => () => { if (timerRef.current) clearTimeout(timerRef.current); }, []);

  const handleMouseEnter = (resourceId: string, label: string) => {
    const span = spanRefs.current.get(resourceId);
    if (!span || span.scrollWidth <= span.clientWidth) return;
    const rect = span.getBoundingClientRect();
    timerRef.current = setTimeout(() => {
      setTooltip({ label, x: rect.left, y: rect.bottom + 4 });
    }, 250);
  };

  const handleMouseLeave = () => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setTooltip(null);
  };

  const handleDiscard = async (e: React.MouseEvent, resourceId: string) => {
    e.stopPropagation();
    setDiscardingId(resourceId);
    try {
      await ipc.discardResource(resourceId);
    } catch (err) {
      toastError("discard resource", err);
    } finally {
      setDiscardingId(null);
    }
  };

  const rail = resources
    .filter((r) => r.wiki_path)
    .map((r) => ({
      resourceId: r.resource_id,
      label: r.title ?? r.source_label,
      wikiPath: r.wiki_path!,
    }));

  return (
    <div className="px-4 pt-4 pb-3 overflow-y-auto flex-1">
      <div className="sticky top-0 z-10 flex items-center gap-2 py-1 mb-2 bg-surface-container-low text-on-surface-variant font-mono text-[14px]">
        <FontAwesomeIcon icon={faFileLines} className={`${iconClass.lg} shrink-0`} />
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
            <div
              key={resource.resourceId}
              className="group w-full flex items-center px-2 py-1.5 rounded hover:bg-surface-container-high transition-colors"
              onMouseLeave={handleMouseLeave}
            >
              <button
                className="flex-1 min-w-0 text-left"
                onMouseEnter={() => handleMouseEnter(resource.resourceId, resource.label)}
                onClick={() => onOpen(resource.label, resource.wikiPath)}
              >
                <span
                  ref={(el) => { spanRefs.current.set(resource.resourceId, el); }}
                  className="text-[14px] text-on-surface truncate block"
                >
                  {resource.label}
                </span>
              </button>
              <button
                className="app-danger-icon-button opacity-0 group-hover:opacity-100 ml-1 p-0.5 shrink-0"
                title="Discard resource"
                disabled={discardingId === resource.resourceId}
                onClick={(e) => handleDiscard(e, resource.resourceId)}
              >
                <FontAwesomeIcon icon={faTrash} className={iconClass.md} />
              </button>
            </div>
          ))
        ) : (
          <p className="text-[13px] text-on-surface-variant italic">no resources yet</p>
        )}
      </div>
      {tooltip && (
        <div
          className="fixed z-50 px-2 py-1 bg-surface-container-high border border-outline/30 text-on-surface text-[13px] rounded shadow-md max-w-[240px] whitespace-normal pointer-events-none"
          style={{ left: tooltip.x, top: tooltip.y }}
        >
          {tooltip.label}
        </div>
      )}
      {modalOpen && <AddResourceModal onClose={() => setModalOpen(false)} />}
    </div>
  );
}
