# Implementation Blueprint: Thread Safety and Locking for the Vault of Markdown Files

## 1. Technical Stack & Requirements

- **Core Dependencies:**
  - `std::sync::{Mutex, Arc}` — Rust stdlib, for in-process state guards
  - `notify` (crate) — filesystem watcher (`RecommendedWatcher`)
  - `tempfile` (crate) — `NamedTempFile` + `persist_noclobber()` for atomic rename-on-create
  - `uuid` (crate) — unique temp file names during cache writes
  - `serde_json` — transaction manifests serialized to `.json` on disk

- **Environmental Prerequisites:** None beyond a writable filesystem; the lock file path is derived automatically from the cache path.

---

## 2. Architecture Map

- **Vault entry point (Tauri commands):** `src-tauri/src/lib.rs`
- **File I/O primitives:** `src-tauri/src/vault/file.rs`
- **Cache layer (advisory lock + CAS):** `src-tauri/src/vault/cache.rs`
- **Rename transactions (crash-safe):** `src-tauri/src/vault/rename_transaction.rs` · `src-tauri/src/vault/rename.rs`
- **File watcher:** `src-tauri/src/vault_watcher.rs`
- **Window / process state guards:** `src-tauri/src/window_state.rs` · `src-tauri/src/telemetry.rs`

**Data Model:**
- `VaultEntry` — immutable snapshot of file metadata; passed by value between threads
- `VaultCache` — serialized to JSON at a fixed path derived from the vault root
- `CacheFileFingerprint` — hash/mtime used for CAS comparison on write
- `RenameTransaction` — JSON manifest written before any file move; drives crash recovery

---

## 3. Step-by-Step Reproduction

### Layer 1 — In-process state (Tauri `manage`)

All long-lived mutable state is wrapped in `Mutex<T>` and registered with `.manage()`:

```rust
// lib.rs:43-67 — example pattern
struct WsBridgeChild(Mutex<Option<Child>>);
struct VaultWatcherState { active: Mutex<Option<ActiveVaultWatcher>> }

app_builder
    .manage(WsBridgeChild(Mutex::new(None)))
    .manage(VaultWatcherState::new());
```

Tauri injects these into command handlers via `State<T>`. Lock is held only for the duration of the mutation and released before any `await` or heavy I/O.

### Layer 2 — Advisory file lock for cache writes

The vault cache (`<vault>/.tolaria-cache/vault.json`) is shared between processes (e.g., two app windows, or a background startup task and a foreground scan). An advisory lock file (`vault.json.lock`) serializes writers:

```
cache.rs:305-319  try_create_cache_write_lock:
  OpenOptions::new().write(true).create_new(true).open(lock_path)
  // create_new=true is atomic on all target OSes — exactly one writer wins
```

The lock file stores the writer's PID. A 30-second staleness check (`CACHE_WRITE_LOCK_STALE_SECS`) lets a new writer reclaim a lock left by a crashed process (`cache.rs:321-336`).

`CacheWriteLock` implements `Drop` to delete the lock file, so it is released even on panic.

### Layer 3 — Compare-and-swap fingerprint on cache write

Even after acquiring the file lock, the writer re-reads the cache fingerprint and compares it to the fingerprint it loaded earlier. If another writer snuck in between load and write, the update is silently skipped:

```rust
// cache.rs:387-398
let current_fingerprint = read_cache_fingerprint(&final_path)?;
if current_fingerprint.as_ref() != expected_previous.as_ref() {
    return Ok(CacheWriteOutcome::SkippedConcurrentUpdate);
}
```

This is a CAS (compare-and-swap) pattern at the file level. Three outcomes are modelled in `CacheWriteOutcome`: `Replaced`, `SkippedConcurrentUpdate`, `SkippedActiveWriter`.

### Layer 4 — Atomic cache file replacement (temp + rename)

The new cache JSON is never written directly to the final path. Instead:

