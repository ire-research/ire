import type { IdeaItem } from "../../types";
import { NotesPane } from "./NotesPane";
import { IdeasPane } from "./IdeasPane";
import { AddResourceSection } from "./AddResourceSection";

interface Props {
  notes: string;
  ideas: IdeaItem[];
  onSaveNotes: (content: string) => Promise<void>;
  onSaveIdeas: (ideas: IdeaItem[]) => Promise<void>;
}

export function RightRail({ notes, ideas, onSaveNotes, onSaveIdeas }: Props) {
  return (
    <aside
      className="flex flex-col bg-surface-container-low border-l border-outline-variant shrink-0 overflow-hidden"
      style={{ width: 320, minWidth: 180, maxWidth: 440 }}
    >
      {/* Notes: top third */}
      <div
        className="flex flex-col overflow-hidden"
        style={{ minHeight: 80, height: "calc((100vh - 64px) / 3)" }}
      >
        <NotesPane content={notes} onSave={onSaveNotes} />
      </div>

      {/* Divider */}
      <div className="h-px bg-outline-variant shrink-0"></div>

      {/* Ideas: middle third */}
      <div
        className="flex flex-col overflow-hidden"
        style={{ minHeight: 80, height: "calc((100vh - 64px) / 3)" }}
      >
        <IdeasPane ideas={ideas} onSave={onSaveIdeas} />
      </div>

      {/* Divider */}
      <div className="h-px bg-outline-variant shrink-0"></div>

      {/* AddResource: flex-1 (remaining space) */}
      <div
        className="flex flex-col overflow-hidden flex-1"
        style={{ minHeight: 72 }}
      >
        <AddResourceSection />
      </div>
    </aside>
  );
}
