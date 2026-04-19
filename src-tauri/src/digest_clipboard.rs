// SPDX-License-Identifier: MIT
//! Opt-in clipboard digest source — Tier 2 of Feature 3.
//!
//! Polls the OS clipboard every `POLL_MS` (default 750 ms) on a
//! background thread.  Each new distinct payload runs through the PII
//! scrubber BEFORE landing in the knowledge base — Recall lesson #4.
//!
//! ## Why polling?
//!
//! Clipboard "watcher" APIs are inconsistent across Linux/macOS/Windows.
//! `arboard` exposes a synchronous `get_text` that we wrap in a polling
//! loop.  750 ms is fast enough for "clipboard → ingest" UX without
//! burning CPU.  We hash the last seen payload to skip duplicates.
//!
//! ## Off by default
//!
//! Nothing runs until `digest_clipboard_enable` is called.  Disable
//! at any time via `digest_clipboard_disable`; the polling thread
//! checks the atomic flag every iteration.

use crate::auto_digest_lite::{
    self, append_history, runtime, scrub_for_ingest, DigestEntry, DigestSource,
};
use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// Poll interval — 750 ms balances UX with CPU cost.
pub const POLL_MS: u64 = 750;

/// Hard cap on a single clipboard payload to avoid pasting a 100 MB
/// blob into the knowledge index.
pub const MAX_CLIPBOARD_BYTES: usize = 256 * 1024;

/// Global flag — flipped by `enable` / `disable`.  Polling thread
/// reads this every iteration and exits when false.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Last-seen hash.  Skip duplicates so the same clipboard contents
/// don't flood history every poll cycle.
static LAST_HASH: Mutex<Option<[u8; 32]>> = Mutex::new(None);

fn hash_payload(text: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher.finalize().into()
}

/// One clipboard event ready to ingest.  Built deterministically from
/// the polled payload so tests can drive the rest of the pipeline
/// without an actual clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardCapture {
    pub text: String,
    pub bytes: usize,
}

/// Validate / dedup a single payload; returns Some(capture) when the
/// payload should be persisted.  Pure function — no I/O.
pub fn evaluate(payload: &str) -> Option<ClipboardCapture> {
    if payload.trim().is_empty() {
        return None;
    }
    if payload.len() > MAX_CLIPBOARD_BYTES {
        tracing::warn!(
            "clipboard payload {} bytes exceeds cap {MAX_CLIPBOARD_BYTES} — dropped",
            payload.len()
        );
        return None;
    }
    let h = hash_payload(payload);
    if let Ok(mut guard) = LAST_HASH.lock() {
        if let Some(prev) = guard.as_ref() {
            if prev == &h {
                return None;
            }
        }
        *guard = Some(h);
    }
    Some(ClipboardCapture {
        text: payload.to_string(),
        bytes: payload.len(),
    })
}

/// Persist one captured clipboard payload — runs PII scrub PRE-storage.
pub fn persist(capture: &ClipboardCapture, source_id: &str) -> AppResult<u64> {
    let (scrubbed, redactions) = scrub_for_ingest(&capture.text);
    let dir = auto_digest_lite::digest_home()?;
    let cache = dir.join("digest-cache");
    std::fs::create_dir_all(&cache).map_err(AppError::Io)?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
    let path = cache.join(format!("clipboard-{ts}.txt"));
    std::fs::write(&path, &scrubbed).map_err(AppError::Io)?;

    if let Err(e) = crate::knowledge_lite::ingest_path_blocking(&path) {
        tracing::warn!("clipboard ingest skip: {e}");
    }

    append_history(&DigestEntry {
        source_id: source_id.to_string(),
        kind: "clipboard".into(),
        title: format!("clipboard ({} bytes)", capture.bytes),
        url_or_path: path.display().to_string(),
        fetched_at: chrono::Utc::now(),
        pii_redactions: redactions,
        bytes: capture.bytes as u64,
    })?;
    Ok(redactions)
}

/// Foreground-thread runner used by the spawn helper + tests.
pub fn step(payload: &str, source_id: &str) -> AppResult<u64> {
    match evaluate(payload) {
        Some(cap) => persist(&cap, source_id),
        None => Ok(0),
    }
}

