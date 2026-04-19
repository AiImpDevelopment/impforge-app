// SPDX-License-Identifier: MIT
//! Type-only surface of `auto_digest_lite` — newtypes, enums,
//! constants, and runtime structs.  No I/O, no Tauri commands; safe
//! to depend on from any sibling sub-module.

use crate::error::{AppError, AppResult};
use notify_debouncer_full::{
    notify::{RecommendedWatcher},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::time::Duration;

/// Default RSS poll interval (seconds).
pub const DEFAULT_POLL_SECS: u64 = 300;
/// Minimum allowed poll interval — never poll faster than once a minute.
pub const MIN_POLL_SECS: u64 = 60;
/// Hard ceiling on bytes the App will fetch from any single feed.
pub const MAX_FEED_BYTES: usize = 8 * 1024 * 1024;
/// Debouncer batch window — see notify-debouncer-full README.
pub const DEBOUNCER_TIMEOUT: Duration = Duration::from_millis(750);
/// Default quiet-hours window in **local** time (start, end) hours.
/// Both are inclusive on `start`, exclusive on `end`; if `start > end`
/// the window crosses midnight.
pub const DEFAULT_QUIET_HOURS: (u32, u32) = (22, 8);
/// User-Agent for every outbound RSS fetch.
pub const FEED_USER_AGENT: &str = concat!(
    "ImpForge-App/",
    env!("CARGO_PKG_VERSION"),
    " (+https://impforge.com)"
);

// ─── Source / state types ────────────────────────────────────────────────

/// One configured digest source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DigestSource {
    /// HTTP(S) feed — RSS / Atom / JSON Feed.  Conditional GET via
    /// `last_modified` / `etag` so unchanged feeds don't re-download.
    Feed {
        id: String,
        url: String,
        interval_secs: u64,
        #[serde(default)]
        last_modified: Option<String>,
        #[serde(default)]
        etag: Option<String>,
        #[serde(default)]
        last_polled_unix: u64,
    },
    /// Local folder.  `recursive` controls subdirectory traversal,
    /// `allow_ext` is the per-folder extension allow-list.
    Folder {
        id: String,
        path: PathBuf,
        recursive: bool,
        #[serde(default)]
        allow_ext: Vec<String>,
    },
    /// Opt-in clipboard monitor.  Only one Clipboard source is allowed
    /// in the source list at a time — second registration replaces.
    /// Actual polling lives in `digest_clipboard.rs`.
    Clipboard { id: String },
    /// Opt-in screenshot folder watcher with OCR.  Defaults to the OS's
    /// canonical screenshot folder when `path` is None.  Actual logic
    /// lives in `digest_screenshots.rs`.
    Screenshots {
        id: String,
        #[serde(default)]
        path: Option<PathBuf>,
    },
    /// Opt-in browser bookmark/history import.  Lives in
    /// `digest_browser.rs` — requires explicit user-action invoke.
    Browser {
        id: String,
        /// `firefox`, `chrome`, `brave`, `edge`, `opera`.
        family: String,
    },
}

impl DigestSource {
    pub fn id(&self) -> &str {
        match self {
            DigestSource::Feed { id, .. }
            | DigestSource::Folder { id, .. }
            | DigestSource::Clipboard { id }
            | DigestSource::Screenshots { id, .. }
            | DigestSource::Browser { id, .. } => id,
        }
    }

    pub fn label(&self) -> String {
        match self {
            DigestSource::Feed { url, .. } => format!("feed: {url}"),
            DigestSource::Folder { path, recursive, .. } => format!(
                "folder: {} ({})",
                path.display(),
                if *recursive { "recursive" } else { "flat" }
            ),
            DigestSource::Clipboard { .. } => "clipboard (opt-in)".into(),
            DigestSource::Screenshots { path, .. } => match path {
                Some(p) => format!("screenshots: {}", p.display()),
                None => "screenshots: <OS default>".into(),
            },
            DigestSource::Browser { family, .. } => format!("browser: {family}"),
        }
    }
}

