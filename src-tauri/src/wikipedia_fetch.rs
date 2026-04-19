// SPDX-License-Identifier: MIT
//! Wikipedia search + article fetch. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.1 SHIP-AS-IS-EQUIVALENT.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real reqwest calls against the public Wikipedia REST API.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// One fetched Wikipedia article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikipediaArticle {
    pub title: String,
    pub summary: String,
    pub body: String,
    pub url: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// One search hit returned by the Wikipedia search API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikipediaSearchHit {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

/// Search Wikipedia for the given query, returning up to `limit` hits.
#[tauri::command]
pub async fn wikipedia_search(
    _query: String,
    _limit: u32,
) -> AppResult<Vec<WikipediaSearchHit>> {
    Err(AppError::Internal(
        "wikipedia_fetch::wikipedia_search not implemented in Phase 1".into(),
    ))
}

/// Fetch the full Wikipedia article identified by `title`.
#[tauri::command]
pub async fn wikipedia_fetch_article(_title: String) -> AppResult<WikipediaArticle> {
    Err(AppError::Internal(
        "wikipedia_fetch::wikipedia_fetch_article not implemented in Phase 1".into(),
    ))
}
