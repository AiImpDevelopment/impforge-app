// SPDX-License-Identifier: MIT
//! SQLite FTS5-backed knowledge index. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED #6.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real SQLite FTS5 ingest + ranked search via BM25.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// One ingested knowledge entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub source: String,
    pub title: String,
    pub body: String,
    pub ingested_at: chrono::DateTime<chrono::Utc>,
}

/// One ranked result from a knowledge search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entry: KnowledgeEntry,
    pub rank: f64,
    pub snippet: String,
}

/// Insert a new knowledge entry into the FTS5 index.
#[tauri::command]
pub async fn knowledge_insert(_entry: KnowledgeEntry) -> AppResult<()> {
    Err(AppError::Internal(
        "knowledge_lite::knowledge_insert not implemented in Phase 1".into(),
    ))
}

/// Run a ranked FTS5 search over the knowledge index.
#[tauri::command]
pub async fn knowledge_search(_query: String, _limit: u32) -> AppResult<Vec<SearchResult>> {
    Err(AppError::Internal(
        "knowledge_lite::knowledge_search not implemented in Phase 1".into(),
    ))
}

/// Return the total number of indexed entries.
#[tauri::command]
pub async fn knowledge_count() -> AppResult<u64> {
    Err(AppError::Internal(
        "knowledge_lite::knowledge_count not implemented in Phase 1".into(),
    ))
}
