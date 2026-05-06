import type { ResourceItem } from "../types";

interface Props {
  resources: ResourceItem[];
  onResourceClick?: (resource: ResourceItem) => void;
}

function displayTitle(r: ResourceItem): string {
  if (r.title) return r.title;
  try {
    return new URL(r.url).hostname;
  } catch {
    return r.url;
  }
}

export function ResourcesList({ resources, onResourceClick }: Props) {
  if (resources.length === 0) {
    return (
      <section className="resources-list">
        <h3>Resources</h3>
        <p className="resources-list__empty">No resources yet</p>
      </section>
    );
  }

  return (
    <section className="resources-list">
      <h3>Resources</h3>
      <ul>
        {resources.map((r) => (
          <li
            key={r.resource_id}
            className="resource"
            title={r.url}
            onClick={r.wiki_path ? () => onResourceClick?.(r) : undefined}
            style={{ cursor: r.wiki_path ? "pointer" : "default" }}
          >
            <span className="resource__title">{displayTitle(r)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
