// SPDX-License-Identifier: MIT
//! Lightweight episodic / procedural / observational memory store. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED #7.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: SQLite + BM25 backed memory store.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Categorisation of a stored memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Episodic,
    Procedural,
    Observational,
    Zettelkasten,
}

/// One stored memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub kind: MemoryKind,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// One BM25-scored search result over the memory store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bm25Result {
    pub entry: MemoryEntry,
    pub score: f64,
}

/// Persist a new memory entry.
#[tauri::command]
pub async fn memory_store(_entry: MemoryEntry) -> AppResult<()> {
    Err(AppError::Internal(
        "memory_lite::memory_store not implemented in Phase 1".into(),
    ))
}

/// Run a BM25 search over the memory store, optionally filtered by kind.
#[tauri::command]
pub async fn memory_search(
    _query: String,
    _kind: Option<MemoryKind>,
    _limit: u32,
) -> AppResult<Vec<Bm25Result>> {
    Err(AppError::Internal(
        "memory_lite::memory_search not implemented in Phase 1".into(),
    ))
}

/// Recall the most recent memories, optionally filtered by kind.
#[tauri::command]
pub async fn memory_recall_recent(
    _kind: Option<MemoryKind>,
    _limit: u32,
) -> AppResult<Vec<MemoryEntry>> {
    Err(AppError::Internal(
        "memory_lite::memory_recall_recent not implemented in Phase 1".into(),
    ))
}