1. A UUID-suffixed `.tmp` file is created with `create_new(true)` in the same directory.
2. Content is written and `sync_all()` is called.
3. `fs::rename(tmp, final)` replaces the cache atomically.
4. On Unix, the parent directory is also `sync_all()`'d to flush the directory entry.

```rust
// cache.rs:409-445 (pseudocode)
let tmp = cache_temp_path(&final_path);           // e.g. vault.json.<uuid>.tmp
let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
f.write_all(&data)?;
f.sync_all()?;
drop(f);
fs::rename(&tmp, &final_path)?;
sync_parent_directory(&final_path)?;              // Unix only
```

Readers either see the old file or the fully-written new file — never a half-written state.

### Layer 5 — Transactional renames with crash recovery

Note renames are the most dangerous operation: they must atomically move or copy a markdown file and update all wikilink back-references. The mechanism is a mini write-ahead log:

1. **Stage:** content is written to a `NamedTempFile` inside `.tolaria-rename-txn/` with `sync_all()`.
2. **Manifest:** a JSON file (`<uuid>.json`) is written recording `old_path`, `new_path`, and `backup_path` — this is the WAL entry.
3. **Commit:** `persist_noclobber()` atomically moves the staged file to the target path (fails rather than clobbering).
4. **Cleanup:** manifest is deleted on success.

On next vault scan (`recover_pending_rename_transactions`), any leftover manifest files are replayed. Recovery logic (`rename_transaction.rs:255-287`) uses three-way state to decide whether to roll forward, roll back, or no-op:

| `backup_path` exists | `new_path` or `old_path` exists | Action |
|---|---|---|
| No | — | Already complete → delete manifest |
| Yes | Yes | Partial → delete backup + manifest |
| Yes | No | Interrupted mid-move → restore from backup |

### Layer 6 — File watcher noise filtering

The `notify` watcher emits events for every filesystem change. Before forwarding to the frontend, events are filtered:

- Paths inside `.git/`, `node_modules/` are dropped.
- Temp file names (`.DS_Store`, `*.tmp`, `*.swp`, `~`-suffixed, `.tolaria-rename-txn`) are dropped (`vault_watcher.rs:15-43`).
- `Access` events (reads) are dropped — only mutation events propagate.

This prevents the cache invalidation loop from triggering on its own temp files or on the advisory lock file.

---

## 4. Critical Logic Constraints

- **`create_new(true)` is the atomicity primitive.** Both the advisory lock and the temp-file creation use this flag. On Linux and macOS it maps to `O_CREAT | O_EXCL`, which is atomic in the kernel. Never replace with `create(true)` — that silently races.

- **The advisory lock is process-level, not thread-level.** Two threads in the same process both win `create_new(true)` if they race, because the lock file already exists from the first thread's creation. If intra-process concurrency of cache writes is ever introduced, an in-process `Mutex` must be added in front of the file lock.

- **Stale lock detection is time-based, not PID-based.** The lock file stores the writer PID but stale detection only checks mtime. If the OS reuses PIDs rapidly, a live lock could be reclaimed. The 30-second window makes this safe in practice but it is not a hard guarantee.

- **`sync_all()` on the temp file before rename.** Without this, a crash after `rename()` could leave the new cache file with unflushed kernel buffers. The parent directory sync on Unix ensures the directory entry survives a power loss.

- **The fingerprint CAS does not use `fsync` between read and write.** Under heavy concurrent load, TOCTOU between `read_cache_fingerprint` and the `rename` could theoretically let two writers both believe they are the winner. The advisory lock makes this a non-issue in practice — but the fingerprint check is a safety net for the case where the lock is reclaimed from a stale (but still alive) writer.

- **Temp files for renames live inside the vault directory** (`.tolaria-rename-txn/`). This ensures they are on the same filesystem as the destination, making `fs::rename()` a true atomic syscall rather than a cross-device copy. Never stage to a system temp dir.

- **The watcher ignores `.tolaria-rename-txn`** explicitly in `is_temp_file_name`. Without this, staging a file for rename would trigger a cache invalidation, causing a redundant rescan mid-transaction.
