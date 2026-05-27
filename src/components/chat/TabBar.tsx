import { useRef, useState } from "react";
import type { ReactNode } from "react";
import type { Tab } from "../../types";
import { Icon } from "../Icon";

interface Props {
  tabs: Tab[];
  activeTabId: string;
  onSelect: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onNew: () => void;
  onRename: (tabId: string, newName: string) => void;
  rightSlot?: ReactNode;
}

function tabIcon(tab: Tab): { icon: string; spin: boolean } {
  if (tab.kind === "chat") return { icon: "chat", spin: false };
  if (tab.kind === "resource") {
    if (tab.resourceStatus === "summarizing") return { icon: "progress_activity", spin: true };
    return { icon: "description", spin: false };
  }
  if (tab.kind === "preview") return { icon: "description", spin: false };
  if (tab.kind === "experiment") return { icon: "science", spin: false };
  return { icon: "chat", spin: false };
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
                  ? "group flex items-center px-3 border-r border-outline-variant bg-surface-container-highest text-on-surface text-xs min-w-max cursor-pointer border-t border-t-primary"
                  : "group flex items-center px-3 border-r border-outline-variant text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors text-xs min-w-max cursor-pointer"
              }
              onClick={() => !isRenaming && onSelect(tab.id)}
            >
              <Icon name={icon} className={`w-[14px] h-[14px] mr-1.5${spin ? " animate-spin" : ""}`} />
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
                  <span>{tab.label}</span>
                  {tab.kind === "chat" && (
                    <button
                      className="opacity-0 group-hover:opacity-100 ml-1 flex items-center justify-center text-on-surface-variant hover:text-on-surface transition-colors shrink-0"
                      title="Rename chat"
                      onClick={(e) => startRename(e, tab)}
                    >
                      <Icon name="edit_document" className="w-[12px] h-[12px]" />
                    </button>
                  )}
                  <button
                    className="opacity-0 group-hover:opacity-100 text-[10px] ml-1 text-on-surface-variant hover:text-on-surface"
                    onClick={(e) => {
                      e.stopPropagation();
                      onClose(tab.id);
                    }}
                    aria-label={`Close ${tab.label}`}
                  >
                    ×
                  </button>
                </>
              )}
            </div>
          );
        })}
        <div
          className="flex items-center justify-center w-8 text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors cursor-pointer shrink-0"
          onClick={onNew}
          role="button"
          aria-label="New tab"
        >
          <Icon name="add" className="w-[16px] h-[16px]" />
        </div>
      </div>
      {rightSlot && <div className="relative flex items-center shrink-0">{rightSlot}</div>}
    </div>
  );
}
