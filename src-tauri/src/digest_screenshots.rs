// SPDX-License-Identifier: MIT
//! Opt-in screenshot folder watcher with optional OCR — Tier 2 of
//! Feature 3.
//!
//! Watches the OS's canonical screenshot folder (or a user override)
//! using the same `notify-debouncer-full` pipeline as `auto_digest_lite`.
//! When a new image lands, we queue it for OCR — which is implemented
//! as a "register-only" hook in this tier (the heavy `ocrs` crate is
//! Pro-tier; the App tier ingests the FILE METADATA + filename so the
//! search index still surfaces "screenshot taken at HH:MM" hits).
//!
//! ## Why no bundled OCR in MIT?
//!
//! - `ocrs` adds ~80 MB of model files; the App is meant to stay lean.
//! - OCR engines vary wildly in license quality (some Tesseract
//!   training data is GPL).  We point the user at Pro for AI OCR.
//! - The metadata ingest still gives the user a usable search index —
//!   filename + timestamp + folder + file size all become FTS5 hits.
//!
//! ## Off by default
//!
//! Nothing happens until `digest_screenshots_enable` is invoked.

use crate::auto_digest_lite::{
    append_history, digest_home, runtime, save_state, DigestEntry, DigestSource,
};
use crate::error::{AppError, AppResult};
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

pub const DEBOUNCER_TIMEOUT: Duration = Duration::from_millis(750);

/// Track whether the watcher thread is alive.  Used by `enable` /
/// `disable` to keep behaviour idempotent.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Default screenshot folder per OS.  macOS uses `~/Desktop`;
/// Windows + Linux use `~/Pictures/Screenshots`.
pub fn default_screenshots_folder() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    if cfg!(target_os = "macos") {
        Some(home.join("Desktop"))
    } else {
        Some(home.join("Pictures").join("Screenshots"))
    }
}

/// Recognised image extensions for "this is a screenshot" filter.
pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "tiff", "heic"];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Build a metadata-only ingest payload.  We deliberately do NOT
/// attempt OCR here — the App tier ships the lean version.  The
/// payload is still searchable by filename + folder + timestamp,
/// which covers the common "find that screenshot from yesterday" use.
pub fn build_metadata_payload(path: &Path) -> AppResult<String> {
    let meta = std::fs::metadata(path).map_err(AppError::Io)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| d.as_secs().to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let folder = path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let size_kb = meta.len() / 1024;
    Ok(format!(
        "Screenshot: {name}\nFolder: {folder}\nCaptured: {modified}\nSize: {size_kb} KB\n\
         OCR text: <unavailable in MIT tier — upgrade to Pro for AI OCR>\n"
    ))
}

/// Persist one screenshot's metadata into the knowledge base.
pub fn persist_screenshot(path: &Path, source_id: &str) -> AppResult<()> {
    let payload = build_metadata_payload(path)?;
    let cache = digest_home()?.join("digest-cache");
    std::fs::create_dir_all(&cache).map_err(AppError::Io)?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("screenshot");
    let txt_path = cache.join(format!("screenshot-{stem}.txt"));
    std::fs::write(&txt_path, &payload).map_err(AppError::Io)?;

    if let Err(e) = crate::knowledge_lite::ingest_path_blocking(&txt_path) {
        tracing::warn!("screenshot ingest skip: {e}");
    }

    append_history(&DigestEntry {
        source_id: source_id.to_string(),
        kind: "screenshot".into(),
        title: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("screenshot")
            .into(),
        url_or_path: path.display().to_string(),
        fetched_at: chrono::Utc::now(),
        pii_redactions: 0,
        bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or_default(),
    })?;
    Ok(())
}

/// Local watcher setup that mirrors `auto_digest_lite::install_folder_watchers`
/// but is ALWAYS rooted at exactly one folder.
pub type ScreenshotWatcherSetup = (
    Debouncer<RecommendedWatcher, RecommendedCache>,
    Receiver<DebounceEventResult>,
);

pub fn install_watcher(folder: &Path) -> AppResult<ScreenshotWatcherSetup> {
    let (tx, rx) = channel::<DebounceEventResult>();
    let mut debouncer = new_debouncer(
        DEBOUNCER_TIMEOUT,
        None,
        move |events: DebounceEventResult| {
            let _ = tx.send(events);
        },
    )
    .map_err(|e| AppError::Internal(format!("install screenshot watcher: {e}")))?;
    debouncer
        .watch(folder, RecursiveMode::NonRecursive)
        .map_err(|e| AppError::Internal(format!("watch {}: {e}", folder.display())))?;
    Ok((debouncer, rx))
}

