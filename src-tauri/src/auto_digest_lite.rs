// SPDX-License-Identifier: MIT
//! Auto-digest engine for RSS feeds and local files. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED #4.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: scheduled RSS pull + local-file watch with content extraction.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Source from which an auto-digest pulls entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DigestSource {
    Rss { url: String },
    LocalFile { path: String },
}

/// One digested entry from a digest source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestEntry {
    pub source: DigestSource,
    pub title: String,
    pub body: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// Register a new digest source. Returns its assigned identifier.
#[tauri::command]
pub async fn digest_add_source(_source: DigestSource) -> AppResult<String> {
    Err(AppError::Internal(
        "auto_digest_lite::digest_add_source not implemented in Phase 1".into(),
    ))
}

/// Remove a registered digest source by id.
#[tauri::command]
pub async fn digest_remove_source(_id: String) -> AppResult<()> {
    Err(AppError::Internal(
        "auto_digest_lite::digest_remove_source not implemented in Phase 1".into(),
    ))
}

/// List all currently-registered digest sources.
#[tauri::command]
pub async fn digest_list_sources() -> AppResult<Vec<DigestSource>> {
    Err(AppError::Internal(
        "auto_digest_lite::digest_list_sources not implemented in Phase 1".into(),
    ))
}

/// Force-run the digest source identified by `id` once.
#[tauri::command]
pub async fn digest_run_once(_id: String) -> AppResult<Vec<DigestEntry>> {
    Err(AppError::Internal(
        "auto_digest_lite::digest_run_once not implemented in Phase 1".into(),
    ))
}

/// Return the most recent digest history (across all sources).
#[tauri::command]
pub async fn digest_history(_limit: u32) -> AppResult<Vec<DigestEntry>> {
    Err(AppError::Internal(
        "auto_digest_lite::digest_history not implemented in Phase 1".into(),
    ))
}
