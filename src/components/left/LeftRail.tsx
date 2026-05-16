import { useEffect, useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import type { ExperimentRow, PulseContent } from "../../types";
import { ipc } from "../../ipc";
import { useChat } from "../../state/chat";
import { useWorkspace } from "../../state/workspace";
import { FocusPane } from "./FocusPane";
import { ResourcesSection } from "./ResourcesSection";
import { ExperimentsSection } from "./ExperimentsSection";

interface ResourceItem {
  label: string;
  wikiPath: string;
}

interface Props {
  pulse: PulseContent;
  resources: ResourceItem[];
}

export function LeftRail({ pulse, resources }: Props) {
  const [experiments, setExperiments] = useState<ExperimentRow[]>([]);
  const openPreviewTab = useChat((s) => s.openPreviewTab);
  const openExperimentTab = useChat((s) => s.openExperimentTab);
  const groupLayout = useWorkspace((s) => s.panelLayout.groups?.left);
  const setGroupLayout = useWorkspace((s) => s.setGroupLayout);
  const defaultLayout =
    groupLayout &&
    Number.isFinite(groupLayout.pulse) &&
    Number.isFinite(groupLayout.resources) &&
    Number.isFinite(groupLayout.experiments)
      ? groupLayout
      : undefined;

  useEffect(() => {
    const loadExperiments = async () => {
      try {
        const result = await ipc.experimentList(50);
        setExperiments(result);
      } catch (e) {
        console.error("Failed to load experiments:", e);
      }
    };

    loadExperiments();

    const interval = setInterval(loadExperiments, 10000);

    return () => clearInterval(interval);
  }, []);

  return (
    <nav className="h-full bg-surface-container-low border-r border-outline-variant flex flex-col overflow-hidden">
      <Group
        id="left"
        orientation="vertical"
        className="flex-1 overflow-hidden"
        defaultLayout={defaultLayout}
        onLayoutChanged={(layout) => setGroupLayout("left", layout)}
      >
        <Panel id="pulse" className="flex flex-col overflow-hidden" defaultSize={33.33} minSize="80px">
          <FocusPane pulse={pulse} />
        </Panel>
        <Separator id="left-focus-resources" className="drag-handle-row border-t border-outline-variant" />
        <Panel id="resources" className="flex flex-col overflow-hidden" defaultSize={33.33} minSize="60px">
          <ResourcesSection resources={resources} onOpen={openPreviewTab} />
        </Panel>
        <Separator id="left-resources-experiments" className="drag-handle-row border-t border-outline-variant" />
        <Panel id="experiments" className="flex flex-col overflow-hidden" defaultSize={33.34} minSize="60px">
          <ExperimentsSection experiments={experiments} onOpen={openExperimentTab} />
        </Panel>
      </Group>
    </nav>
  );
}
