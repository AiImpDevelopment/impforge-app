// SPDX-License-Identifier: MIT
//! On-disk persistence + in-memory `DigestState` mutators.
//!
//! Pure functions over `DigestState` — no Tauri commands here.  Every
//! mutator returns the new `DigestSource` (or unit) so callers can
//! follow up with [`save_state`] in a single transaction.

use crate::error::{AppError, AppResult};
use std::path::PathBuf;

use super::helpers::{history_path, next_source_id, sources_path};
use super::types::{
    DigestEntry, DigestSource, DigestState, DigestStats, MIN_POLL_SECS,
};

// ─── Persistence ─────────────────────────────────────────────────────────

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

// ─── Add / remove ────────────────────────────────────────────────────────

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

