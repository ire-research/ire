import { useEffect, useRef, useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { ipc, onWikiChanged } from "../ipc";
import { useChat } from "../state/chat";
import { useWorkspace } from "../state/workspace";
import { useChatOptions } from "../state/chatOptions";
import { toastError } from "../state/toasts";
import type { PulseContent, ResourceItem } from "../types";
import { ChatPane } from "./chat/ChatPane";
import { FocusPane } from "./left/FocusPane";
import { MarkdownPane } from "./MarkdownPane";
import { ResourceInput } from "./ResourceInput";
import { ResourcesList } from "./ResourcesList";

export function Layout() {
  const openPreviewTab = useChat((s) => s.openPreviewTab);
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);
  const panelLayout = useWorkspace((s) => s.panelLayout);
  const setGroupLayout = useWorkspace((s) => s.setGroupLayout);
  const toPersisted = useWorkspace((s) => s.toPersisted);
  const effort = useChatOptions((s) => s.effort);
  const workspace = phase.kind === "ready" ? phase.workspace : null;

  const groups = panelLayout.groups ?? {};

  const [pulseContent, setPulseContent] = useState("");
  const [pulseObject, setPulseObject] = useState<PulseContent>({ research_question: "", this_week: "" });
  const [notesContent, setNotesContent] = useState("");
  const [ideasContent, setIdeasContent] = useState("");
  const [resources, setResources] = useState<ResourceItem[]>([]);

  // Load wiki files and resources when workspace becomes ready
  useEffect(() => {
    if (phase.kind !== "ready") return;
    Promise.all([
      ipc.readWikiFile("status/pulse.md"),
      ipc.readWikiFile("notes.md"),
      ipc.readWikiFile("ideas.md"),
      ipc.readPulse(),
    ])
      .then(([pulse, notes, ideas, pulseData]) => {
        setPulseContent(pulse.content);
        setPulseObject(pulseData);
        setNotesContent(notes.content);
        setIdeasContent(ideas.content);
      })
      .catch((e) => toastError("load wiki", e));

    ipc.listResources()
      .then(setResources)
      .catch((e) => toastError("load resources", e));
  }, [phase.kind]);

  // Re-read affected file on wiki-changed events
  useEffect(() => {
    const unlisten = onWikiChanged(({ path }) => {
      if (path === "status/pulse.md") {
        ipc.readWikiFile("status/pulse.md").then((f) => setPulseContent(f.content));
        ipc.readPulse().then((p) => setPulseObject(p)).catch((e) => toastError("load pulse", e));
      } else if (path === "notes.md") {
        ipc.readWikiFile("notes.md").then((f) => setNotesContent(f.content));
      } else if (path === "ideas.md") {
        ipc.readWikiFile("ideas.md").then((f) => setIdeasContent(f.content));
      } else if (path.startsWith("resources/")) {
        ipc.listResources().then(setResources).catch((e) => toastError("load resources", e));
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Debounced persistence of panel layout to .ire/workspace.json.
  // Skip the initial render (otherwise we overwrite the loaded file with defaults).
  const skipInitialLayoutSave = useRef(true);
  useEffect(() => {
    if (skipInitialLayoutSave.current) {
      skipInitialLayoutSave.current = false;
      return;
    }
    const handle = setTimeout(() => {
      ipc.saveWorkspaceState(toPersisted()).catch((e) =>
        toastError("save layout", e),
      );
    }, 1000);
    return () => clearTimeout(handle);
  }, [panelLayout, toPersisted]);

  // Debounced persistence of effort to .ire/workspace.json.
  const skipInitialEffortSave = useRef(true);
  useEffect(() => {
    if (skipInitialEffortSave.current) {
      skipInitialEffortSave.current = false;
      return;
    }
    const handle = setTimeout(() => {
      ipc.saveWorkspaceState({ ...toPersisted(), effort }).catch((e) =>
        toastError("save effort", e),
      );
    }, 1000);
    return () => clearTimeout(handle);
  }, [effort, toPersisted]);

  const handleClose = async () => {
    await ipc.closeWorkspace();
    const status = await ipc.setupStatus();
    setPhase({ kind: "setup", status });
  };

  const handleSaveNotes = async (content: string) => {
    await ipc.saveNotes(content).catch((e) => toastError("save notes", e));
  };

  const handleSaveIdeas = async (content: string) => {
    await ipc.saveIdeas(content).catch((e) => toastError("save ideas", e));
  };

  return (
    <div className="layout">
      <header className="topbar">
        <div className="topbar__name">{workspace?.name ?? "workspace"}</div>
        <div className="topbar__path" title={workspace?.path}>
          {workspace?.path}
        </div>
        <div className="topbar__spacer" />
        <button onClick={handleClose}>Close</button>
        <button className="topbar__settings" aria-label="Settings">
          ⚙
        </button>
      </header>

      <Group
        id="body"
        orientation="horizontal"
        className="layout__body"
        defaultLayout={groups.body}
        onLayoutChanged={(layout) => setGroupLayout("body", layout)}
      >
        <Panel
          id="left"
          defaultSize="22%"
          minSize="15%"
          collapsible
          className="column column--left"
        >
          <FocusPane pulse={pulseObject} />
          <Group
            id="left"
            orientation="vertical"
            className="column__inner"
            defaultLayout={groups.left}
            onLayoutChanged={(layout) => setGroupLayout("left", layout)}
          >
            <Panel id="pulse" defaultSize="55%" minSize="20%">
              <MarkdownPane title="pulse.md" content={pulseContent} />
            </Panel>
            <Separator className="resize-handle resize-handle--v" />
            <Panel id="resources" defaultSize="45%" minSize="20%">
              <ResourcesList
                resources={resources}
                onResourceClick={(r) => openPreviewTab(r.title ?? r.url, r.wiki_path!)}
              />
            </Panel>
          </Group>
        </Panel>

        <Separator className="resize-handle resize-handle--h" />

        <Panel id="center" defaultSize="56%" minSize="30%" className="column column--center">
          <ChatPane />
        </Panel>

        <Separator className="resize-handle resize-handle--h" />

        <Panel
          id="right"
          defaultSize="22%"
          minSize="15%"
          collapsible
          className="column column--right"
        >
          <Group
            id="right"
            orientation="vertical"
            className="column__inner"
            defaultLayout={groups.right}
            onLayoutChanged={(layout) => setGroupLayout("right", layout)}
          >
            <Panel id="notes" defaultSize="40%" minSize="15%">
              <MarkdownPane
                title="notes.md"
                content={notesContent}
                showSubmit
                onSubmit={handleSaveNotes}
              />
            </Panel>
            <Separator className="resize-handle resize-handle--v" />
            <Panel id="ideas" defaultSize="40%" minSize="15%">
              <MarkdownPane
                title="ideas.md"
                content={ideasContent}
                showSubmit
                onSubmit={handleSaveIdeas}
              />
            </Panel>
            <Separator className="resize-handle resize-handle--v" />
            <Panel id="resource-input" defaultSize="20%" minSize="10%">
              <ResourceInput />
            </Panel>
          </Group>
        </Panel>
      </Group>
    </div>
  );
}
