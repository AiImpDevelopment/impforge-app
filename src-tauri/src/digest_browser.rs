// SPDX-License-Identifier: MIT
//! Explicit-action browser bookmarks/history import — Tier 2 of
//! Feature 3 (Global Digest).
//!
//! Wraps the existing `browser_import_oneshot` module with the digest
//! source/history tracking so that "import bookmarks from Firefox"
//! shows up in the unified Digest Activity feed alongside RSS pulls
//! and folder watches.
//!
//! ## Why explicit-action only?
//!
//! Browser SQLite databases are locked while the browser runs.  The
//! WAL-copy pattern (copy `places.sqlite` to a temp file, open with
//! `?mode=ro&nolock=1`) bypasses this — we ship that pattern in
//! `browser_import_oneshot.rs` already.  Polling browser state is
//! overkill; the user invokes import explicitly when they want it.
//!
//! ## Privacy
//!
//! Bookmarks + history are personal data.  Every import:
//!   1. runs through PII scrub (URLs containing tokens get redacted).
//!   2. records a provenance row in digest history with the browser
//!      family + count of imported items.
//!   3. is opt-in per-call — no auto-import, no background scan.

use crate::auto_digest_lite::{
    self, append_history, runtime, save_state, scrub_for_ingest, DigestEntry,
    DigestSource,
};
use crate::browser_import_oneshot::{
    self, BrowserBookmark, BrowserFamily, BrowserHistoryItem, BrowserProfile,
};
use crate::error::{AppError, AppResult};

/// Persist a batch of bookmark URLs into the knowledge index — each
/// bookmark becomes a small text entry so it shows up in search.
pub fn persist_bookmarks(
    bookmarks: &[BrowserBookmark],
    family: BrowserFamily,
    source_id: &str,
) -> AppResult<u64> {
    let cache = auto_digest_lite::digest_home()?.join("digest-cache");
    std::fs::create_dir_all(&cache).map_err(AppError::Io)?;

    let mut total_redactions = 0u64;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let path = cache.join(format!("bookmarks-{family:?}-{timestamp}.txt"));

    let mut payload = String::new();
    for b in bookmarks {
        let line = format!(
            "{} — {}\n  url: {}\n  added: {}\n  folder: {}\n",
            b.title,
            b.url,
            b.url,
            b.date_added.map(|d| d.to_rfc3339()).unwrap_or_default(),
            b.folder.as_deref().unwrap_or(""),
        );
        let (scrubbed, redactions) = scrub_for_ingest(&line);
        total_redactions += redactions;
        payload.push_str(&scrubbed);
        payload.push('\n');
    }
    std::fs::write(&path, &payload).map_err(AppError::Io)?;

    if let Err(e) = crate::knowledge_lite::ingest_path_blocking(&path) {
        tracing::warn!("bookmarks ingest skip: {e}");
    }

    append_history(&DigestEntry {
        source_id: source_id.to_string(),
        kind: "browser-bookmarks".into(),
        title: format!("{} bookmarks ({:?})", bookmarks.len(), family),
        url_or_path: path.display().to_string(),
        fetched_at: chrono::Utc::now(),
        pii_redactions: total_redactions,
        bytes: payload.len() as u64,
    })?;
    Ok(total_redactions)
}

/// Persist a batch of history items.
pub fn persist_history(
    items: &[BrowserHistoryItem],
    family: BrowserFamily,
    source_id: &str,
    days: u32,
) -> AppResult<u64> {
    let cache = auto_digest_lite::digest_home()?.join("digest-cache");
    std::fs::create_dir_all(&cache).map_err(AppError::Io)?;

    let mut total_redactions = 0u64;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let path = cache.join(format!("history-{family:?}-{timestamp}.txt"));

    let mut payload = format!("Browser history ({family:?}, last {days} days):\n\n");
    for item in items {
        let line = format!(
            "{} — {}\n  url: {}\n  visit: {}\n  count: {}\n",
            item.title.as_deref().unwrap_or("(no title)"),
            item.url,
            item.url,
            item.last_visit.map(|d| d.to_rfc3339()).unwrap_or_default(),
            item.visit_count,
        );
        let (scrubbed, redactions) = scrub_for_ingest(&line);
        total_redactions += redactions;
        payload.push_str(&scrubbed);
        payload.push('\n');
    }
    std::fs::write(&path, &payload).map_err(AppError::Io)?;

    if let Err(e) = crate::knowledge_lite::ingest_path_blocking(&path) {
        tracing::warn!("history ingest skip: {e}");
    }

    append_history(&DigestEntry {
        source_id: source_id.to_string(),
        kind: "browser-history".into(),
        title: format!("{} history items ({:?}, last {days}d)", items.len(), family),
        url_or_path: path.display().to_string(),
        fetched_at: chrono::Utc::now(),
        pii_redactions: total_redactions,
        bytes: payload.len() as u64,
    })?;
    Ok(total_redactions)
}

