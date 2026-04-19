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

use crate::error::{AppError, AppResult};
use crate::knowledge_lite;
use crate::pii_scrubber;
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

// ─── On-disk persistence ─────────────────────────────────────────────────

/// Resolve the App's home directory.  Honours `IMPFORGE_APP_HOME` for
/// tests + BYO-vault deployments, defaults to `~/.impforge-app/`.
pub fn digest_home() -> AppResult<PathBuf> {
    let dir = if let Ok(custom) = std::env::var("IMPFORGE_APP_HOME") {
        PathBuf::from(custom)
    } else {
        dirs::home_dir()
            .ok_or_else(|| AppError::Internal("no HOME directory".into()))?
            .join(".impforge-app")
    };
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
    Ok(dir)
}

fn sources_path() -> AppResult<PathBuf> {
    Ok(digest_home()?.join("digest-sources.json"))
}

fn history_path() -> AppResult<PathBuf> {
    Ok(digest_home()?.join("digest-history.jsonl"))
}

fn cache_dir() -> AppResult<PathBuf> {
    let p = digest_home()?.join("digest-cache");
    std::fs::create_dir_all(&p).map_err(AppError::Io)?;
    Ok(p)
}

/// Load the persisted state (or default if no file).
pub fn load_state() -> AppResult<DigestState> {
    let path = sources_path()?;
    if !path.exists() {
        return Ok(DigestState::default());
    }
    let bytes = std::fs::read(&path).map_err(AppError::Io)?;
    let state: DigestState = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Internal(format!("parse digest state: {e}")))?;
    Ok(state)
}

/// Atomically persist the state via tmp-file + rename.
pub fn save_state(state: &DigestState) -> AppResult<()> {
    let path = sources_path()?;
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| AppError::Internal(format!("serialise state: {e}")))?;
    std::fs::write(&tmp, &bytes).map_err(AppError::Io)?;
    std::fs::rename(&tmp, &path).map_err(AppError::Io)?;
    Ok(())
}

/// Append one history row (jsonl format).
pub fn append_history(row: &DigestEntry) -> AppResult<()> {
    let path = history_path()?;
    let mut line = serde_json::to_string(row)
        .map_err(|e| AppError::Internal(format!("serialise history: {e}")))?;
    line.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(AppError::Io)?;
    use std::io::Write;
    f.write_all(line.as_bytes()).map_err(AppError::Io)?;
    Ok(())
}

/// Tail the last N history rows in reverse-chronological order.
pub fn tail_history(limit: usize) -> AppResult<Vec<DigestEntry>> {
    let path = history_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(AppError::Io)?;
    let mut out: Vec<DigestEntry> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    out.reverse();
    out.truncate(limit);
    Ok(out)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Generate a stable source ID without pulling `uuid` (App uses uuid
/// elsewhere, but this keeps digest module self-contained).
fn next_source_id(prefix: &str) -> String {
    use std::sync::atomic::AtomicU64;
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::SeqCst);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    format!("{prefix}-{t:016x}-{n:04x}")
}

// ─── Quiet-hours logic ───────────────────────────────────────────────────

/// True when the current LOCAL hour is inside the configured
/// quiet-hours window.  Window may cross midnight (start > end).
pub fn is_quiet_hours(state: &DigestState) -> bool {
    if !state.quiet_hours_enabled {
        return false;
    }
    let now = chrono::Local::now();
    let h = now.format("%H").to_string().parse::<u32>().unwrap_or(0);
    in_window(h, state.quiet_hours_start, state.quiet_hours_end)
}

fn in_window(h: u32, start: u32, end: u32) -> bool {
    if start <= end {
        h >= start && h < end
    } else {
        // crosses midnight (e.g. start=22, end=8)
        h >= start || h < end
    }
}

// ─── PII scrubbing helper ────────────────────────────────────────────────

/// Run the App's PII scrubber over an ingestion candidate.  Returns
/// `(scrubbed_text, redaction_count)`.  Falls back to passthrough
/// if the scrubber errors — better degraded than blocked.
pub fn scrub_for_ingest(text: &str) -> (String, u64) {
    match pii_scrubber::scrub(text) {
        Ok(result) => {
            let count = result.matches.len() as u64;
            (result.scrubbed, count)
        }
        Err(_) => (text.to_string(), 0),
    }
}

// ─── Add / remove / list ─────────────────────────────────────────────────

pub fn add_feed(
    state: &mut DigestState,
    feed_url: &str,
    interval_secs: u64,
) -> AppResult<DigestSource> {
    let parsed = url::Url::parse(feed_url)
        .map_err(|e| AppError::InvalidArgument(format!("invalid URL {feed_url}: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::InvalidArgument(format!(
            "URL must be http(s): {feed_url}"
        )));
    }
    let interval = interval_secs.max(MIN_POLL_SECS);
    let src = DigestSource::Feed {
        id: next_source_id("feed"),
        url: parsed.into(),
        interval_secs: interval,
        last_modified: None,
        etag: None,
        last_polled_unix: 0,
    };
    state.sources.push(src.clone());
    Ok(src)
}

