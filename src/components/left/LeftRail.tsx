import { Group, Panel, Separator } from "react-resizable-panels";
import { useChat } from "../../state/chat";
import { useWorkspace } from "../../state/workspace";
import { useWorkspaceData } from "../../state/workspaceData";
import { FocusPane } from "./FocusPane";
import { ResourcesSection } from "./ResourcesSection";
import { ExperimentsSection } from "./ExperimentsSection";

export function LeftRail() {
  const experiments = useWorkspaceData((s) => s.experiments);
  const openPreviewTab = useChat((s) => s.openPreviewTab);
  const openExperimentTab = useChat((s) => s.openExperimentTab);
  const groupLayout = useWorkspace((s) => s.panelLayout.groups?.left);
  const setGroupLayout = useWorkspace((s) => s.setGroupLayout);
  const defaultLayout =
    groupLayout &&
    Number.isFinite(groupLayout.focus) &&
    Number.isFinite(groupLayout.resources) &&
    Number.isFinite(groupLayout.experiments)
      ? groupLayout
      : undefined;

  return (
    <nav className="h-full bg-surface-container-low border-r border-outline-variant flex flex-col overflow-hidden">
      <Group
        id="left"
        orientation="vertical"
        className="flex-1 overflow-hidden"
        defaultLayout={defaultLayout}
        onLayoutChanged={(layout) => setGroupLayout("left", layout)}
      >
        <Panel id="focus" className="flex flex-col overflow-hidden" defaultSize={33.33} minSize="80px">
          <FocusPane />
        </Panel>
        <Separator id="left-focus-resources" className="drag-handle-row border-t border-outline-variant" />
        <Panel id="resources" className="flex flex-col overflow-hidden" defaultSize={33.33} minSize="60px">
          <ResourcesSection onOpen={openPreviewTab} />
        </Panel>
        <Separator id="left-resources-experiments" className="drag-handle-row border-t border-outline-variant" />
        <Panel id="experiments" className="flex flex-col overflow-hidden" defaultSize={33.34} minSize="60px">
          <ExperimentsSection
            experiments={experiments}
            onOpen={openExperimentTab}
          />
        </Panel>
      </Group>
    </nav>
  );
}
