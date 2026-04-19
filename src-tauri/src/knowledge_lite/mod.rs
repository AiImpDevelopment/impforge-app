// SPDX-License-Identifier: MIT
//! SQLite FTS5-backed knowledge index with Reciprocal Rank Fusion hybrid retrieval.
//!
//! Tier 2 (impforge-app) of Feature 2: Document Upload + RAG.
//!
//! ## Why a dual FTS5 table layout?
//!
//! Single-tokeniser FTS5 has known weaknesses:
//! * `porter unicode61` is great for stemmed English but butchers German
//!   compounds and stops working entirely on partial words.
//! * `trigram` handles partial matches and Umlaute (`ä`/`ö`/`ü`/`ß`) by
//!   construction but loses morphological grouping.
//!
//! Best-of-both: index every chunk into BOTH virtual tables and merge
//! the rankings via Reciprocal Rank Fusion at query time.  This is the
//! same architecture LanceDB / Elasticsearch / Pinecone use, and it's
//! the moat the major editor-AIs (Cursor, Continue) don't ship even
//! paid.  We ship it free.
//!
//! ## Hard limits (MIT FREE tier)
//!
//! * 50 MB total ingested bytes
//! * 5 000 000 chunks
//!
//! Beyond either: explicit error pointing at ImpForge Pro.
//!
//! ## Privacy
//!
//! Database lives at `~/.impforge-app/knowledge.db` — every byte stays
//! on the user's disk.  No content uploaded.  Per REGEL 000-BRIDGE-NOT-PROCESS.
//!
//! ## Crown-Jewel directory layout (Phase-2 split)
//!
//! - [`types`]    — `KnowledgeEntry`, `SearchResult`, etc. + tier limits
//! - [`schema`]   — singleton `CONN` + `open_or_create` + `with_conn`
//! - [`chunk`]    — pure text chunking + line index + file hash
//! - [`ingest`]   — `ingest_one` + `ingest_path_blocking` (helper for digest)
//! - [`search`]   — `hybrid_search` + RRF fusion + PDF page render
//! - [`commands`] — 9 Tauri commands wired into `lib.rs`
//! - [`tests`]    — behavioural tests (`cfg(test)` only)

pub mod chunk;
pub mod commands;
pub mod ingest;
pub mod schema;
pub mod search;
pub mod types;

#[cfg(test)]
mod tests;

// ─── Public surface re-exports (backward-compat with pre-split paths) ────
//
// External callers (`auto_digest_lite::feed`, `auto_digest_lite::watcher`,
// `digest_browser`, `digest_clipboard`, `digest_screenshots`) `use
// crate::knowledge_lite::*` — every symbol they use must still resolve
// through `crate::knowledge_lite`.

// Type surface.
pub use types::{
    ChunkSlice, CitationPreview, IngestOutcome, KnowledgeEntry, KnowledgeStats,
    SearchResult, MIT_MAX_BYTES, MIT_MAX_CHUNKS,
};

// Pure helpers.
pub use chunk::chunk_text;

// Search engine.
pub use search::hybrid_search;

// Ingest helpers.  `ingest_path_blocking` is the cross-module API.
pub use ingest::ingest_path_blocking;

// Tauri commands (registered in `lib.rs`).
pub use commands::{
    knowledge_count, knowledge_delete_doc, knowledge_get_citation,
    knowledge_ingest_dir, knowledge_ingest_path, knowledge_insert,
    knowledge_pro_teaser_count, knowledge_search, knowledge_stats,
};
