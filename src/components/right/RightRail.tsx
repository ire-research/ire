import { Group, Panel, Separator } from "react-resizable-panels";
import type { IdeaItem } from "../../types";
import { useWorkspace } from "../../state/workspace";
import { NotesPane } from "./NotesPane";
import { IdeasPane } from "./IdeasPane";

interface Props {
  notes: string;
  ideas: IdeaItem[];
  onSaveNotes: (content: string) => Promise<void>;
  onSaveIdeas: (ideas: IdeaItem[]) => Promise<void>;
}

export function RightRail({ notes, ideas, onSaveNotes, onSaveIdeas }: Props) {
  const groupLayout = useWorkspace((s) => s.panelLayout.groups?.right);
  const setGroupLayout = useWorkspace((s) => s.setGroupLayout);
  const defaultLayout =
    groupLayout &&
    Number.isFinite(groupLayout.notes) &&
    Number.isFinite(groupLayout.ideas)
      ? groupLayout
      : undefined;

  return (
    <aside className="h-full flex flex-col bg-surface-container-low border-l border-outline-variant shrink-0 overflow-hidden">
      <Group
        id="right"
        orientation="vertical"
        className="flex-1 overflow-hidden"
        defaultLayout={defaultLayout}
        onLayoutChanged={(layout) => setGroupLayout("right", layout)}
      >
        <Panel id="notes" className="flex flex-col overflow-hidden" defaultSize={50} minSize="80px">
          <NotesPane content={notes} onSave={onSaveNotes} />
        </Panel>
        <Separator id="right-notes-ideas" className="drag-handle-row border-t border-outline-variant" />
        <Panel id="ideas" className="flex flex-col overflow-hidden" defaultSize={50} minSize="80px">
          <IdeasPane ideas={ideas} onSave={onSaveIdeas} />
        </Panel>
      </Group>
    </aside>
  );
}