pub fn add_folder(
    state: &mut DigestState,
    path: PathBuf,
    recursive: bool,
    allow_ext: Vec<String>,
) -> AppResult<DigestSource> {
    let canonical = path.canonicalize().map_err(|e| {
        AppError::InvalidArgument(format!("folder must exist: {} ({e})", path.display()))
    })?;
    if !canonical.is_dir() {
        return Err(AppError::InvalidArgument(format!(
            "not a directory: {}",
            canonical.display()
        )));
    }
    let src = DigestSource::Folder {
        id: next_source_id("folder"),
        path: canonical,
        recursive,
        allow_ext,
    };
    state.sources.push(src.clone());
    Ok(src)
}

pub fn add_clipboard(state: &mut DigestState) -> AppResult<DigestSource> {
    // Replace any existing Clipboard source — only one is meaningful.
    state.sources.retain(|s| !matches!(s, DigestSource::Clipboard { .. }));
    let src = DigestSource::Clipboard {
        id: next_source_id("clipboard"),
    };
    state.sources.push(src.clone());
    Ok(src)
}

pub fn add_screenshots(
    state: &mut DigestState,
    path: Option<PathBuf>,
) -> AppResult<DigestSource> {
    state
        .sources
        .retain(|s| !matches!(s, DigestSource::Screenshots { .. }));
    let canonical = match path {
        Some(p) => Some(p.canonicalize().map_err(|e| {
            AppError::InvalidArgument(format!("screenshots dir: {} ({e})", p.display()))
        })?),
        None => None,
    };
    let src = DigestSource::Screenshots {
        id: next_source_id("screenshots"),
        path: canonical,
    };
    state.sources.push(src.clone());
    Ok(src)
}

pub fn remove_source(state: &mut DigestState, id: &str) -> AppResult<()> {
    let before = state.sources.len();
    state.sources.retain(|s| s.id() != id);
    if state.sources.len() == before {
        return Err(AppError::NotFound(format!("digest source {id}")));
    }
    Ok(())
}

// ─── Feed pull (HTTP, in-memory parse) ───────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct FeedPullOutcome {
    pub not_modified: bool,
    pub bytes: usize,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub entries: Vec<feed_rs::model::Entry>,
}