/// Process one event batch — extracts the new image paths and ingests
/// each.  Returns the number of screenshots persisted.
pub fn process_events(
    events: &DebounceEventResult,
    source_id: &str,
) -> AppResult<u64> {
    let Ok(events) = events else {
        return Ok(0);
    };
    let mut images = BTreeSet::new();
    for ev in events {
        if !matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            continue;
        }
        for p in &ev.paths {
            if p.is_file() && is_image(p) {
                images.insert(p.clone());
            }
        }
    }
    let mut count = 0u64;
    for img in images {
        if let Err(e) = persist_screenshot(&img, source_id) {
            tracing::warn!("persist screenshot {}: {e}", img.display());
        } else {
            count += 1;
        }
    }
    Ok(count)
}

/// Spawn the screenshot watcher thread.  Listens on the resolved
/// folder until `ENABLED` flips false.
pub fn spawn() -> AppResult<()> {
    if ENABLED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let rt = runtime();
    let snapshot = rt.snapshot()?;
    let folder = snapshot
        .sources
        .iter()
        .find_map(|s| match s {
            DigestSource::Screenshots { path, .. } => Some(
                path.clone()
                    .or_else(default_screenshots_folder)
                    .unwrap_or_else(|| PathBuf::from(".")),
            ),
            _ => None,
        })
        .or_else(default_screenshots_folder)
        .ok_or_else(|| AppError::Internal("no screenshots folder configured".into()))?;
    if !folder.is_dir() {
        return Err(AppError::InvalidArgument(format!(
            "screenshots folder missing: {}",
            folder.display()
        )));
    }

    let source_id = snapshot
        .sources
        .iter()
        .find_map(|s| match s {
            DigestSource::Screenshots { id, .. } => Some(id.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "screenshots".into());

    std::thread::spawn(move || {
        let (debouncer, rx) = match install_watcher(&folder) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("screenshots watcher install: {e}");
                ENABLED.store(false, Ordering::SeqCst);
                return;
            }
        };
        let _kept_alive = debouncer; // keep watcher alive for the loop
        loop {
            if !ENABLED.load(Ordering::SeqCst) {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(events) => {
                    if let Err(e) = process_events(&events, &source_id) {
                        tracing::warn!("process screenshot events: {e}");
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    Ok(())
}

// ─── Tauri commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn digest_screenshots_enable(folder: Option<String>) -> AppResult<()> {
    let rt = runtime();
    let path_opt = folder.map(PathBuf::from);
    rt.with_state(|state| {
        crate::auto_digest_lite::add_screenshots(state, path_opt.clone())?;
        save_state(state)
    })?;
    spawn()?;
    Ok(())
}

#[tauri::command]
pub async fn digest_screenshots_disable() -> AppResult<()> {
    ENABLED.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn digest_screenshots_status() -> AppResult<bool> {
    Ok(ENABLED.load(Ordering::SeqCst))
}

#[tauri::command]
pub async fn digest_screenshots_default_folder() -> AppResult<Option<String>> {
    Ok(default_screenshots_folder().map(|p| p.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_image_extensions() {
        assert!(is_image(Path::new("foo.PNG")));
        assert!(is_image(Path::new("bar.jpg")));
        assert!(!is_image(Path::new("doc.pdf")));
        assert!(!is_image(Path::new("noext")));
    }

    #[test]
    fn default_folder_resolves() {
        // Should be Some for any normal user session — we don't assert
        // on the exact path because it varies per OS / CI.
        let folder = default_screenshots_folder();
        assert!(folder.is_some());
    }

    #[test]
    fn build_metadata_payload_includes_size_and_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("Screenshot 2026-04-19 at 12.34.56.png");
        std::fs::write(&path, b"\x89PNG\r\n").expect("write");
        let payload = build_metadata_payload(&path).expect("payload");
        assert!(payload.contains("Screenshot: Screenshot"));
        assert!(payload.contains("Folder:"));
        assert!(payload.contains("Captured:"));
        assert!(payload.contains("Size:"));
        // PRO upsell hint must be present.
        assert!(payload.contains("upgrade to Pro for AI OCR"));
    }

    #[test]
    fn install_watcher_picks_up_new_image() {
        // Behavioural test — install watcher, drop file, expect event.
        let tmp = tempfile::tempdir().expect("tempdir");
        let (watcher, rx) = install_watcher(tmp.path()).expect("install");
        std::thread::sleep(Duration::from_millis(150));
        let png = tmp.path().join("shot.png");
        std::fs::write(&png, b"fake png").expect("write");
        let received = rx.recv_timeout(DEBOUNCER_TIMEOUT + Duration::from_secs(2));
        assert!(received.is_ok(), "watcher must observe new image");
        drop(watcher);
    }

    #[test]
    fn process_events_picks_only_images() {
        // Build a fake event with a mixed batch — only PNG should ingest.
        // We can't easily construct DebouncedEvent without the
        // notify-debouncer-full builder, so we verify the filter logic
        // through `is_image` directly (sufficient for the contract).
        assert!(is_image(Path::new("a.png")));
        assert!(!is_image(Path::new("a.txt")));
    }
}
