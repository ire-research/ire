import type { Tab } from "../../types";

interface Props {
  tabs: Tab[];
  activeTabId: string;
  onSelect: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onNew: () => void;
}

export function TabBar({ tabs, activeTabId, onSelect, onClose, onNew }: Props) {
  return (
    <div className="tab-bar">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          className={`tab-bar__tab${tab.id === activeTabId ? " tab-bar__tab--active" : ""}`}
          onClick={() => onSelect(tab.id)}
        >
          <span className="tab-bar__label">
            {tab.kind === "resource" && tab.resourceStatus === "summarizing" && (
              <span className="tab-bar__spinner" aria-hidden="true" />
            )}
            <span>{tab.label}</span>
          </span>
          {!tab.isPinned && (
            <button
              className="tab-bar__close"
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
      ))}
      <button className="tab-bar__new" onClick={onNew} aria-label="New tab">
        +
      </button>
    </div>
  );
}
