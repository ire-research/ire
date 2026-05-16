import { useEffect, useState } from "react";
import type { ExperimentRow, PulseContent } from "../../types";
import { ipc } from "../../ipc";
import { useChat } from "../../state/chat";
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
    <nav
      className="bg-surface-container-low border-r border-outline-variant flex flex-col overflow-hidden"
      style={{ width: 280, minWidth: 160, maxWidth: 420 }}
    >
      {/* Focus: top third */}
      <div className="flex flex-col overflow-hidden" style={{ minHeight: 80, height: "calc((100vh - 64px) / 3)" }}>
        <FocusPane pulse={pulse} />
      </div>

      {/* Divider */}
      <div className="h-px bg-outline-variant shrink-0"></div>

      {/* Resources: middle third */}
      <div className="flex flex-col overflow-hidden" style={{ minHeight: 60, height: "calc((100vh - 64px) / 3)" }}>
        <ResourcesSection resources={resources} onOpen={openPreviewTab} />
      </div>

      {/* Divider */}
      <div className="h-px bg-outline-variant shrink-0"></div>

      {/* Experiments: flex-1 (takes remaining space) */}
      <div className="flex flex-col overflow-hidden flex-1" style={{ minHeight: 60 }}>
        <ExperimentsSection experiments={experiments} onOpen={openExperimentTab} />
      </div>
    </nav>
  );
}
