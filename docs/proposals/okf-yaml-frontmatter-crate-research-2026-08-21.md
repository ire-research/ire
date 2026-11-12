# OKF and YAML Frontmatter Crates for IRE

Research date: August 21, 2026

## Decision

Adopt the [`okf`](https://crates.io/crates/okf) crate. It is a pure-Rust, zero-dependency implementation of exactly the spec IRE is targeting — Open Knowledge Format v0.2 — and it already solves the two hard requirements (order-preserving round-trip and preservation of unknown keys) that a general YAML crate would force IRE to solve by hand.

The main caveat is adoption: 414 all-time downloads, 18 GitHub stars, first published June 2026. Mitigation is that it is Apache-2.0 with zero dependencies and a small surface, so it can be vendored if it ever goes unmaintained. The fallback, if a dependency this young is unacceptable, is `yaml-rust2` plus IRE's own splicing layer — not `serde_yaml`, which is archived.

---

## 1. Is there a Rust crate that implements OKF?

Yes — one serious one, plus a fork.

| Crate | Version | Published | Downloads | License | Deps |
|---|---|---|---|---|---|
| [`okf`](https://crates.io/crates/okf) | 0.2.1 | 2026-07-27 | 414 | Apache-2.0 | **zero** |
| [`okf-permissive`](https://crates.io/crates/okf-permissive) | 0.2.0 | 2026-07-25 | 61 | — | — |
| [`okf-graph`](https://docs.rs/okf-graph/latest/okf_graph/) | — | — | — | — | — |

`okf` lives at [github.com/W4G1/okf](https://github.com/W4G1/okf) (Apache-2.0, created 2026-06-16, last push 2026-08-13, 18 stars — [GitHub API](https://api.github.com/repos/W4G1/okf)). Its own README describes it as "A pure-Rust, zero-dependency implementation of the Open Knowledge Format (OKF)" and states it is licensed Apache-2.0 "the same license as the upstream [OKF project]".

**It is not Google's.** The [GoogleCloudPlatform/knowledge-catalog](https://github.com/GoogleCloudPlatform/knowledge-catalog) repo reports its languages as TypeScript, HTML, Python, JavaScript, CSS — [no Rust at all](https://api.github.com/repos/GoogleCloudPlatform/knowledge-catalog/languages). The `okf/` directory there contains `SPEC.md`, `pyproject.toml`, `src`, `tests`, `bundles`, `samples` — i.e. the reference implementation is Python. `okf` (the crate) is a third-party port that explicitly tracks it: its README claims compatibility is "checked against the reference's four published bundles (`acme_retail`, `crypto_bitcoin`, `ga4`, `stackoverflow`): all 53 concepts load, every one is conformant, and each document's frontmatter re-serializes to a value PyYAML reads back identically."

`okf-permissive` is a relaxation ("accepting spaces and emoji in names"); ignore it unless IRE needs non-conformant concept ids.

### What `okf` actually gives IRE

Types and modules ([docs.rs/okf](https://docs.rs/okf/latest/okf/)):

- [`Document`](https://docs.rs/okf/latest/okf/struct.Document.html) — `pub struct Document { pub frontmatter: Frontmatter, pub body: String }`, with `Document::parse(text: &str)`, `Document::new(frontmatter, body)`, and `Document::serialize() -> String`.
- [`Frontmatter`](https://docs.rs/okf/latest/okf/struct.Frontmatter.html) — an ordered mapping with raw access (`get`, `set`, `as_mapping`, `as_mapping_mut`, `into_mapping`) plus typed accessors for every OKF field IRE cares about: `type_()`, `title()`, `description()`, `resource()`, `tags()`, `sources() -> Vec<Source>`, `generated() -> Option<Generated>`, `verified() -> Vec<Verification>`, `latest_verification()`, `trust_tier()`, `status()`, `stale_after()`, `is_stale_on()`, `content_changed_at()`.
- Domain types already modeled: `Source`, `Generated`, `Verification`, `Actor`, `Date`, `DateTime`, `Status`, `TrustTier`, `ConceptId`, `Bundle`, `Link`, `Attribution`, `Citation`.
- Constants: `REQUIRED_FRONTMATTER_KEYS`, `RECOMMENDED_FRONTMATTER_KEYS`, `KNOWN_FRONTMATTER_KEYS`, `LEGACY_FRONTMATTER_KEYS`, `PREFERRED_KEY_ORDER` ([frontmatter module docs](https://docs.rs/okf/latest/okf/frontmatter/index.html)).
- Bundle-level: `validate_bundle()`, `validate_bundle_at(&bundle, today)`, `lint_bundle()`, `bundle_diff()`, cross-link graph and backlinks.

The [frontmatter module docs](https://docs.rs/okf/latest/okf/frontmatter/index.html) state the design directly:

> "OKF frontmatter is an open mapping: a few well-known keys (§4.1 of the spec) plus arbitrary producer-defined extensions that consumers MUST preserve when round-tripping. `Frontmatter` therefore stores the full `Mapping` verbatim and layers typed accessors on top, rather than deserializing into a fixed struct that would drop unknown keys."

That is precisely the OKF §4.1 requirement in the brief, already implemented.

The [`yaml` module](https://docs.rs/okf/latest/okf/yaml/index.html) is a dependency-free YAML *subset*: block mappings including nested/indented blocks, block sequences, flow collections (`[a, b]`, `{a: 1}`), plain/single/double-quoted scalars, `|` and `>` block scalars, `#` comments and blank lines, and the core scalar types. It deliberately rejects anchors/aliases, explicit tags, multiple documents, and complex mapping keys with a clear `YamlError` rather than misbehaving. Its stated guarantee: `parse(emit(parse(x))) == parse(x)` — "Emitting and re-parsing preserves the logical value and key order."

One nice detail for IRE's timestamps: the module keeps every date/datetime-shaped scalar as a **string**, and emits datetime-valued scalars quoted, because "a bare ISO datetime is not stable even under the reference's own round-trip: PyYAML loads it into a `datetime` and dumps it back as `2026-06-30 14:00:00+00:00`, losing the `T` and `Z` separators §5.2 asks for." A bare `YYYY-MM-DD` stays plain.

### The one gap: `serialize()` is not byte-for-byte on the body

From the [source of `Document::serialize`](https://docs.rs/okf/latest/src/okf/document.rs.html):

```rust
/// `parse` followed by `serialize` preserves frontmatter key order and the
/// body (modulo trailing-newline normalization), matching the reference.
/// Flow collections are re-emitted in block style, which is the same value
/// written differently.
pub fn serialize(&self) -> String {
    let fm_text = Value::Mapping(self.frontmatter.as_mapping().clone())
        .to_yaml_string().trim_end().to_string();
    let body = if self.body.ends_with('\n') { self.body.clone() }
               else { format!("{}\n", self.body) };
    format!("{FRONTMATTER_DELIM}\n{fm_text}\n{FRONTMATTER_DELIM}\n\n{body}")
}
```

Three normalizations to be aware of, all small and all easy to work around:

1. It forces exactly one blank line between the closing `---` and the body.
2. It appends a trailing newline if the body lacks one.
3. Comments inside the frontmatter block are parsed but **not** re-emitted (`Value` has no comment slot), and flow collections become block style.

If IRE needs the body byte-identical, do not call `serialize()`. Use `Document::parse` / `Frontmatter` for reading and editing, then emit only the frontmatter block and splice it into the original text at the delimiters — IRE's existing `replace()` in `src-tauri/src/ire/frontmatter.rs` already does that splice. That keeps the body untouched by construction and the whole normalization question disappears.

`Frontmatter::reorder_preferred()` is available when IRE *wants* canonical ordering (a port of the reference's `_reorder_frontmatter`); the docs note "No key is added, dropped, or rewritten, so only the serialized order changes." Do not call it on every write, or existing files churn once.

---

## 2. The general YAML / frontmatter field

All figures from the crates.io API on 2026-08-21.

### YAML engines

| Crate | Version | Latest release | All-time dl | Recent dl | License | Status |
|---|---|---|---|---|---|---|
| [`serde_yaml`](https://crates.io/crates/serde_yaml) | `0.9.34+deprecated` | 2024-03-25 | 376.4M | 88.2M | MIT OR Apache-2.0 | **Archived** |
| [`yaml-rust2`](https://crates.io/crates/yaml-rust2) | 0.12.0 | 2026-08-18 | 52.5M | 14.1M | MIT OR Apache-2.0 | Active |
| [`saphyr`](https://crates.io/crates/saphyr) | 0.0.12 | 2026-08-18 | 1.9M | 726k | MIT OR Apache-2.0 | Active |
| [`saphyr-parser`](https://crates.io/crates/saphyr-parser) | 0.0.12 | 2026-08-18 | 3.3M | 1.8M | MIT OR Apache-2.0 | Active |
| [`serde_norway`](https://crates.io/crates/serde_norway) | 0.9.42 | 2024-12-21 | 9.7M | 2.9M | MIT OR Apache-2.0 | Maintained fork |
| [`serde_yaml_ng`](https://crates.io/crates/serde_yaml_ng) | 0.10.0 | 2024-05-26 | 9.1M | 5.1M | MIT | Maintained fork |
| [`yaml-rust`](https://crates.io/crates/yaml-rust) | 0.4.5 | 2021-01-03 | 171.4M | 34.8M | MIT/Apache-2.0 | Unmaintained |

**`serde_yaml` is confirmed dead.** [github.com/dtolnay/serde-yaml](https://github.com/dtolnay/serde-yaml) reports `"archived": true` with `"pushed_at": "2024-03-25T00:50:35Z"` ([GitHub API](https://api.github.com/repos/dtolnay/serde-yaml)) — archived on/around **2024-03-25**, the same day as the final release. The README says plainly: *"Rust library for using the Serde serialization framework with data in YAML file format. (This project is no longer maintained.)"* The published version string is literally `0.9.34+deprecated`. Its 88M recent downloads are inertia, not health. **Do not add it to IRE.**

- **`serde_yaml_ng`** — [acatton/serde-yaml-ng](https://github.com/acatton/serde-yaml-ng), 113 stars, last push 2025-09-14. A minimal-change continuation fork.
- **`serde_norway`** — the repo redirects to [cafkafk/serde-norway](https://api.github.com/repos/cafkafk/serde-norway), 56 stars, last push 2025-08-04. The other continuation fork; used by the Nix ecosystem.
- **`yaml-rust2`** — [Ethiraric/yaml-rust2](https://github.com/Ethiraric/yaml-rust2), "A pure Rust YAML implementation", 261 stars, pushed 2026-08-18, actively released. This is the live successor to the ubiquitous but dead `yaml-rust`.
- **`saphyr`** — [saphyr-rs/saphyr](https://github.com/saphyr-rs/saphyr), "YAML 1.2 implementation in pure Rust", same release cadence as yaml-rust2 (both 0.0.12/0.12.0 on 2026-08-18). Same maintainer lineage. Still `0.0.x`, so its API is explicitly unstable.

All of the above are pure Rust with no system dependencies (no libyaml C shim).

### Frontmatter wrappers

| Crate | Version | Latest release | All-time dl | Recent dl | License | Non-dev deps |
|---|---|---|---|---|---|---|
| [`gray_matter`](https://crates.io/crates/gray_matter) | 0.3.2 | 2025-07-10 | 723k | 145k | MIT | serde, thiserror, serde_json, toml 0.9, yaml-rust2 0.10 |
| [`markdown-frontmatter`](https://crates.io/crates/markdown-frontmatter) | 0.5.1 | 2026-03-03 | 20.3k | 3.0k | MIT | thiserror, serde, serde_json, **serde_yaml 0.9.34**, toml |
| [`frontmatter-gen`](https://crates.io/crates/frontmatter-gen) | 0.0.6 | 2026-06-21 | 23.4k | 6.3k | MIT OR Apache-2.0 | 20 non-dev deps incl. tokio, clap, tera, pulldown-cmark, uuid, url |
| [`matter`](https://crates.io/crates/matter) | 0.1.0-alpha4 | 2020-07-16 | 187.6k | 8.1k | BSD-3-Clause | lazy_static, regex |
| [`serde-frontmatter`](https://crates.io/crates/serde-frontmatter) | 0.1.0 | 2021-07-17 | 4.8k | 199 | **GPL-3.0** | — |
| [`extract-frontmatter`](https://crates.io/crates/extract-frontmatter) | 4.1.1 | 2022-04-22 | 30.3k | — | — | — |
| [`edikt-frontmatter`](https://crates.io/crates/edikt-frontmatter) | 0.1.0 | 2026-07-18 | 58 | 58 | MIT OR Apache-2.0 | edikt-core, edikt-jsonc, edikt-toml, edikt-yaml, thiserror |
| [`pulldown-cmark-frontmatter`](https://crates.io/crates/pulldown-cmark-frontmatter) | 0.4.0 | 2024-09-16 | 12.1k | — | — | — |
| [`markdown-it`](https://crates.io/crates/markdown-it) | 0.6.1 | 2024-07-07 | 271.7k | 99.1k | MIT | — |

Also visible on a crates.io `frontmatter` search and worth knowing about, though none is a fit for a library dependency: `hyalo-core`/`hyalo-cli`, `mif-frontmatter` ("Markdown frontmatter <-> JSON-LD lossless round-trip"), `diaryx_core`, `vaultdb-core`, `mdql-core`, `knap`, `mdatron`, `mdvs`, `metadata-gen`, `issuectl`, `toml-frontmatter`, `mdbook-frontmatter-strip`.

Disqualifications, briefly:

- `markdown-frontmatter` depends on archived `serde_yaml` — adopting it re-imports the dead crate.
- `frontmatter-gen` drags in ~20 dependencies including tokio, clap, tera and pulldown-cmark. Unacceptable weight inside a Tauri backend for a frontmatter parser, and it is still `0.0.6`.
- `matter` is a 2020 alpha; `serde-frontmatter` is GPL-3.0 (license-incompatible with shipping IRE as-is) and effectively dead at 199 recent downloads.
- `markdown-it` is a markdown renderer, not a frontmatter store. Frontmatter is a plugin concern; it would not help IRE write anything back.
- `pulldown-cmark-frontmatter` extracts only, and stalled in 2024.

---

## 3. Per-candidate answers to the specific questions

### `okf` 0.2.1

- **Nested maps/lists?** Yes. Block mappings *including nested/indented blocks*, block sequences, and flow collections, per the [`yaml` module docs](https://docs.rs/okf/latest/okf/yaml/index.html). `generated: { by, at }` and `sources: [{...}]` are first-class, with typed accessors (`Generated`, `Vec<Source>`, `Vec<Verification>`).
- **Serialize back?** Yes — `Document::serialize()` and an internal `Value::to_yaml_string()`.
- **Unknown keys round-trip?** Yes, by design — `Frontmatter` keeps the full `Mapping` verbatim and never deserializes into a fixed struct.
- **Key order?** Preserved. The module's stated invariant is `parse(emit(parse(x))) == parse(x)` including key order, and `Frontmatter::set` "preserv[es] position if it already exists". Reordering is opt-in via `reorder_preferred()`.
- **Comments?** Parsed, not preserved on emit.
- **Body separate?** Yes — `Document.body: String` is a public field, `Document.frontmatter` the other.
- **Cost?** Zero dependencies, std-only. Essentially free.

### `yaml-rust2` 0.12.0

- **Nested?** Yes, full YAML 1.2.
- **Serialize?** Yes — `YamlEmitter` is re-exported at the crate root; the crate-level docs' first example is parse-then-`emitter.dump()`.
- **Unknown keys?** Trivially, since `Yaml` is an untyped tree — nothing is "unknown".
- **Key order?** **Preserved.** [`yaml_rust2::yaml::Hash`](https://docs.rs/yaml-rust2/latest/yaml_rust2/yaml/type.Hash.html) is `pub type Hash = LinkedHashMap<Yaml, Yaml>;` — insertion-ordered. This is the single most important reason to prefer it over any serde-mapping approach.
- **Comments?** Not preserved by the emitter.
- **Body separate?** No — it is a YAML engine only. IRE supplies the `---` splitting (it already has it).
- **Cost?** Pure Rust, no system deps. MSRV 1.65.0 with all features off; `encoding` is on by default.

### `saphyr` / `saphyr-parser` 0.0.12

- Same lineage and same shape as yaml-rust2: `Yaml::load_from_str` plus `YamlEmitter`. Adds `MarkedYaml` / `MarkedYamlOwned`, which carry `Marker`s for the **beginning and end of each node in the input** — the raw material you would need to build a true format-preserving editor, since spans let you rewrite only the bytes of one value.
- Still `0.0.x`. Don't build IRE on an explicitly-unstable API for this.

### `gray_matter` 0.3.2

- **Nested?** Yes for parsing — YAML/TOML/JSON engines, YAML backed by yaml-rust2.
- **Serialize?** **No.** It is an extractor. There is no emit path back to a document.
- **Key order?** **Lost.** [`Pod::Hash(HashMap<String, Pod>)`](https://docs.rs/gray_matter/latest/gray_matter/enum.Pod.html) — an unordered `std::collections::HashMap`. Every write would reshuffle keys and produce exactly the noisy git diffs the brief wants to avoid.
- **Body separate?** Yes — `ParsedEntity { content, excerpt, data }`, and the excerpt feature is genuinely nice.
- **Verdict:** good if IRE only ever *reads*. Wrong crate the moment it writes.

### `serde_yaml` / `serde_yaml_ng` / `serde_norway` (as a class)

- Nested: yes. Serialize: yes. Unknown keys: only via `#[serde(flatten)] HashMap` or an untyped `Value`.
- **Key order:** the serde `Mapping` types are insertion-ordered in the `serde_yaml` lineage, but round-tripping through `#[serde(flatten)]` into a `HashMap` is not, and struct field order is fixed at compile time regardless of what the file said. In practice this class reorders IRE's files unless everything goes through an untyped `Value`, at which point serde is buying nothing over yaml-rust2.
- Comments: never preserved.
- `serde_yaml` itself is archived; the two forks are alive but slow (last releases 2024-05 and 2024-12).

---

## 4. Format-preserving YAML editing (the ruamel.yaml round-trip equivalent)

The honest general answer is that **no widely-adopted Rust crate does comment-and-formatting-preserving YAML editing** the way `toml_edit` does for TOML. There is no `yaml_edit` with meaningful adoption. Every mainstream engine above (`yaml-rust2`, `saphyr`, the `serde_yaml` family) parses to a value tree that has no slot for comments, so emitting always reflows.

Two things come close and are worth naming:

- **[`edikt`](https://github.com/jhheider/edikt)** is a genuine attempt at exactly this. Its README leads with "Edit config files without reflowing them" and describes "A lossless, format-preserving editor for JSONC/JSON5, TOML, YAML, INI, KDL, and flat key-value files, driven by a jq-flavored expression language". It advertises `edikt -i '.services.web.replicas = 3' compose.yaml` with "anchors, flow style, and comments survive", and — directly on point — a Markdown frontmatter mode: *"Markdown frontmatter — edit the metadata block, the prose untouched"*, exposed as the [`edikt-frontmatter`](https://crates.io/crates/edikt-frontmatter) crate (0.1.0, 2026-07-18, MIT OR Apache-2.0). It is the closest thing to ruamel round-trip mode in Rust. But it is **58 downloads old**, splits across five crates (`edikt-core`, `edikt-yaml`, `edikt-jsonc`, `edikt-toml`, `thiserror`), and its edit language is a jq-flavored string DSL rather than a typed API — an odd fit for IRE writing a known set of fields.
- **`saphyr`'s `MarkedYaml`** gives you input spans, which is the primitive you would build such an editor on. It is not an editor itself.

**For IRE this mostly does not matter.** IRE owns the files it writes (`EXPERIMENT.md` and friends are generated, not hand-maintained config with load-bearing comments), and the body — the part a human actually hand-writes — is preserved by splicing, not by the YAML layer. Losing comments *inside the frontmatter block* costs nothing IRE has.

---

## RECOMMENDATION

**Adopt `okf`, and keep IRE's own splice-on-write.**

Concretely:

1. Add `okf = "0.2"` to `src-tauri/Cargo.toml`. Zero transitive dependencies, so compile time and binary size barely move — cheaper than adding `yaml-rust2`, and far cheaper than `gray_matter` (which pulls yaml-rust2, toml 0.9, serde_json and thiserror anyway).
2. Replace the read path in `src-tauri/src/ire/frontmatter.rs` (currently `parse() -> (Option<HashMap<String,String>>, &str)`, 112 lines) with `okf::Document::parse`. The `HashMap<String,String>` return type is the thing that has to go: it cannot represent `sources` or `generated` at all, and it is unordered, so it could not preserve field order on write even if the values fit.
3. Keep `replace()`. Do not call `Document::serialize()` — it normalizes the blank line after `---` and the trailing newline, and re-emits flow collections as block style. Instead, mutate `Frontmatter` (`set`, `as_mapping_mut`), emit just the frontmatter block, and splice it between the existing delimiters. The body then stays byte-for-byte identical by construction, which is a stronger guarantee than any crate can offer you.
4. Do **not** call `reorder_preferred()` on routine writes; it would rewrite every existing file once. Reserve it for a deliberate one-shot normalization if IRE ever wants canonical OKF ordering.

Why this over the alternatives:

- The immediate need (scalar fields in `EXPERIMENT.md`) is satisfied by anything, including the status quo. The decision should be made on the near-future need, and there `okf` is not just adequate but pre-built: `sources`, `verified`, `generated`, `Actor`, `TrustTier`, `stale_after` and `status` are already modeled, spec-referenced section by section, and tested against Google's four published bundles. Choosing a generic YAML crate means hand-writing all of that, plus the §4.1 unknown-key preservation rule, plus the quoted-timestamp rule that keeps `T`/`Z` from being eaten.
- It gets the two properties that are hard to retrofit: **insertion-ordered mappings** (no noisy git diffs) and **verbatim retention of producer-defined keys** (OKF §4.1 compliance for free).
- Alignment matters more than the crate. If IRE is committing to OKF for Claim and Resource concepts, tracking a library that ports the reference implementation keeps IRE honest about conformance as the spec moves; a bespoke parser will drift.

Risks and the exit:

- **Youth.** 414 downloads, 18 stars, one maintainer, ~2 months old. Accept this only because it is Apache-2.0 with zero dependencies — the escape hatch is to vendor the two modules IRE uses (`yaml`, `document`/`frontmatter`) into `src-tauri/src/ire/` and keep going. That is a far cheaper fallback than being stuck under an abandoned crate with a dependency tree.
- **YAML subset, not YAML 1.2.** No anchors, aliases, explicit tags, or multi-document streams. For OKF frontmatter that is a feature (it errors clearly rather than misbehaving), but if IRE ever needs to read arbitrary third-party YAML, this crate is not the tool.
- **Pin loosely and read the changelog.** It is at `0.2.x` tracking spec v0.2; a spec bump will move the API.

If `okf`'s youth is judged unacceptable: fall back to **`yaml-rust2` 0.12** (52M downloads, released three days ago, `LinkedHashMap`-backed so order survives, `YamlEmitter` for the write path) and model the OKF field families as IRE's own types on top. That is strictly more work for strictly less spec fidelity, but it is the safe choice and it is a real one. It is not `serde_yaml`, which has been archived since 2024-03-25 and ships as `0.9.34+deprecated`.

---

## Sources

- OKF spec: [GoogleCloudPlatform/knowledge-catalog `okf/SPEC.md`](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md) · [repo languages](https://api.github.com/repos/GoogleCloudPlatform/knowledge-catalog/languages)
- `okf`: [crates.io](https://crates.io/crates/okf) · [docs.rs](https://docs.rs/okf/latest/okf/) · [`Document`](https://docs.rs/okf/latest/okf/struct.Document.html) · [`Frontmatter`](https://docs.rs/okf/latest/okf/struct.Frontmatter.html) · [`frontmatter` module](https://docs.rs/okf/latest/okf/frontmatter/index.html) · [`yaml` module](https://docs.rs/okf/latest/okf/yaml/index.html) · [`document.rs` source](https://docs.rs/okf/latest/src/okf/document.rs.html) · [GitHub](https://github.com/W4G1/okf)
- `serde_yaml`: [crates.io](https://crates.io/crates/serde_yaml) · [archived repo](https://github.com/dtolnay/serde-yaml) · [GitHub API showing `archived: true`, `pushed_at: 2024-03-25`](https://api.github.com/repos/dtolnay/serde-yaml)
- `serde_yaml_ng`: [crates.io](https://crates.io/crates/serde_yaml_ng) · [GitHub](https://github.com/acatton/serde-yaml-ng)
- `serde_norway`: [crates.io](https://crates.io/crates/serde_norway) · [GitHub](https://github.com/cafkafk/serde-norway)
- `yaml-rust2`: [crates.io](https://crates.io/crates/yaml-rust2) · [docs.rs](https://docs.rs/yaml-rust2/latest/yaml_rust2/) · [`Hash` type alias](https://docs.rs/yaml-rust2/latest/yaml_rust2/yaml/type.Hash.html) · [GitHub](https://github.com/Ethiraric/yaml-rust2)
- `saphyr`: [crates.io](https://crates.io/crates/saphyr) · [docs.rs](https://docs.rs/saphyr/latest/saphyr/) · [GitHub](https://github.com/saphyr-rs/saphyr)
- `gray_matter`: [crates.io](https://crates.io/crates/gray_matter) · [docs.rs](https://docs.rs/gray_matter/latest/gray_matter/) · [`Pod`](https://docs.rs/gray_matter/latest/gray_matter/enum.Pod.html) · [GitHub](https://github.com/yuchanns/gray-matter-rs)
- `edikt` / `edikt-frontmatter`: [GitHub](https://github.com/jhheider/edikt) · [crates.io](https://crates.io/crates/edikt-frontmatter)
- Others: [`frontmatter-gen`](https://crates.io/crates/frontmatter-gen) · [`markdown-frontmatter`](https://crates.io/crates/markdown-frontmatter) · [`matter`](https://crates.io/crates/matter) · [`serde-frontmatter`](https://crates.io/crates/serde-frontmatter) · [`markdown-it`](https://crates.io/crates/markdown-it) · [`okf-permissive`](https://crates.io/crates/okf-permissive)
