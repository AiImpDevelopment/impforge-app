// SPDX-License-Identifier: MIT
//! `run_once` scheduler + global `DigestRuntime` singleton + test
//! environment lock.
//!
//! This module owns the process-wide singletons.  Splitting them out
//! keeps the rest of `auto_digest_lite` testable in isolation
//! (sub-module tests construct fresh `DigestState` values without
//! touching the global runtime).

use crate::error::{AppError, AppResult};
use std::sync::Arc;
use std::time::Duration;

use super::feed::{persist_feed_entry, pull_feed};
use super::helpers::now_unix;
use super::state::save_state;
use super::types::{DigestRuntime, DigestSource, DigestState, RunOnceSummary};
use super::watcher::ingest_folder_once;

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