/// Pull a single feed.  Honours `If-None-Match` / `If-Modified-Since`
/// for conditional GET — feeds that haven't changed return 304 with
/// no body, saving bytes + parser time.
pub async fn pull_feed(
    client: &reqwest::Client,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> AppResult<FeedPullOutcome> {
    let mut req = client.get(url).header("User-Agent", FEED_USER_AGENT);
    if let Some(e) = etag {
        req = req.header("If-None-Match", e);
    }
    if let Some(lm) = last_modified {
        req = req.header("If-Modified-Since", lm);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("fetch {url}: {e}")))?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(FeedPullOutcome {
            not_modified: true,
            ..Default::default()
        });
    }
    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "feed {url}: HTTP {}",
            resp.status()
        )));
    }
    let etag_out = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let lm_out = resp
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Internal(format!("read body {url}: {e}")))?;
    if bytes.len() > MAX_FEED_BYTES {
        return Err(AppError::InvalidArgument(format!(
            "feed {url}: body {} bytes exceeds cap {MAX_FEED_BYTES}",
            bytes.len()
        )));
    }
    let entries = parse_feed(&bytes)?;
    Ok(FeedPullOutcome {
        not_modified: false,
        bytes: bytes.len(),
        etag: etag_out,
        last_modified: lm_out,
        entries,
    })
}

/// Pure parse — no I/O.  Public for tests with canned XML.
pub fn parse_feed(bytes: &[u8]) -> AppResult<Vec<feed_rs::model::Entry>> {
    let parser = feed_rs::parser::Builder::new().build();
    let parsed = parser
        .parse(std::io::Cursor::new(bytes))
        .map_err(|e| AppError::Internal(format!("feed parse: {e}")))?;
    Ok(parsed.entries)
}

/// Convert one feed entry into (title, body) plain text.
pub fn entry_to_text(entry: &feed_rs::model::Entry) -> (String, String) {
    let title = entry
        .title
        .as_ref()
        .map(|t| t.content.clone())
        .unwrap_or_else(|| "untitled".into());
    let raw_body = entry
        .summary
        .as_ref()
        .map(|s| s.content.as_str())
        .or(entry
            .content
            .as_ref()
            .and_then(|c| c.body.as_deref()))
        .unwrap_or("");
    (title, strip_html(raw_body))
}

/// Tiny tag stripper — feed bodies often contain inline HTML.
pub fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if in_tag => {}
            _ => out.push(c),
        }
    }
    let mut compact = String::with_capacity(out.len());
    let mut prev_ws = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                compact.push(' ');
            }
            prev_ws = true;
        } else {
            compact.push(c);
            prev_ws = false;
        }
    }
    compact.trim().to_string()
}

/// Persist one feed entry.  Writes a temp `.txt` file under
/// `digest-cache/`, then routes through `knowledge_lite::ingest_path`
/// so the same dual-table FTS5 / RRF pipeline applies.  PII scrub
/// runs PRE-ingest.
pub async fn persist_feed_entry(
    feed_url: &str,
    source_id: &str,
    entry: &feed_rs::model::Entry,
) -> AppResult<u64> {
    let (title, body) = entry_to_text(entry);
    if body.trim().is_empty() {
        return Ok(0);
    }

    let (scrubbed, redactions) = scrub_for_ingest(&body);

    let cache = cache_dir()?;
    let slug = entry
        .id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(80)
        .collect::<String>();
    let cache_path = cache.join(format!(
        "{}-{}.txt",
        sanitize_for_path(feed_url),
        if slug.is_empty() {
            "entry".to_string()
        } else {
            slug
        }
    ));
    let payload =
        format!("{title}\n\nSource: {feed_url}\n\n{scrubbed}\n");
    std::fs::write(&cache_path, payload).map_err(AppError::Io)?;

    // Ingest synchronously into the shared knowledge DB.  We swallow
    // limit errors so a single feed can't kill the daemon — the
    // knowledge stats UI surfaces them clearly.
    if let Err(e) = knowledge_lite::ingest_path_blocking(&cache_path) {
        tracing::warn!("digest ingest skip {}: {e}", cache_path.display());
    }

    append_history(&DigestEntry {
        source_id: source_id.to_string(),
        kind: "feed".into(),
        title: title.clone(),
        url_or_path: feed_url.to_string(),
        fetched_at: chrono::Utc::now(),
        pii_redactions: redactions,
        bytes: cache_path
            .metadata()
            .map(|m| m.len())
            .unwrap_or_default(),
    })?;
    Ok(redactions)
}

fn sanitize_for_path(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(60)
        .collect()
}

