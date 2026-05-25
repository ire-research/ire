import { Group, Panel, Separator } from "react-resizable-panels";
import { useWorkspace } from "../../state/workspace";
import { usePaneSignals, selectRightRailPulse } from "../../state/paneSignals";
import { useTransientClass } from "../../hooks/useTransientClass";
import { NotesPane } from "./NotesPane";
import { IdeasPane } from "./IdeasPane";

export function RightRail() {
  const groupLayout = useWorkspace((s) => s.panelLayout.groups?.right);
  const setGroupLayout = useWorkspace((s) => s.setGroupLayout);
  const railPulse = usePaneSignals(selectRightRailPulse);
  const railRef = useTransientClass<HTMLElement>(railPulse, "rail-signal-active", 1100);
  const defaultLayout =
    groupLayout &&
    Number.isFinite(groupLayout.notes) &&
    Number.isFinite(groupLayout.ideas)
      ? groupLayout
      : undefined;

  return (
    <aside
      ref={railRef}
      data-side="right"
      className="rail-signal h-full flex flex-col bg-surface-container-low border-l border-outline-variant shrink-0 overflow-hidden"
    >
      <Group
        id="right"
        orientation="vertical"
        className="flex-1 overflow-hidden"
        defaultLayout={defaultLayout}
        onLayoutChanged={(layout) => setGroupLayout("right", layout)}
      >
        <Panel id="notes" className="flex flex-col overflow-hidden" defaultSize={50} minSize="80px">
          <NotesPane />
        </Panel>
        <Separator id="right-notes-ideas" className="drag-handle-row border-t border-outline-variant" />
        <Panel id="ideas" className="flex flex-col overflow-hidden" defaultSize={50} minSize="80px">
          <IdeasPane />
        </Panel>
      </Group>
    </aside>
  );
}
