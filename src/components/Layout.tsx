import { useEffect, useRef, useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import type { PanelImperativeHandle } from "react-resizable-panels";
import { ipc } from "../ipc";
import { useWorkspace } from "../state/workspace";
import { useChat } from "../state/chat";
import { useWorkspaceData, selectRunningCount } from "../state/workspaceData";
import { useChatOptions } from "../state/chatOptions";
import { toastError } from "../state/toasts";
import { ChatPane } from "./chat/ChatPane";
import { LeftRail } from "./left/LeftRail";
import { RightRail } from "./right/RightRail";
import { StatusBar } from "./StatusBar";
import { Icon } from "./Icon";

export function Layout() {
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);
  const toPersisted = useWorkspace((s) => s.toPersisted);
  const panelLayout = useWorkspace((s) => s.panelLayout);
  const setGroupLayout = useWorkspace((s) => s.setGroupLayout);
  const setPanelCollapsed = useWorkspace((s) => s.setPanelCollapsed);
  const model = useChatOptions((s) => s.model);
  const provider = useChatOptions((s) => s.provider);
  const effort = useChatOptions((s) => s.effort);
  const tabs = useChat((s) => s.tabs);
  const activeTabId = useChat((s) => s.activeTabId);
  const leftPanelRef = useRef<PanelImperativeHandle>(null);
  const rightPanelRef = useRef<PanelImperativeHandle>(null);

  const [wsDropdownOpen, setWsDropdownOpen] = useState(false);
  const wsDropdownRef = useRef<HTMLDivElement>(null);

  const [helpOpen, setHelpOpen] = useState(false);
  const helpRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (wsDropdownRef.current && !wsDropdownRef.current.contains(e.target as Node)) {
        setWsDropdownOpen(false);
      }
      if (helpRef.current && !helpRef.current.contains(e.target as Node)) {
        setHelpOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const handleOpenInVscode = async () => {
    setWsDropdownOpen(false);
    await ipc.openInVscode(workspacePath).catch((e: unknown) => toastError("open in VS Code", e));
  };

  const runningCount = useWorkspaceData(selectRunningCount);

  // Debounced workspace state persistence
  const skipInitialSave = useRef(true);
  useEffect(() => {
    if (skipInitialSave.current) { skipInitialSave.current = false; return; }
    const handle = setTimeout(() => {
      ipc.saveWorkspaceState(toPersisted()).catch((e) => toastError("save state", e));
    }, 1000);
    return () => clearTimeout(handle);
  }, [panelLayout, model, provider, effort, tabs, activeTabId, toPersisted]);

  const handleClose = async () => {
    await ipc.saveWorkspaceState(toPersisted()).catch((e) => toastError("save state", e));
    // Save all non-empty, non-streaming chat tabs to history before closing.
    const currentTabs = useChat.getState().tabs;
    for (const tab of currentTabs) {
      if (tab.kind === "chat" && tab.messages.length > 0 && !tab.isStreaming) {
        const sessionUuid = tab.historySessionUuid ?? crypto.randomUUID();
        const startedAt = tab.historyStartedAt ?? new Date().toISOString();
        await ipc
          .chatHistorySave(tab.label, provider, model, startedAt, JSON.stringify(tab.messages), sessionUuid)
          .catch(() => {}); // best-effort
      }
    }
    await ipc.closeWorkspace();
    useChat.getState().reset();
    const status = await ipc.setupStatus();
    setPhase({ kind: "setup", status });
  };

  const workspacePath = phase.kind === "ready" ? phase.workspace.path : "";
  const storedBodyLayout = panelLayout.groups?.body;
  const bodyLayout =
    storedBodyLayout &&
    Number.isFinite(storedBodyLayout.left) &&
    Number.isFinite(storedBodyLayout.center) &&
    Number.isFinite(storedBodyLayout.right)
      ? storedBodyLayout
      : undefined;
  const leftCollapsed = panelLayout.collapsed?.left ?? false;
  const rightCollapsed = panelLayout.collapsed?.right ?? false;

  useEffect(() => {
    const panel = leftPanelRef.current;
    if (!panel) return;
    if (leftCollapsed && !panel.isCollapsed()) {
      panel.collapse();
    } else if (!leftCollapsed && panel.isCollapsed()) {
      panel.expand();
    }
  }, [leftCollapsed]);

  useEffect(() => {
    const panel = rightPanelRef.current;
    if (!panel) return;
    if (rightCollapsed && !panel.isCollapsed()) {
      panel.collapse();
    } else if (!rightCollapsed && panel.isCollapsed()) {
      panel.expand();
    }
  }, [rightCollapsed]);

  const toggleLeftRail = () => {
    const panel = leftPanelRef.current;
    if (!panel) return;
    const nextCollapsed = !panel.isCollapsed();
    if (nextCollapsed) {
      panel.collapse();
    } else {
      panel.expand();
    }
    setPanelCollapsed("left", nextCollapsed);
  };

  const toggleRightRail = () => {
    const panel = rightPanelRef.current;
    if (!panel) return;
    const nextCollapsed = !panel.isCollapsed();
    if (nextCollapsed) {
      panel.collapse();
    } else {
      panel.expand();
    }
    setPanelCollapsed("right", nextCollapsed);
  };
  const syncLeftCollapsed = () => {
    const collapsed = leftPanelRef.current?.isCollapsed() ?? false;
    if (collapsed !== leftCollapsed) setPanelCollapsed("left", collapsed);
  };
  const syncRightCollapsed = () => {
    const collapsed = rightPanelRef.current?.isCollapsed() ?? false;
    if (collapsed !== rightCollapsed) setPanelCollapsed("right", collapsed);
  };
  const toggleButtonClass = "text-on-surface-variant hover:text-on-surface transition-colors flex items-center justify-center w-7 h-7 rounded border border-transparent hover:bg-surface-container-low hover:border-outline-variant";

  return (
    <div className="flex flex-col h-screen bg-background text-on-surface overflow-hidden">
      {/* Top NavBar */}
      <header className="flex items-center justify-between px-3 h-10 w-full bg-background border-b border-outline-variant shrink-0">
        <div className="flex items-center gap-3 min-w-0">
          <div className="flex items-center gap-1.5 min-w-0 text-xs font-medium text-on-surface" ref={wsDropdownRef} title={workspacePath}>
            <Icon name="folder" className="w-[16px] h-[16px] text-primary shrink-0" />
            <span className="truncate max-w-[360px]">{workspacePath}</span>
            <div className="relative shrink-0">
              <button
                onClick={() => setWsDropdownOpen((o) => !o)}
                className="text-on-surface-variant hover:text-on-surface transition-colors ml-0.5"
                aria-label="Workspace options"
              >
                <Icon name="expand_more" className="w-[14px] h-[14px]" />
              </button>
              {wsDropdownOpen && (
                <div className="absolute left-0 top-full mt-1 bg-surface-container-low border border-outline-variant rounded shadow-lg z-50 min-w-[160px]">
                  <button
                    className="flex items-center gap-2 w-full px-3 py-2 text-xs text-on-surface hover:bg-surface-container transition-colors"
                    onClick={handleOpenInVscode}
                  >
                    <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 shrink-0" fill="currentColor" aria-hidden="true">
                      <path d="M23.15 2.587L18.21.21a1.494 1.494 0 0 0-1.705.29l-9.46 8.63-4.12-3.128a.999.999 0 0 0-1.276.057L.327 7.261A1 1 0 0 0 .326 8.74L3.899 12 .326 15.26a1 1 0 0 0 .001 1.479L1.65 17.94a.999.999 0 0 0 1.276.057l4.12-3.128 9.46 8.63a1.492 1.492 0 0 0 1.704.29l4.942-2.377A1.5 1.5 0 0 0 24 19.08V4.92a1.5 1.5 0 0 0-.85-1.333zm-5.146 14.861L10.826 12l7.178-5.448v10.896z" />
                    </svg>
                    Open in VS Code
                  </button>
                </div>
              )}
            </div>
          </div>
          {runningCount > 0 && (
            <div className="flex items-center gap-2 border border-warn/30 text-warn px-2 py-0.5 rounded text-xs bg-warn/5">
              <span className="w-1.5 h-1.5 rounded-full bg-warn animate-pulse" />
              running {runningCount} exp
            </div>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <div className="flex items-center gap-1">
            <button
              className={toggleButtonClass}
              onClick={toggleLeftRail}
              aria-label={leftCollapsed ? "Show left sidebar" : "Hide left sidebar"}
              title={leftCollapsed ? "Show left sidebar" : "Hide left sidebar"}
            >
              <Icon name="sidebar_left" className="w-[18px] h-[18px]" />
            </button>
            <button
              className={toggleButtonClass}
              onClick={toggleRightRail}
              aria-label={rightCollapsed ? "Show right sidebar" : "Hide right sidebar"}
              title={rightCollapsed ? "Show right sidebar" : "Hide right sidebar"}
            >
              <Icon name="sidebar_right" className="w-[18px] h-[18px]" />
            </button>
          </div>
          <button
            className="text-on-surface-variant hover:text-on-surface transition-colors flex items-center justify-center p-1"
            aria-label="Settings"
          >
            <Icon name="settings" className="w-[18px] h-[18px]" />
          </button>
          <div className="relative" ref={helpRef}>
            <button
              className={`text-on-surface-variant hover:text-on-surface transition-colors flex items-center justify-center w-7 h-7 rounded border ${helpOpen ? "border-outline-variant bg-surface-container-low text-on-surface" : "border-transparent"}`}
              onClick={() => setHelpOpen((o) => !o)}
              aria-label="Help"
              aria-haspopup="true"
              aria-expanded={helpOpen}
            >
              <Icon name="help" className="w-[16px] h-[16px]" />
            </button>
            {helpOpen && (
              <div className="absolute top-full right-0 mt-1.5 w-[200px] bg-surface-container-high border border-outline-variant rounded-lg shadow-lg z-50 overflow-hidden">
                <a
                  className="flex items-center gap-2 w-full px-3 py-2 text-xs font-medium text-on-surface-variant hover:bg-surface-container-highest hover:text-on-surface transition-colors"
                  href="https://github.com/giacomo-ciro/ire/issues"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  <Icon name="info" className="w-[14px] h-[14px] shrink-0" />
                  Report a bug
                </a>
                <div className="flex items-center gap-2 px-3 py-2 border-t border-outline-variant">
                  <span className="text-xs font-normal text-outline-variant">IRE</span>
                  <span className="text-[10px] font-medium text-on-surface-variant bg-surface-container-highest border border-outline-variant rounded px-[5px] py-[1px]">v0.0.0</span>
                </div>
              </div>
            )}
          </div>
          <button
            className="h-7 border border-outline-variant rounded px-3 text-xs font-medium text-on-surface-variant hover:bg-surface-container-low hover:border-outline transition-colors"
            onClick={handleClose}
          >
            close
          </button>
        </div>
      </header>
      {/* Main content: rails + center */}
      <Group
        id="body"
        orientation="horizontal"
        className="flex-1 overflow-hidden"
        defaultLayout={bodyLayout}
        onLayoutChanged={(layout) => setGroupLayout("body", layout)}
      >
        <Panel
          id="left"
          className="h-full"
          defaultSize="280px"
          collapsedSize="0px"
          collapsible
          minSize="160px"
          groupResizeBehavior="preserve-pixel-size"
          panelRef={leftPanelRef}
          onResize={syncLeftCollapsed}
        >
          <LeftRail />
        </Panel>
        <Separator id="body-left-center" className={leftCollapsed ? "hidden" : "drag-handle-col"} disabled={leftCollapsed} />
        <Panel id="center" className="h-full min-w-0" minSize="320px">
          <ChatPane />
        </Panel>
        <Separator id="body-center-right" className={rightCollapsed ? "hidden" : "drag-handle-col"} disabled={rightCollapsed} />
        <Panel
          id="right"
          className="h-full"
          defaultSize="320px"
          collapsedSize="0px"
          collapsible
          minSize="180px"
          groupResizeBehavior="preserve-pixel-size"
          panelRef={rightPanelRef}
          onResize={syncRightCollapsed}
        >
          <RightRail />
        </Panel>
      </Group>
      {/* Bottom status bar */}
      <StatusBar />
    </div>
  );
}