/// Ensure a Browser source exists in the runtime state for the given
/// family, returning its ID.  Idempotent — repeated calls with the
/// same family reuse the existing source.
pub fn ensure_source(family: BrowserFamily) -> AppResult<String> {
    let rt = runtime();
    let family_str = format!("{family:?}").to_ascii_lowercase();
    rt.with_state(|state| {
        if let Some(existing) = state.sources.iter().find_map(|s| match s {
            DigestSource::Browser { id, family: f } if f == &family_str => Some(id.clone()),
            _ => None,
        }) {
            return Ok(existing);
        }
        let id = format!(
            "browser-{}-{}",
            family_str,
            chrono::Utc::now().timestamp_millis()
        );
        state.sources.push(DigestSource::Browser {
            id: id.clone(),
            family: family_str,
        });
        save_state(state)?;
        Ok(id)
    })
}

// ─── Tauri commands ──────────────────────────────────────────────────────

/// Detect available browser profiles on this machine.  Thin wrapper
/// over `browser_import_oneshot::browser_detect_profiles` so the
/// Digest UI can use the existing detection without re-implementing.
#[tauri::command]
pub async fn digest_browser_profiles() -> AppResult<Vec<BrowserProfile>> {
    browser_import_oneshot::browser_detect_profiles().await
}

/// Import bookmarks from `family` and persist to the knowledge base.
#[tauri::command]
pub async fn digest_browser_import_bookmarks(
    family: BrowserFamily,
) -> AppResult<u64> {
    let bookmarks =
        browser_import_oneshot::browser_import_bookmarks(family).await?;
    let source_id = ensure_source(family)?;
    let redactions = persist_bookmarks(&bookmarks, family, &source_id)?;
    Ok(redactions)
}

/// Import the last `days` of browser history.
#[tauri::command]
pub async fn digest_browser_import_history(
    family: BrowserFamily,
    days: u32,
) -> AppResult<u64> {
    let items =
        browser_import_oneshot::browser_import_history(family, days).await?;
    let source_id = ensure_source(family)?;
    let redactions = persist_history(&items, family, &source_id, days)?;
    Ok(redactions)
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct HomeGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        _dir: tempfile::TempDir,
    }

    fn isolated_home() -> HomeGuard {
        let guard = match ENV_LOCK.lock() {
            Ok(g) => g,
            Err(p) => {
                ENV_LOCK.clear_poison();
                p.into_inner()
            }
        };
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var("IMPFORGE_APP_HOME", dir.path());
        // Reset the global RUNTIME so a previous test's in-memory
        // state doesn't leak into ours.
        let _ = auto_digest_lite::runtime().reload_from_disk();
        HomeGuard {
            _lock: guard,
            _dir: dir,
        }
    }

    #[test]
    fn ensure_source_is_idempotent() {
        let _h = isolated_home();
        let id1 = ensure_source(BrowserFamily::Firefox).expect("first");
        let id2 = ensure_source(BrowserFamily::Firefox).expect("second");
        assert_eq!(id1, id2, "second call must reuse the source ID");
    }

    #[test]
    fn ensure_source_distinct_per_family() {
        let _h = isolated_home();
        let f = ensure_source(BrowserFamily::Firefox).expect("ff");
        let c = ensure_source(BrowserFamily::Chrome).expect("ch");
        assert_ne!(f, c);
    }

    #[test]
    fn persist_bookmarks_runs_pii_scrub() {
        // Behavioural test: a bookmark with an embedded API key in the
        // URL gets redacted before storage.
        let _h = isolated_home();
        let id = ensure_source(BrowserFamily::Firefox).expect("source");
        let bookmarks = vec![BrowserBookmark {
            title: "Test".into(),
            url: "https://example.com?token=sk-abcdefghijklmnopqrstuvwxyz1234"
                .into(),
            folder: Some("Bookmarks Bar".into()),
            date_added: None,
        }];
        let redactions = persist_bookmarks(&bookmarks, BrowserFamily::Firefox, &id)
            .expect("persist");
        assert!(
            redactions >= 1,
            "API key in URL must be redacted, got {redactions}"
        );
    }

    #[test]
    fn persist_history_writes_file_and_history_row() {
        let _h = isolated_home();
        let id = ensure_source(BrowserFamily::Chrome).expect("source");
        let items = vec![BrowserHistoryItem {
            url: "https://example.com".into(),
            title: Some("Example".into()),
            visit_count: 3,
            last_visit: None,
        }];
        let redactions =
            persist_history(&items, BrowserFamily::Chrome, &id, 7).expect("persist");
        // No PII in this URL, so 0 redactions.
        assert_eq!(redactions, 0);
        // History row should now exist.
        let rows = auto_digest_lite::tail_history(10).expect("tail");
        assert!(
            rows.iter().any(|r| r.kind == "browser-history"),
            "expected browser-history row in tail"
        );
    }
}