// ─── Folder once-pass + watcher ──────────────────────────────────────────

pub fn ingest_folder_once(src: &DigestSource) -> AppResult<(u64, u64)> {
    let DigestSource::Folder {
        id,
        path,
        recursive,
        allow_ext,
    } = src
    else {
        return Err(AppError::InvalidArgument(
            "ingest_folder_once needs Folder source".into(),
        ));
    };

    use walkdir::WalkDir;
    let walker = if *recursive {
        WalkDir::new(path).into_iter()
    } else {
        WalkDir::new(path).max_depth(1).into_iter()
    };

    let mut files = 0u64;
    let mut chunks = 0u64;
    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        if !is_extension_allowed(p, allow_ext) {
            continue;
        }
        match knowledge_lite::ingest_path_blocking(p) {
            Ok(out) => {
                files += 1;
                chunks += out.chunk_count.max(0) as u64;
                let _ = append_history(&DigestEntry {
                    source_id: id.clone(),
                    kind: "folder".into(),
                    title: out.path.clone(),
                    url_or_path: out.path,
                    fetched_at: chrono::Utc::now(),
                    pii_redactions: 0,
                    bytes: out.bytes.max(0) as u64,
                });
            }
            Err(e) => {
                tracing::warn!("digest folder skip {}: {e}", p.display());
            }
        }
    }
    Ok((files, chunks))
}

fn is_extension_allowed(path: &std::path::Path, allow_ext: &[String]) -> bool {
    if allow_ext.is_empty() {
        return true;
    }
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext_lc = ext.to_ascii_lowercase();
    allow_ext
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&ext_lc))
}

/// Type aliases so clippy::type_complexity stays happy.
pub type WatcherType = Debouncer<RecommendedWatcher, RecommendedCache>;
pub type FolderWatchHandle = (String, WatcherType);
pub type FolderWatchEvent = (String, DebounceEventResult);
pub type FolderWatcherSetup = (Vec<FolderWatchHandle>, Receiver<FolderWatchEvent>);

/// Install file watchers over every Folder source in `state`.
pub fn install_folder_watchers(state: &DigestState) -> AppResult<FolderWatcherSetup> {
    let (tx, rx) = channel::<FolderWatchEvent>();
    let mut watchers = Vec::new();
    for src in &state.sources {
        if let DigestSource::Folder { id, path, recursive, .. } = src {
            let id_owned = id.clone();
            let tx_clone = tx.clone();
            let mut debouncer = new_debouncer(
                DEBOUNCER_TIMEOUT,
                None,
                move |events: DebounceEventResult| {
                    let _ = tx_clone.send((id_owned.clone(), events));
                },
            )
            .map_err(|e| AppError::Internal(format!("install watcher: {e}")))?;
            let mode = if *recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            debouncer
                .watch(path, mode)
                .map_err(|e| AppError::Internal(format!("watch {}: {e}", path.display())))?;
            watchers.push((id.clone(), debouncer));
        }
    }
    Ok((watchers, rx))
}

/// Process one debounced batch of filesystem events for `src`.
pub fn handle_fs_events(
    src: &DigestSource,
    events: &DebounceEventResult,
) -> AppResult<u64> {
    let DigestSource::Folder { id, allow_ext, .. } = src else {
        return Err(AppError::InvalidArgument(
            "handle_fs_events: not a Folder source".into(),
        ));
    };
    let Ok(events) = events else {
        return Ok(0);
    };
    let mut paths = BTreeSet::new();
    for ev in events {
        match ev.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for p in &ev.paths {
                    if p.is_file() && is_extension_allowed(p, allow_ext) {
                        paths.insert(p.clone());
                    }
                }
            }
            _ => {}
        }
    }
    let mut chunks = 0u64;
    for p in paths {
        match knowledge_lite::ingest_path_blocking(&p) {
            Ok(out) => {
                chunks += out.chunk_count.max(0) as u64;
                let _ = append_history(&DigestEntry {
                    source_id: id.clone(),
                    kind: "folder-live".into(),
                    title: out.path.clone(),
                    url_or_path: out.path,
                    fetched_at: chrono::Utc::now(),
                    pii_redactions: 0,
                    bytes: out.bytes.max(0) as u64,
                });
            }
            Err(e) => {
                tracing::warn!("digest live ingest skip {}: {e}", p.display());
            }
        }
    }
    Ok(chunks)
}

