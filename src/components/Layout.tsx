import { Group, Panel, Separator } from "react-resizable-panels";
import { useWorkspace } from "../state/workspace";
import { FocusBanner } from "./FocusBanner";
import { MarkdownPane } from "./MarkdownPane";
import { ResourceInput } from "./ResourceInput";
import { ResourcesList } from "./ResourcesList";
import { ChatPane } from "./chat/ChatPane";

const SEED_PULSE = `# Pulse

**Question:** What inductive bias makes self-attention generalize past training-length sequences?

**Blocker:** Need a clean ablation comparing RoPE vs ALiBi at extrapolation.

**Focus:** Reproduce ALiBi's headline result on a small transformer.
`;

const SEED_NOTES = `- Talked to V. about positional encoding choices
- Reread the ALiBi paper, sections 3-4
- TODO: write the eval harness
`;

const SEED_IDEAS = `- Try a hybrid RoPE+ALiBi where heads vote
- Ablate by sequence length, not just by metric
`;

export function Layout() {
  const workspaceName = useWorkspace((s) => s.workspaceName);

  return (
    <div className="layout">
      <header className="topbar">
        <div className="topbar__name">{workspaceName}</div>
        <div className="topbar__spacer" />
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
          <FocusBanner focus="Reproduce ALiBi's headline result on a small transformer." />
          <Group orientation="vertical" className="column__inner">
            <Panel defaultSize="55%" minSize="20%">
              <MarkdownPane title="pulse.md" initialContent={SEED_PULSE} />
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
                initialContent={SEED_NOTES}
                showSubmit
              />
            </Panel>
            <Separator className="resize-handle resize-handle--v" />
            <Panel defaultSize="40%" minSize="15%">
              <MarkdownPane
                title="ideas.md"
                initialContent={SEED_IDEAS}
                showSubmit
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
