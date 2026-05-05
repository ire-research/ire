import type { ResourceItem } from "../types";

interface Props {
  resources: ResourceItem[];
}

function displayTitle(r: ResourceItem): string {
  if (r.title) return r.title;
  try {
    return new URL(r.url).hostname;
  } catch {
    return r.url;
  }
}

export function ResourcesList({ resources }: Props) {
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
          <li key={r.resource_id} className="resource" title={r.url}>
            <span className="resource__title">{displayTitle(r)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
