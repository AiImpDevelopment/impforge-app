// SPDX-License-Identifier: MIT
//! Public type surface of `knowledge_lite` — newtypes, structs, and
//! tier-limit constants.  No I/O, no SQL, no Tauri.

use serde::{Deserialize, Serialize};

/// Hard limit on total ingested bytes for the MIT FREE tier.
pub const MIT_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Hard limit on total chunks for the MIT FREE tier.
pub const MIT_MAX_CHUNKS: i64 = 5_000_000;

/// Default chunk window in characters (≈ 256-512 tokens).
pub(super) const CHUNK_CHARS: usize = 1500;
/// Overlap between adjacent chunks (~12% — keeps cross-boundary thoughts intact).
pub(super) const CHUNK_OVERLAP_CHARS: usize = 180;

// ─── Public types (kept stable across tiers) ─────────────────────────────

/// One ingested knowledge entry.  Shape preserved across tiers so the
/// frontend code is shared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub source: String,
    pub title: String,
    pub body: String,
    pub ingested_at: chrono::DateTime<chrono::Utc>,
}

/// One ranked search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entry: KnowledgeEntry,
    pub rank: f64,
    pub snippet: String,
    /// Per-retriever sub-scores (`porter`, `trigram`).  Useful for the
    /// frontend's "why this result?" tooltip.
    pub sub_scores: serde_json::Value,
    /// 1-based starting line of the matched chunk in the source file.
    pub line_start: i64,
    /// 1-based ending line.
    pub line_end: i64,
}

/// Aggregate stats — used by the dashboard tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStats {
    pub document_count: i64,
    pub chunk_count: i64,
    pub total_bytes: i64,
    pub languages: Vec<(String, i64)>,
    pub remaining_documents_estimate: i64,
    pub remaining_bytes: i64,
}

/// Citation preview returned to the UI for the highlighted chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationPreview {
    /// Source file path.
    pub doc_path: String,
    /// Document format string ("pdf", "markdown", ...).
    pub format: String,
    /// 1-based line range.
    pub line_start: i64,
    pub line_end: i64,
    /// Plain-text excerpt of the chunk for the citation panel.
    pub text: String,
    /// Optional PNG bytes (base64-encoded) for PDF citation previews.
    /// `None` for non-PDF formats.
    pub page_image_png_b64: Option<String>,
}

/// Outcome of one ingest call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOutcome {
    pub doc_id: i64,
    pub path: String,
    pub format: String,
    pub language: String,
    pub chunk_count: i64,
    pub bytes: i64,
    pub skipped_duplicate: bool,
}

/// One chunk produced by [`crate::knowledge_lite::chunk_text`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSlice {
    pub text: String,
    pub line_start: i64,
    pub line_end: i64,
}
