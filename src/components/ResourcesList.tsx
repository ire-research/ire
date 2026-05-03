interface SeedResource {
  slug: string;
  title: string;
  status: "indexed" | "pending_review" | "pending_summary";
}

const SEED: SeedResource[] = [
  {
    slug: "attention-is-all-you-need",
    title: "Vaswani et al. (2017) — Attention Is All You Need",
    status: "indexed",
  },
  {
    slug: "lora",
    title: "Hu et al. (2021) — LoRA: Low-Rank Adaptation",
    status: "pending_review",
  },
];

export function ResourcesList() {
  return (
    <section className="resources-list">
      <h3>Resources</h3>
      <ul>
        {SEED.map((r) => (
          <li key={r.slug} className={`resource resource--${r.status}`}>
            <span className="resource__title">{r.title}</span>
            <span className="resource__status">{r.status}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
