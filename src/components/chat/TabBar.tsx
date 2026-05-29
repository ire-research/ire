import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faMessage, faSpinner, faFileLines, faFlask, faPencil, faPlus, iconClass } from "../../icons";
import type { IconDefinition } from "@fortawesome/fontawesome-svg-core";
import type { Tab } from "../../types";

interface Props {
  tabs: Tab[];
  activeTabId: string;
  onSelect: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onNew: () => void;
  onRename: (tabId: string, newName: string) => void;
  rightSlot?: ReactNode;
}

function tabIcon(tab: Tab): { icon: IconDefinition; spin: boolean } {
  if (tab.kind === "chat") return { icon: faMessage, spin: false };
  if (tab.kind === "resource") {
    if (tab.resourceStatus === "summarizing") return { icon: faSpinner, spin: true };
    return { icon: faFileLines, spin: false };
  }
  if (tab.kind === "preview") return { icon: faFileLines, spin: false };
  if (tab.kind === "experiment") return { icon: faFlask, spin: false };
  return { icon: faMessage, spin: false };
}

/** Renders a tab label, typewriter-animating when the label changes while mounted
 *  (e.g. a background-generated chat title typing over "Untitled"). Does not animate on
 *  first mount, nor on manual rename — the inline rename input unmounts this span,
 *  so it remounts fresh on commit with the prev ref re-seeded. */
function TabLabel({ label }: { label: string }) {
  const [display, setDisplay] = useState(label);
  const prev = useRef(label);

  useEffect(() => {
    if (prev.current === label) return;
    prev.current = label;
    let i = 0;
    setDisplay("");
    const id = setInterval(() => {
      i++;
      setDisplay(label.slice(0, i));
      if (i >= label.length) clearInterval(id);
    }, 40);
    return () => clearInterval(id);
  }, [label]);

  return <span className="font-mono flex-1 truncate min-w-0">{display}</span>;
}

export function TabBar({ tabs, activeTabId, onSelect, onClose, onNew, onRename, rightSlot }: Props) {
  const [renamingTabId, setRenamingTabId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const startRename = (e: React.MouseEvent, tab: Tab) => {
    e.stopPropagation();
    setRenamingTabId(tab.id);
    setRenameValue(tab.label);
  };

  const commitRename = (tabId: string) => {
    const trimmed = renameValue.trim();
    if (trimmed) onRename(tabId, trimmed);
    setRenamingTabId(null);
  };

  const handleKeyDown = (e: React.KeyboardEvent, tabId: string) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commitRename(tabId);
    } else if (e.key === "Escape") {
      setRenamingTabId(null);
    }
  };

  return (
    <div className="relative flex h-8 border-b border-outline-variant bg-surface-container-low shrink-0 px-2">
      <div className="flex min-w-0 flex-1 overflow-x-auto no-scrollbar">
        {tabs.map((tab) => {
          const isActive = tab.id === activeTabId;
          const isRenaming = renamingTabId === tab.id;
          const { icon, spin } = tabIcon(tab);
          return (
            <div
              key={tab.id}
              className={
                isActive
                  ? "group relative flex items-center px-3 border-r border-outline-variant bg-surface-container-highest text-on-surface text-xs w-32 shrink-0 cursor-pointer border-t border-t-primary overflow-hidden"
                  : "group relative flex items-center px-3 border-r border-outline-variant text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors text-xs w-32 shrink-0 cursor-pointer overflow-hidden"
              }
              onClick={() => !isRenaming && onSelect(tab.id)}
            >
              <FontAwesomeIcon icon={icon} className={`${iconClass.sm} mr-1.5${spin ? " animate-spin" : ""}`} />
              {isRenaming ? (
                <input
                  ref={inputRef}
                  autoFocus
                  className="text-xs text-on-surface bg-transparent border-b border-outline outline-none w-20 min-w-0"
                  value={renameValue}
                  onChange={(e) => setRenameValue(e.target.value)}
                  onKeyDown={(e) => handleKeyDown(e, tab.id)}
                  onBlur={() => commitRename(tab.id)}
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <>
                  <TabLabel label={tab.label} />
                  <div className="absolute right-0 top-0 bottom-0 flex items-center gap-px px-1 opacity-0 group-hover:opacity-100 transition-opacity bg-surface-container-highest">
                    {tab.kind === "chat" && (
                      <button
                        className="flex items-center justify-center w-[18px] h-[18px] text-on-surface-variant hover:text-on-surface hover:bg-white/[0.08] rounded-sm transition-colors"
                        title="Rename chat"
                        onClick={(e) => startRename(e, tab)}
                      >
                        <FontAwesomeIcon icon={faPencil} className={iconClass.sm} />
                      </button>
                    )}
                    <button
                      className="flex items-center justify-center w-[18px] h-[18px] text-on-surface-variant hover:text-on-surface hover:bg-white/[0.08] rounded-sm transition-colors text-[18px] leading-none"
                      onClick={(e) => {
                        e.stopPropagation();
                        onClose(tab.id);
                      }}
                      aria-label={`Close ${tab.label}`}
                    >
                      ×
                    </button>
                  </div>
                </>
              )}
            </div>
          );
        })}
        <div
          className="app-icon-button w-8 cursor-pointer shrink-0"
          onClick={onNew}
          role="button"
          aria-label="New tab"
        >
          <span className="flex items-center justify-center w-6 h-6 rounded text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors">
            <FontAwesomeIcon icon={faPlus} className={iconClass.md} />
          </span>
        </div>
      </div>
      {rightSlot && <div className="relative flex items-center shrink-0">{rightSlot}</div>}
    </div>
  );
}