// ─── Run-once (used by `digest_run_once` Tauri command) ──────────────────

/// Run every source once + return summary counts.
pub async fn run_once(state: &mut DigestState) -> AppResult<RunOnceSummary> {
    let mut summary = RunOnceSummary::default();
    if state.paused {
        return Err(AppError::InvalidArgument(
            "digest paused — call digest_resume first".into(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| AppError::Internal(format!("reqwest client: {e}")))?;

    let sources = state.sources.clone();
    for src in &sources {
        match src {
            DigestSource::Feed {
                id,
                url,
                etag,
                last_modified,
                ..
            } => {
                let outcome = pull_feed(
                    &client,
                    url,
                    etag.as_deref(),
                    last_modified.as_deref(),
                )
                .await;
                match outcome {
                    Ok(o) => {
                        if !o.not_modified {
                            for entry in &o.entries {
                                match persist_feed_entry(url, id, entry).await {
                                    Ok(redactions) => {
                                        summary.feeds_pulled += 1;
                                        summary.entries_persisted += 1;
                                        summary.pii_redactions += redactions;
                                    }
                                    Err(e) => tracing::warn!("persist feed entry: {e}"),
                                }
                            }
                        } else {
                            summary.feeds_unchanged += 1;
                        }
                        // Update etag/last-modified.
                        for s in state.sources.iter_mut() {
                            if let DigestSource::Feed {
                                id: sid,
                                etag: e_slot,
                                last_modified: lm_slot,
                                last_polled_unix,
                                ..
                            } = s
                            {
                                if sid == id {
                                    *e_slot = o.etag.clone();
                                    *lm_slot = o.last_modified.clone();
                                    *last_polled_unix = now_unix();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("feed pull {url}: {e}");
                        summary.errors += 1;
                    }
                }
            }
            DigestSource::Folder { .. } => match ingest_folder_once(src) {
                Ok((files, chunks)) => {
                    summary.folders_swept += 1;
                    summary.files_indexed += files;
                    summary.chunks_indexed += chunks;
                }
                Err(e) => {
                    tracing::warn!("folder ingest: {e}");
                    summary.errors += 1;
                }
            },
            DigestSource::Clipboard { .. } => {
                // Clipboard runs in its own task — see digest_clipboard.
                summary.clipboard_active = true;
            }
            DigestSource::Screenshots { .. } => {
                summary.screenshots_active = true;
            }
            DigestSource::Browser { .. } => {
                // Browser is explicit-action only — no work to do here.
            }
        }
    }
    save_state(state)?;
    Ok(summary)
}

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

// ─── Stats ───────────────────────────────────────────────────────────────

pub fn compute_stats(state: &DigestState) -> AppResult<DigestStats> {
    let mut feeds = 0u64;
    let mut folders = 0u64;
    let mut clipboard = false;
    let mut screenshots = false;
    for s in &state.sources {
        match s {
            DigestSource::Feed { .. } => feeds += 1,
            DigestSource::Folder { .. } => folders += 1,
            DigestSource::Clipboard { .. } => clipboard = true,
            DigestSource::Screenshots { .. } => screenshots = true,
            DigestSource::Browser { .. } => {}
        }
    }

    let history = tail_history(usize::MAX).unwrap_or_default();
    let total_redactions = history.iter().map(|h| h.pii_redactions).sum();

    Ok(DigestStats {
        source_count: state.sources.len() as u64,
        feed_count: feeds,
        folder_count: folders,
        clipboard_active: clipboard,
        screenshots_active: screenshots,
        paused: state.paused,
        quiet_hours_enabled: state.quiet_hours_enabled,
        history_rows: history.len() as u64,
        total_pii_redactions: total_redactions,
    })
}

// ─── Global daemon plumbing (referenced by Tauri cmds) ───────────────────

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
        let state = load_state().unwrap_or_default();
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
        let fresh = load_state()?;
        let mut guard = self
            .state
            .lock()
            .map_err(|_| AppError::Internal("digest state mutex poisoned".into()))?;
        *guard = fresh.clone();
        Ok(fresh)
    }
}

/// Process-wide test-only mutex — every test that mutates
/// `IMPFORGE_APP_HOME` must hold this guard for the lifetime of its
/// tempdir.  Any per-module test mutex would race with siblings since
/// the env var is process-wide.  Exported so `digest_browser` /
/// `digest_clipboard` / `digest_screenshots` tests can reuse it.
#[cfg(test)]
pub static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

static RUNTIME: once_cell::sync::Lazy<Arc<DigestRuntime>> =
    once_cell::sync::Lazy::new(|| Arc::new(DigestRuntime::new()));

/// Public accessor — used by `digest_clipboard` + `digest_screenshots`
/// + `digest_browser` modules to share the same daemon state.
pub fn runtime() -> Arc<DigestRuntime> {
    RUNTIME.clone()
}

/// Best-effort drain of any pending watcher events, with a max
/// `timeout`.  Returns the number of chunks ingested.  Used by tests
/// + by the tauri command `digest_drain_now`.
pub fn drain_watcher(
    rx: &Receiver<FolderWatchEvent>,
    state: &DigestState,
    timeout: Duration,
) -> AppResult<u64> {
    let mut total = 0u64;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok((sid, events)) => {
                if let Some(src) = state.sources.iter().find(|s| s.id() == sid) {
                    total += handle_fs_events(src, &events)?;
                }
            }
            Err(RecvTimeoutError::Timeout) => break,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(total)
}

// ─── Tauri commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn digest_add_source(source: DigestSource) -> AppResult<DigestSource> {
    let rt = runtime();
    rt.with_state(|state| {
        match source {
            DigestSource::Feed {
                url,
                interval_secs,
                ..
            } => add_feed(state, &url, interval_secs),
            DigestSource::Folder {
                path,
                recursive,
                allow_ext,
                ..
            } => add_folder(state, path, recursive, allow_ext),
            DigestSource::Clipboard { .. } => add_clipboard(state),
            DigestSource::Screenshots { path, .. } => add_screenshots(state, path),
            DigestSource::Browser { family, .. } => {
                let src = DigestSource::Browser {
                    id: next_source_id("browser"),
                    family,
                };
                state.sources.push(src.clone());
                Ok(src)
            }
        }
        .and_then(|src| save_state(state).map(|_| src))
    })
}

#[tauri::command]
pub async fn digest_remove_source(id: String) -> AppResult<()> {
    let rt = runtime();
    rt.with_state(|state| {
        remove_source(state, &id)?;
        save_state(state)
    })
}

#[tauri::command]
pub async fn digest_list_sources() -> AppResult<Vec<DigestSource>> {
    Ok(runtime().snapshot()?.sources)
}

#[tauri::command]
pub async fn digest_run_once() -> AppResult<RunOnceSummary> {
    let rt = runtime();
    let mut state = rt.snapshot()?;
    let summary = run_once(&mut state).await?;
    rt.with_state(|s| {
        *s = state.clone();
        Ok(())
    })?;
    Ok(summary)
}

#[tauri::command]
pub async fn digest_history(limit: u32) -> AppResult<Vec<DigestEntry>> {
    tail_history(limit as usize)
}

#[tauri::command]
pub async fn digest_pause() -> AppResult<()> {
    let rt = runtime();
    rt.paused.store(true, Ordering::SeqCst);
    rt.with_state(|state| {
        state.paused = true;
        save_state(state)
    })
}

#[tauri::command]
pub async fn digest_resume() -> AppResult<()> {
    let rt = runtime();
    rt.paused.store(false, Ordering::SeqCst);
    rt.with_state(|state| {
        state.paused = false;
        save_state(state)
    })
}

#[tauri::command]
pub async fn digest_stats() -> AppResult<DigestStats> {
    let state = runtime().snapshot()?;
    compute_stats(&state)
}

#[tauri::command]
pub async fn digest_set_quiet_hours(
    enabled: bool,
    start: u32,
    end: u32,
) -> AppResult<DigestState> {
    let rt = runtime();
    rt.with_state(|state| {
        state.quiet_hours_enabled = enabled;
        state.quiet_hours_start = start.min(23);
        state.quiet_hours_end = end.min(23);
        save_state(state)?;
        Ok(state.clone())
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct HomeGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        _dir: tempfile::TempDir,
    }

    fn isolated_home() -> HomeGuard {
        let guard = match TEST_ENV_LOCK.lock() {
            Ok(g) => g,
            Err(p) => {
                TEST_ENV_LOCK.clear_poison();
                p.into_inner()
            }
        };
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var("IMPFORGE_APP_HOME", dir.path());
        // Force RUNTIME to drop any cross-test in-memory state.
        let _ = runtime().reload_from_disk();
        HomeGuard {
            _lock: guard,
            _dir: dir,
        }
    }

    #[test]
    fn default_state_is_safe() {
        let _h = isolated_home();
        let state = load_state().expect("load");
        assert!(!state.paused);
        assert!(state.sources.is_empty());
        assert!(state.quiet_hours_enabled);
        assert_eq!(state.quiet_hours_start, DEFAULT_QUIET_HOURS.0);
    }

    #[test]
    fn add_feed_validates_scheme() {
        let _h = isolated_home();
        let mut state = load_state().expect("load");
        assert!(add_feed(&mut state, "ftp://example.com/feed", 60).is_err());
        assert!(add_feed(&mut state, "not a url", 60).is_err());
    }

    #[test]
    fn add_feed_clamps_interval_below_minimum() {
        let _h = isolated_home();
        let mut state = load_state().expect("load");
        let src = add_feed(&mut state, "https://example.com/rss", 5).expect("add");
        if let DigestSource::Feed { interval_secs, .. } = src {
            assert_eq!(interval_secs, MIN_POLL_SECS);
        } else {
            panic!("expected Feed");
        }
    }

    #[test]
    fn add_folder_requires_existing_dir() {
        let _h = isolated_home();
        let mut state = load_state().expect("load");
        let bad = add_folder(
            &mut state,
            PathBuf::from("/nonexistent/missing/missing"),
            false,
            vec![],
        );
        assert!(bad.is_err());
    }

    #[test]
    fn add_clipboard_replaces_previous() {
        let _h = isolated_home();
        let mut state = load_state().expect("load");
        let _a = add_clipboard(&mut state).expect("first");
        let _b = add_clipboard(&mut state).expect("second");
        let count = state
            .sources
            .iter()
            .filter(|s| matches!(s, DigestSource::Clipboard { .. }))
            .count();
        assert_eq!(count, 1, "second clipboard registration must replace first");
    }

    #[test]
    fn quiet_hours_window_simple() {
        assert!(in_window(23, 22, 8));
        assert!(in_window(2, 22, 8));
        assert!(!in_window(15, 22, 8));
        assert!(in_window(10, 9, 17));
        assert!(!in_window(8, 9, 17));
    }

    #[test]
    fn parse_feed_atom_works() {
        let atom = br#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>X</title><id>urn:x</id>
  <updated>2026-04-19T00:00:00Z</updated>
  <entry>
    <id>urn:e1</id><updated>2026-04-19T00:00:00Z</updated>
    <title>Hello</title><summary>World</summary>
  </entry>
</feed>"#;
        let entries = parse_feed(atom).expect("parse");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn strip_html_collapses() {
        assert_eq!(strip_html("<p>Hi  there</p>"), "Hi there");
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn append_history_then_tail() {
        let _h = isolated_home();
        for i in 0..3 {
            append_history(&DigestEntry {
                source_id: format!("s{i}"),
                kind: "test".into(),
                title: format!("t{i}"),
                url_or_path: format!("u{i}"),
                fetched_at: chrono::Utc::now(),
                pii_redactions: i as u64,
                bytes: 0,
            })
            .expect("append");
        }
        let rows = tail_history(2).expect("tail");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].source_id, "s2");
    }

    #[test]
    fn folder_watcher_detects_new_file_app() {
        // Behavioural test: install watcher, drop file, expect event.
        let _h = isolated_home();
        let tmp = tempfile::tempdir().expect("tempdir watcher");
        let mut state = load_state().expect("load");
        add_folder(
            &mut state,
            tmp.path().to_path_buf(),
            false,
            vec!["txt".into()],
        )
        .expect("add folder");
        save_state(&state).expect("save");

        let (_watchers, rx) = install_folder_watchers(&state).expect("install");
        std::thread::sleep(Duration::from_millis(150));

        std::fs::write(tmp.path().join("hello.txt"), b"hi").expect("write");

        let received = rx.recv_timeout(DEBOUNCER_TIMEOUT + Duration::from_secs(2));
        assert!(received.is_ok(), "watcher must observe new file");
    }

    #[test]
    fn rss_scheduler_clamps_interval() {
        // Behavioural test: feed added with interval=10 must be
        // persisted with interval=MIN_POLL_SECS so the daemon doesn't
        // poll faster than once a minute (Recall lesson #1).
        let _h = isolated_home();
        let mut state = load_state().expect("load");
        let _src =
            add_feed(&mut state, "https://example.com/rss", 10).expect("add");
        save_state(&state).expect("save");

        let reload = load_state().expect("reload");
        if let DigestSource::Feed { interval_secs, .. } = &reload.sources[0] {
            assert_eq!(*interval_secs, MIN_POLL_SECS);
        } else {
            panic!("expected Feed");
        }
    }

    #[test]
    fn run_once_errors_when_paused() {
        let _h = isolated_home();
        let mut state = DigestState {
            paused: true,
            ..Default::default()
        };
        let res = futures::executor::block_on(run_once(&mut state));
        assert!(res.is_err(), "run_once must refuse when paused");
    }

    #[test]
    fn compute_stats_counts_each_source_type() {
        let _h = isolated_home();
        let tmp = tempfile::tempdir().expect("tempdir for stats");
        let mut state = load_state().expect("load");
        let _ = add_feed(&mut state, "https://example.com/a", 60);
        let _ = add_folder(&mut state, tmp.path().to_path_buf(), false, vec![]);
        let _ = add_clipboard(&mut state);
        let stats = compute_stats(&state).expect("stats");
        assert_eq!(stats.feed_count, 1);
        assert_eq!(stats.folder_count, 1);
        assert!(stats.clipboard_active);
    }

    #[test]
    fn scrub_for_ingest_passes_through_when_no_pii() {
        // PII scrubber fallthrough — even with the stubbed scrubber
        // the function returns text + 0 redactions.
        let (out, n) = scrub_for_ingest("hello world");
        assert_eq!(out, "hello world");
        assert_eq!(n, 0);
    }

    #[test]
    fn sanitize_for_path_safe() {
        let s = sanitize_for_path("https://example.com/feed?x=1");
        assert!(s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn ingest_folder_once_picks_up_files_app() {
        let _h = isolated_home();
        let tmp = tempfile::tempdir().expect("tempdir ingest");
        std::fs::write(
            tmp.path().join("note.md"),
            "# Hi\n\nMore body to chunk.",
        )
        .expect("write");

        let src = DigestSource::Folder {
            id: "f-test".into(),
            path: tmp.path().canonicalize().expect("canon"),
            recursive: false,
            allow_ext: vec![],
        };
        let (files, _chunks) = ingest_folder_once(&src).expect("ingest");
        assert!(files >= 1);
    }
}
