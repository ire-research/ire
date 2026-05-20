import type { Tab } from "../../types";
import { Icon } from "../Icon";

interface Props {
  tabs: Tab[];
  activeTabId: string;
  onSelect: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onNew: () => void;
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

export function TabBar({ tabs, activeTabId, onSelect, onClose, onNew }: Props) {
  return (
    <div className="flex h-8 border-b border-outline-variant bg-surface-container-low shrink-0 px-2 overflow-x-auto no-scrollbar">
      {tabs.map((tab) => {
        const isActive = tab.id === activeTabId;
        const { icon, spin } = tabIcon(tab);
        return (
          <div
            key={tab.id}
            className={
              isActive
                ? "group flex items-center px-3 border-r border-outline-variant bg-surface-container-highest text-on-surface text-xs min-w-max cursor-pointer border-t border-t-primary"
                : "group flex items-center px-3 border-r border-outline-variant text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors text-xs min-w-max cursor-pointer"
            }
            onClick={() => onSelect(tab.id)}
          >
            <Icon name={icon} className={`w-[14px] h-[14px] mr-1.5${spin ? " animate-spin" : ""}`} />
            <span>{tab.label}</span>
            {!tab.isPinned && (
              <button
                className="opacity-0 group-hover:opacity-100 text-[10px] ml-1.5 text-on-surface-variant hover:text-on-surface"
                onClick={(e) => {
                  e.stopPropagation();
                  onClose(tab.id);
                }}
                aria-label={`Close ${tab.label}`}
              >
                ×
              </button>
            )}
          </div>
        );
      })}
      <div
        className="flex items-center justify-center w-8 text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors cursor-pointer"
        onClick={onNew}
        role="button"
        aria-label="New tab"
      >
        <Icon name="add" className="w-[16px] h-[16px]" />
      </div>
    </div>
  );
}
