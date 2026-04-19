// SPDX-License-Identifier: MIT
//! Pure helpers — paths, timestamps, ID generation, quiet-hours
//! arithmetic, PII scrubbing wrapper, path sanitisation, extension
//! filtering.  Zero side effects beyond reading env / clock.

use crate::error::{AppError, AppResult};
use crate::pii_scrubber;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::DigestState;

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

pub(super) fn sources_path() -> AppResult<PathBuf> {
    Ok(digest_home()?.join("digest-sources.json"))
}

pub(super) fn history_path() -> AppResult<PathBuf> {
    Ok(digest_home()?.join("digest-history.jsonl"))
}

pub(super) fn cache_dir() -> AppResult<PathBuf> {
    let p = digest_home()?.join("digest-cache");
    std::fs::create_dir_all(&p).map_err(AppError::Io)?;
    Ok(p)
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Generate a stable source ID without pulling `uuid` (App uses uuid
/// elsewhere, but this keeps digest module self-contained).
pub(super) fn next_source_id(prefix: &str) -> String {
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

pub(super) fn in_window(h: u32, start: u32, end: u32) -> bool {
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

pub(super) fn sanitize_for_path(input: &str) -> String {
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

pub(super) fn is_extension_allowed(path: &std::path::Path, allow_ext: &[String]) -> bool {
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
