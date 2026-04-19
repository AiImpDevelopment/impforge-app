// SPDX-License-Identifier: MIT
//! One-shot browser bookmark + history importer. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED #5.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: explicit-action profile detection + bookmark/history extraction.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Supported browser families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    Firefox,
    Chromium,
    Brave,
    Edge,
    Vivaldi,
    Opera,
}

/// One detected browser profile on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub kind: BrowserKind,
    pub path: String,
    pub name: String,
}

/// One imported bookmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    pub folder: Option<String>,
}

/// One imported history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visited_at: chrono::DateTime<chrono::Utc>,
}

/// Detect available browser profiles on the local machine.
#[tauri::command]
pub async fn browser_detect_profiles() -> AppResult<Vec<BrowserProfile>> {
    Err(AppError::Internal(
        "browser_import_oneshot::browser_detect_profiles not implemented in Phase 1".into(),
    ))
}

/// Import all bookmarks from the given browser profile.
#[tauri::command]
pub async fn browser_import_bookmarks(_profile: BrowserProfile) -> AppResult<Vec<Bookmark>> {
    Err(AppError::Internal(
        "browser_import_oneshot::browser_import_bookmarks not implemented in Phase 1".into(),
    ))
}

/// Import history entries (most-recent first, capped at `limit`) from a profile.
#[tauri::command]
pub async fn browser_import_history(
    _profile: BrowserProfile,
    _limit: u32,
) -> AppResult<Vec<HistoryEntry>> {
    Err(AppError::Internal(
        "browser_import_oneshot::browser_import_history not implemented in Phase 1".into(),
    ))
}