/// Persisted state — sources + pause flag + quiet-hours config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DigestState {
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub quiet_hours_start: u32,
    #[serde(default)]
    pub quiet_hours_end: u32,
    #[serde(default)]
    pub quiet_hours_enabled: bool,
    #[serde(default)]
    pub sources: Vec<DigestSource>,
}

impl Default for DigestState {
    fn default() -> Self {
        Self {
            paused: false,
            quiet_hours_start: DEFAULT_QUIET_HOURS.0,
            quiet_hours_end: DEFAULT_QUIET_HOURS.1,
            quiet_hours_enabled: true,
            sources: Vec::new(),
        }
    }
}

/// One row of digest history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestEntry {
    pub source_id: String,
    pub kind: String,
    pub title: String,
    pub url_or_path: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    /// PII redactions performed on the body before storage.  Surfaces
    /// in the activity feed so the user sees PII protection in action.
    #[serde(default)]
    pub pii_redactions: u64,
    #[serde(default)]
    pub bytes: u64,
}

/// Aggregate stats — used by the dashboard tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestStats {
    pub source_count: u64,
    pub feed_count: u64,
    pub folder_count: u64,
    pub clipboard_active: bool,
    pub screenshots_active: bool,
    pub paused: bool,
    pub quiet_hours_enabled: bool,
    pub history_rows: u64,
    pub total_pii_redactions: u64,
}

/// Result of one `pull_feed` invocation — captures conditional-GET
/// 304s and the parsed entry list otherwise.
#[derive(Debug, Clone, Default)]
pub struct FeedPullOutcome {
    pub not_modified: bool,
    pub bytes: usize,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub entries: Vec<feed_rs::model::Entry>,
}

/// Aggregate counts returned by [`crate::auto_digest_lite::run_once`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunOnceSummary {
    pub feeds_pulled: u64,
    pub feeds_unchanged: u64,
    pub entries_persisted: u64,
    pub folders_swept: u64,
    pub files_indexed: u64,
    pub chunks_indexed: u64,
    pub pii_redactions: u64,
    pub clipboard_active: bool,
    pub screenshots_active: bool,
    pub errors: u64,
}

/// Type aliases so clippy::type_complexity stays happy.
pub type WatcherType = Debouncer<RecommendedWatcher, RecommendedCache>;
pub type FolderWatchHandle = (String, WatcherType);
pub type FolderWatchEvent = (String, DebounceEventResult);
pub type FolderWatcherSetup = (Vec<FolderWatchHandle>, Receiver<FolderWatchEvent>);

/// Runtime-shared state — Tauri commands flip pause / resume here.
pub struct DigestRuntime {
    pub state: Mutex<DigestState>,
    pub paused: AtomicBool,
}

impl Default for DigestRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl DigestRuntime {
    pub fn new() -> Self {
        let state = super::state::load_state().unwrap_or_default();
        let paused = AtomicBool::new(state.paused);
        Self {
            state: Mutex::new(state),
            paused,
        }
    }

    pub fn snapshot(&self) -> AppResult<DigestState> {
        self.state
            .lock()
            .map(|g| g.clone())
            .map_err(|_| AppError::Internal("digest state mutex poisoned".into()))
    }

    pub fn with_state<F, R>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&mut DigestState) -> AppResult<R>,
    {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| AppError::Internal("digest state mutex poisoned".into()))?;
        let r = f(&mut guard)?;
        Ok(r)
    }

    /// Reload the in-memory state from disk.  Used by tests that
    /// switch `IMPFORGE_APP_HOME` between runs and need the runtime
    /// to forget the previous test's data.  Returns the loaded state.
    pub fn reload_from_disk(&self) -> AppResult<DigestState> {
        let fresh = super::state::load_state()?;
        let mut guard = self
            .state
            .lock()
            .map_err(|_| AppError::Internal("digest state mutex poisoned".into()))?;
        *guard = fresh.clone();
        Ok(fresh)
    }
}
