// SPDX-License-Identifier: MIT
//! Folder filesystem watcher + one-shot folder ingest + event drain.
//!
//! Wraps `notify-debouncer-full` so callers don't see the raw
//! crate types.  Every event flows through `knowledge_lite::ingest_path_blocking`
//! — same pipeline as direct ingest commands.

use crate::error::{AppError, AppResult};
use crate::knowledge_lite;
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode},
    DebounceEventResult,
};
use std::collections::BTreeSet;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;

use super::helpers::is_extension_allowed;
use super::state::append_history;
use super::types::{
    DigestEntry, DigestSource, DigestState, FolderWatchEvent,
    FolderWatcherSetup, DEBOUNCER_TIMEOUT,
};

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
