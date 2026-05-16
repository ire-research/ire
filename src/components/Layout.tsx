import { useEffect, useRef, useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { ipc, onWikiChanged } from "../ipc";
import { useWorkspace } from "../state/workspace";
import { useChatOptions } from "../state/chatOptions";
import { toastError } from "../state/toasts";
import type { IdeaItem, PulseContent, ResourceItem } from "../types";
import { ChatPane } from "./chat/ChatPane";
import { LeftRail } from "./left/LeftRail";
import { RightRail } from "./right/RightRail";
import { StatusBar } from "./StatusBar";

export function Layout() {
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);
  const toPersisted = useWorkspace((s) => s.toPersisted);
  const panelLayout = useWorkspace((s) => s.panelLayout);
  const setGroupLayout = useWorkspace((s) => s.setGroupLayout);
  const effort = useChatOptions((s) => s.effort);

  const [pulseObject, setPulseObject] = useState<PulseContent>({ research_question: "", this_week: "" });
  const [notesContent, setNotesContent] = useState("");
  const [ideas, setIdeas] = useState<IdeaItem[]>([]);
  const [resources, setResources] = useState<ResourceItem[]>([]);
  const [runningCount, setRunningCount] = useState(0);

  // Load data on workspace ready
  useEffect(() => {
    if (phase.kind !== "ready") return;
    Promise.all([
      ipc.readPulse(),
      ipc.readWikiFile("notes.md"),
      ipc.readIdeas(),
      ipc.listResources(),
      ipc.experimentList(50),
    ])
      .then(([pulseData, notes, ideasData, resourcesData, exps]) => {
        setPulseObject(pulseData);
        setNotesContent(notes.content);
        setIdeas(ideasData);
        setResources(resourcesData);
        setRunningCount(exps.filter((e) => e.status === "running").length);
      })
      .catch((e: unknown) => toastError("load workspace data", e));
  }, [phase.kind]);

  // Wiki-changed listener
  useEffect(() => {
    const unlisten = onWikiChanged(({ path }) => {
      if (path === "pulse/RESEARCH-QUESTION.md" || path === "pulse/THIS-WEEK.md") {
        ipc.readPulse().then(setPulseObject).catch((e) => toastError("load pulse", e));
      } else if (path === "notes.md") {
        ipc.readWikiFile("notes.md").then((f) => setNotesContent(f.content)).catch(() => {});
      } else if (path === "ideas.json") {
        ipc.readIdeas().then(setIdeas).catch(() => {});
      } else if (path.startsWith("resources/")) {
        ipc.listResources().then(setResources).catch((e) => toastError("load resources", e));
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Debounced workspace state persistence
  const skipInitialSave = useRef(true);
  useEffect(() => {
    if (skipInitialSave.current) { skipInitialSave.current = false; return; }
    const handle = setTimeout(() => {
      ipc.saveWorkspaceState(toPersisted()).catch((e) => toastError("save state", e));
    }, 1000);
    return () => clearTimeout(handle);
  }, [toPersisted]);

  // Debounced effort persistence
  const skipInitialEffortSave = useRef(true);
  useEffect(() => {
    if (skipInitialEffortSave.current) { skipInitialEffortSave.current = false; return; }
    const handle = setTimeout(() => {
      ipc.saveWorkspaceState({ ...toPersisted(), effort }).catch((e) => toastError("save effort", e));
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

  const handleSaveIdeas = async (updatedIdeas: IdeaItem[]) => {
    try {
      await ipc.saveIdeasJson(updatedIdeas);
      setIdeas(updatedIdeas);
    } catch (e) {
      toastError("save ideas", e);
    }
  };

  const railResources = resources.map((r) => ({
    label: r.title ?? r.url,
    wikiPath: r.wiki_path ?? "",
  }));
  const storedBodyLayout = panelLayout.groups?.body;
  const bodyLayout =
    storedBodyLayout &&
    Number.isFinite(storedBodyLayout.left) &&
    Number.isFinite(storedBodyLayout.center) &&
    Number.isFinite(storedBodyLayout.right)
      ? storedBodyLayout
      : undefined;

  return (
    <div className="flex flex-col h-screen bg-background text-on-surface overflow-hidden">
      {/* Top NavBar */}
      <header className="flex items-center justify-between px-3 h-10 w-full bg-background border-b border-outline-variant shrink-0">
        <div className="flex items-center gap-2">
          {runningCount > 0 && (
            <div className="flex items-center gap-2 border border-warn/30 text-warn px-2 py-0.5 rounded text-xs bg-warn/5">
              <span className="w-1.5 h-1.5 rounded-full bg-warn animate-pulse" />
              running {runningCount} exp
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            className="text-on-surface-variant hover:text-on-surface transition-colors text-xs px-2 py-1"
            onClick={handleClose}
          >
            close workspace
          </button>
          <button
            className="text-on-surface-variant hover:text-on-surface transition-colors flex items-center justify-center p-1"
            aria-label="Settings"
          >
            <span className="material-symbols-outlined text-[18px]">settings</span>
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
          minSize="160px"
          maxSize="420px"
          groupResizeBehavior="preserve-pixel-size"
        >
          <LeftRail pulse={pulseObject} resources={railResources} />
        </Panel>
        <Separator id="body-left-center" className="drag-handle-col" />
        <Panel id="center" className="h-full min-w-0" minSize="320px">
          <ChatPane />
        </Panel>
        <Separator id="body-center-right" className="drag-handle-col" />
        <Panel
          id="right"
          className="h-full"
          defaultSize="320px"
          minSize="180px"
          maxSize="440px"
          groupResizeBehavior="preserve-pixel-size"
        >
          <RightRail
            notes={notesContent}
            ideas={ideas}
            onSaveNotes={handleSaveNotes}
            onSaveIdeas={handleSaveIdeas}
          />
        </Panel>
      </Group>
      {/* Bottom status bar */}
      <StatusBar />
    </div>
  );
}
