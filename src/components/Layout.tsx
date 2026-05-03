import { useEffect, useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { ipc, onWikiChanged } from "../ipc";
import { useWorkspace } from "../state/workspace";
import { ChatPane } from "./chat/ChatPane";
import { FocusBanner } from "./FocusBanner";
import { MarkdownPane } from "./MarkdownPane";
import { ResourceInput } from "./ResourceInput";
import { ResourcesList } from "./ResourcesList";

function parseFocus(pulse: string): string {
  const match = pulse.match(/\*\*Focus:\*\*\s*(.+)/);
  return match ? match[1].trim() : "";
}

export function Layout() {
  const phase = useWorkspace((s) => s.phase);
  const setPhase = useWorkspace((s) => s.setPhase);
  const theme = useWorkspace((s) => s.theme);
  const toggleTheme = useWorkspace((s) => s.toggleTheme);
  const workspace = phase.kind === "ready" ? phase.workspace : null;

  const [pulseContent, setPulseContent] = useState("");
  const [notesContent, setNotesContent] = useState("");
  const [ideasContent, setIdeasContent] = useState("");

  // Load wiki files when workspace becomes ready
  useEffect(() => {
    if (phase.kind !== "ready") return;
    Promise.all([
      ipc.readWikiFile("status/pulse.md"),
      ipc.readWikiFile("notes.md"),
      ipc.readWikiFile("ideas.md"),
    ])
      .then(([pulse, notes, ideas]) => {
        setPulseContent(pulse.content);
        setNotesContent(notes.content);
        setIdeasContent(ideas.content);
      })
      .catch((e) => console.error("failed to load wiki files", e));
  }, [phase.kind]);

  // Re-read affected file on wiki-changed events
  useEffect(() => {
    const unlisten = onWikiChanged(({ path }) => {
      if (path === "status/pulse.md") {
        ipc.readWikiFile("status/pulse.md").then((f) => setPulseContent(f.content));
      } else if (path === "notes.md") {
        ipc.readWikiFile("notes.md").then((f) => setNotesContent(f.content));
      } else if (path === "ideas.md") {
        ipc.readWikiFile("ideas.md").then((f) => setIdeasContent(f.content));
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (theme === "light") {
      document.documentElement.dataset.theme = "light";
    } else {
      delete document.documentElement.dataset.theme;
    }
  }, [theme]);

  const handleClose = async () => {
    await ipc.closeWorkspace();
    const status = await ipc.setupStatus();
    setPhase({ kind: "setup", status });
  };

  const handleSaveNotes = async (content: string) => {
    await ipc.saveNotes(content).catch((e) => console.error("save notes:", e));
  };

  const handleSaveIdeas = async (content: string) => {
    await ipc.saveIdeas(content).catch((e) => console.error("save ideas:", e));
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
        <button onClick={toggleTheme}>
          {theme === "dark" ? "Light" : "Dark"}
        </button>
        <button className="topbar__settings" aria-label="Settings">
          ⚙
        </button>
      </header>

      <Group orientation="horizontal" className="layout__body">
        <Panel
          defaultSize="22%"
          minSize="15%"
          collapsible
          className="column column--left"
        >
          <FocusBanner focus={parseFocus(pulseContent)} />
          <Group orientation="vertical" className="column__inner">
            <Panel defaultSize="55%" minSize="20%">
              <MarkdownPane title="pulse.md" content={pulseContent} />
            </Panel>
            <Separator className="resize-handle resize-handle--v" />
            <Panel defaultSize="45%" minSize="20%">
              <ResourcesList />
            </Panel>
          </Group>
        </Panel>

        <Separator className="resize-handle resize-handle--h" />

        <Panel defaultSize="56%" minSize="30%" className="column column--center">
          <ChatPane />
        </Panel>

        <Separator className="resize-handle resize-handle--h" />

        <Panel
          defaultSize="22%"
          minSize="15%"
          collapsible
          className="column column--right"
        >
          <Group orientation="vertical" className="column__inner">
            <Panel defaultSize="40%" minSize="15%">
              <MarkdownPane
                title="notes.md"
                content={notesContent}
                showSubmit
                onSubmit={handleSaveNotes}
              />
            </Panel>
            <Separator className="resize-handle resize-handle--v" />
            <Panel defaultSize="40%" minSize="15%">
              <MarkdownPane
                title="ideas.md"
                content={ideasContent}
                showSubmit
                onSubmit={handleSaveIdeas}
              />
            </Panel>
            <Separator className="resize-handle resize-handle--v" />
            <Panel defaultSize="20%" minSize="10%">
              <ResourceInput />
            </Panel>
          </Group>
        </Panel>
      </Group>
    </div>
  );
}
