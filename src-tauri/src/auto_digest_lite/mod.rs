// SPDX-License-Identifier: MIT
//! Auto-digest engine for RSS feeds and local file folders.  Tier-2 of
//! Feature 3 (Global Digest).  Replaces the Phase-1 stub with a real
//! pipeline: `notify v8` (cross-platform debounced filesystem events),
//! `feed-rs` (RSS / Atom / JSON-Feed parser), and `reqwest` (HTTPS pull
//! with conditional GET).
//!
//! Storage and chunking are delegated to `knowledge_lite` so digest
//! ingest shares the dual-table FTS5 / RRF infrastructure documented
//! in Feature 2 (Tier 2).
//!
//! ## Recall anti-pattern defence
//!
//!   1. **Off by default** — every source is opt-in via Tauri command.
//!   2. **AES-GCM at rest** — files persist into the encrypted SQLite
//!      knowledge DB shared with `knowledge_lite`.
//!   3. **PII scrubbed PRE-LLM** — feed entry text routes through
//!      `pii_scrubber::scrub` before insertion (see [`scrub_for_ingest`]).
//!   4. **Provenance** — every entry carries the source URL or path in
//!      the `documents.path` column, plus the `source_id` of the digest
//!      source that produced it.
//!   5. **Single-key pause** — `digest_pause` flips a global flag the
//!      daemon checks every 250 ms; `digest_resume` clears it.
//!   6. **Quiet hours** — 22:00-08:00 local time defaults are exposed
//!      via [`is_quiet_hours`]; daemon skips network calls in that
//!      window unless the user disables it.
//!   7. **Per-folder + per-app gating** — each Folder source carries
//!      its own `allow_ext` filter so the user can restrict which
//!      extensions get ingested.
//!   8. **Zero telemetry** — nothing leaves the process beyond the
//!      RSS HTTP fetches the user explicitly added.
//!
//! ## Storage layout
//!
//! State persists to `$IMPFORGE_APP_HOME/digest-sources.json`, history
//! to `$IMPFORGE_APP_HOME/digest-history.jsonl`, and feed entry caches
//! land in `$IMPFORGE_APP_HOME/digest-cache/`.
//!
//! ## Crown-Jewel directory layout (Phase-2 split)
//!
//! - [`types`]      — newtypes, enums, `DigestRuntime`, constants
//! - [`helpers`]    — paths, timestamps, IDs, quiet-hours, PII helper
//! - [`state`]      — load/save, history, add/remove, stats
//! - [`feed`]       — RSS pull/parse/persist (the only network code)
//! - [`watcher`]    — folder once-pass + filesystem watcher + drain
//! - [`scheduler`]  — `run_once` + global runtime + test env lock
//! - [`commands`]   — 9 Tauri commands wired into `lib.rs`
//! - [`tests`]      — behavioural tests (cfg(test) only)

pub mod commands;
pub mod feed;
pub mod helpers;
pub mod scheduler;
pub mod state;
pub mod types;
pub mod watcher;

#[cfg(test)]
mod tests;

// ─── Public surface re-exports (backward-compat with pre-split paths) ────
//
// External callers (digest_browser, digest_clipboard, digest_screenshots,
// etc.) `use crate::auto_digest_lite::{Foo, bar}` — every symbol they
// used before the split must still resolve through `crate::auto_digest_lite`.
// This block is the explicit contract — adding a new sub-module symbol
// does NOT automatically re-export it; you must list it here too.

// Tauri commands (registered in `lib.rs`).
pub use commands::{
    digest_add_source, digest_history, digest_list_sources, digest_pause,
    digest_remove_source, digest_resume, digest_run_once,
    digest_set_quiet_hours, digest_stats,
};

// Feed pipeline.
pub use feed::{entry_to_text, parse_feed, persist_feed_entry, pull_feed, strip_html};

// Helpers used by sibling digest_* modules.
pub use helpers::{digest_home, is_quiet_hours, scrub_for_ingest};

// Persistence + state mutators.
pub use state::{
    add_clipboard, add_feed, add_folder, add_screenshots, append_history,
    compute_stats, load_state, remove_source, save_state, tail_history,
};

// Scheduler + runtime.
pub use scheduler::{run_once, runtime};
#[cfg(test)]
pub use scheduler::TEST_ENV_LOCK;

// Type surface — everyone uses these names without the `types::` prefix.
pub use types::{
    DigestEntry, DigestRuntime, DigestSource, DigestState, DigestStats,
    FeedPullOutcome, FolderWatchEvent, FolderWatchHandle,
    FolderWatcherSetup, RunOnceSummary, WatcherType, DEBOUNCER_TIMEOUT,
    DEFAULT_POLL_SECS, DEFAULT_QUIET_HOURS, FEED_USER_AGENT,
    MAX_FEED_BYTES, MIN_POLL_SECS,
};

// Watcher functions — used by `digest_screenshots` and tests.
pub use watcher::{drain_watcher, handle_fs_events, ingest_folder_once, install_folder_watchers};