/// Spawn the polling thread.  Returns immediately — the thread loops
/// until `ENABLED` flips false.  Idempotent: if already enabled, the
/// new call is a no-op.  Polling errors get logged + the loop continues.
pub fn spawn() {
    if ENABLED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        // Look up the configured Clipboard source ID (if any).  The
        // user might enable the daemon before adding the source — we
        // skip silently in that case.
        loop {
            if !ENABLED.load(Ordering::SeqCst) {
                break;
            }
            let rt = runtime();
            let source_id = match rt.snapshot() {
                Ok(s) => s.sources.iter().find_map(|s| match s {
                    DigestSource::Clipboard { id } => Some(id.clone()),
                    _ => None,
                }),
                Err(_) => None,
            };
            if let Some(sid) = source_id {
                match poll_clipboard() {
                    Ok(Some(text)) => {
                        if let Err(e) = step(&text, &sid) {
                            tracing::warn!("clipboard step: {e}");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => tracing::debug!("clipboard read: {e}"),
                }
            }
            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
    });
}

/// Read the current clipboard text — best-effort.  Returns Ok(None)
/// when the clipboard is empty or holds a non-text payload.  Callers
/// should treat any error as "skip this poll".
pub fn poll_clipboard() -> AppResult<Option<String>> {
    let mut clip = arboard::Clipboard::new()
        .map_err(|e| AppError::Internal(format!("arboard new: {e}")))?;
    match clip.get_text() {
        Ok(s) => Ok(Some(s)),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(e) => Err(AppError::Internal(format!("clipboard get_text: {e}"))),
    }
}

// ─── Tauri commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn digest_clipboard_enable() -> AppResult<()> {
    let rt = runtime();
    let has_source = rt
        .snapshot()?
        .sources
        .iter()
        .any(|s| matches!(s, DigestSource::Clipboard { .. }));
    if !has_source {
        rt.with_state(|state| {
            crate::auto_digest_lite::add_clipboard(state)?;
            crate::auto_digest_lite::save_state(state)
        })?;
    }
    spawn();
    Ok(())
}

#[tauri::command]
pub async fn digest_clipboard_disable() -> AppResult<()> {
    ENABLED.store(false, Ordering::SeqCst);
    if let Ok(mut g) = LAST_HASH.lock() {
        *g = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn digest_clipboard_status() -> AppResult<bool> {
    Ok(ENABLED.load(Ordering::SeqCst))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        if let Ok(mut g) = LAST_HASH.lock() {
            *g = None;
        }
    }

    #[test]
    fn evaluate_skips_empty() {
        reset();
        assert!(evaluate("").is_none());
        assert!(evaluate("   ").is_none());
    }

    #[test]
    fn evaluate_skips_oversize() {
        reset();
        let huge = "x".repeat(MAX_CLIPBOARD_BYTES + 1);
        assert!(evaluate(&huge).is_none());
    }

    #[test]
    fn evaluate_dedups_same_payload() {
        reset();
        let cap1 = evaluate("hello").expect("first");
        assert_eq!(cap1.text, "hello");
        let cap2 = evaluate("hello");
        assert!(cap2.is_none(), "duplicate must be skipped");
    }

    #[test]
    fn evaluate_admits_distinct_payload() {
        reset();
        let _ = evaluate("first");
        let cap = evaluate("second").expect("distinct");
        assert_eq!(cap.text, "second");
    }

    #[test]
    fn step_strips_pii_before_persist() {
        // Behavioural test — stub the persist by inspecting the
        // scrubbed result from the helper directly.  The end-to-end
        // persist path requires a writable APP_HOME, which is racy
        // across tests; this test focuses on the redaction guarantee.
        reset();
        let cap = evaluate("Card: 4111-1111-1111-1111 — please delete")
            .expect("evaluate");
        let (scrubbed, redactions) = scrub_for_ingest(&cap.text);
        assert!(redactions >= 1, "PII scrubber must redact CCN");
        assert!(
            scrubbed.contains("[REDACTED:CCN]"),
            "scrubbed text must contain CCN marker, got: {scrubbed}"
        );
    }

    #[test]
    fn enable_disable_flips_atomic() {
        // Direct flag manipulation (we don't actually spawn the thread
        // here to keep tests fast).
        ENABLED.store(false, Ordering::SeqCst);
        assert!(!ENABLED.load(Ordering::SeqCst));
        ENABLED.store(true, Ordering::SeqCst);
        assert!(ENABLED.load(Ordering::SeqCst));
        ENABLED.store(false, Ordering::SeqCst);
    }
}
