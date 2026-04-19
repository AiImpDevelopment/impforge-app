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

use crate::document_parse::{detect_format_path, parse_file, DocumentFormat, ParseResult};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;
use walkdir::WalkDir;

/// Hard limit on total ingested bytes for the MIT FREE tier.
pub const MIT_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Hard limit on total chunks for the MIT FREE tier.
pub const MIT_MAX_CHUNKS: i64 = 5_000_000;

/// Default chunk window in characters (≈ 256-512 tokens).
const CHUNK_CHARS: usize = 1500;
/// Overlap between adjacent chunks (~12% — keeps cross-boundary thoughts intact).
const CHUNK_OVERLAP_CHARS: usize = 180;

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

// ─── Connection management ───────────────────────────────────────────────

/// Singleton connection guarded by a `Mutex`.  rusqlite connections are
/// `!Sync`, but our Tauri commands are async + can be invoked
/// concurrently — the Mutex serialises writes (FTS5 doesn't tolerate
/// concurrent writers anyway).
static CONN: Mutex<Option<Connection>> = Mutex::new(None);

fn knowledge_db_path() -> AppResult<PathBuf> {
    let dir = if let Ok(custom) = std::env::var("IMPFORGE_APP_HOME") {
        PathBuf::from(custom)
    } else {
        let home = dirs::home_dir().ok_or_else(|| {
            AppError::Internal("home directory not resolvable".into())
        })?;
        home.join(".impforge-app")
    };
    std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
    Ok(dir.join("knowledge.db"))
}

fn open_or_create(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|e| AppError::Internal(format!("sqlite open: {e}")))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| AppError::Internal(format!("pragma WAL: {e}")))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| AppError::Internal(format!("pragma sync: {e}")))?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS documents (
            id           INTEGER PRIMARY KEY,
            path         TEXT UNIQUE NOT NULL,
            format       TEXT NOT NULL,
            title        TEXT,
            language     TEXT,
            ingested_at  INTEGER NOT NULL,
            hash         TEXT NOT NULL,
            size_bytes   INTEGER NOT NULL,
            page_count   INTEGER NOT NULL DEFAULT 1,
            heading_path TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_documents_hash ON documents(hash);
        CREATE INDEX IF NOT EXISTS idx_documents_lang ON documents(language);

        -- Porter+unicode61 — stemmed English, prefix indexed for fast
        -- "imp" → "impforge" expansions.
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_porter USING fts5(
            text,
            heading_path,
            doc_id     UNINDEXED,
            chunk_idx  UNINDEXED,
            line_start UNINDEXED,
            line_end   UNINDEXED,
            tokenize='porter unicode61',
            prefix='3'
        );

        -- Trigram — handles German Umlaute (ä/ö/ü/ß), partial matches,
        -- typos.  Slower index but indispensable for bilingual quality.
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_trigram USING fts5(
            text,
            doc_id     UNINDEXED,
            chunk_idx  UNINDEXED,
            line_start UNINDEXED,
            line_end   UNINDEXED,
            tokenize='trigram'
        );
        "#,
    )
    .map_err(|e| AppError::Internal(format!("schema: {e}")))?;

    Ok(conn)
}

fn with_conn<F, R>(f: F) -> AppResult<R>
where
    F: FnOnce(&mut Connection) -> AppResult<R>,
{
    let mut guard = CONN
        .lock()
        .map_err(|_| AppError::Internal("knowledge db mutex poisoned".into()))?;
    if guard.is_none() {
        let path = knowledge_db_path()?;
        let conn = open_or_create(&path)?;
        *guard = Some(conn);
    }
    let conn = guard
        .as_mut()
        .ok_or_else(|| AppError::Internal("knowledge db not initialised".into()))?;
    f(conn)
}

// ─── Chunking ────────────────────────────────────────────────────────────

/// Split text into char-bounded chunks with overlap.  Preserves
/// 1-based `(start_line, end_line)` for citation.
///
/// Strategy:
/// 1. Walk the text by `CHUNK_CHARS` chars.
/// 2. Try to back off the boundary to the nearest paragraph break (`\n\n`)
///    so we don't slice a sentence.
/// 3. Emit `CHUNK_OVERLAP_CHARS` of context from the previous chunk into
///    the next — keeps cross-boundary thoughts findable.
pub fn chunk_text(text: &str) -> Vec<ChunkSlice> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<ChunkSlice> = Vec::new();
    let mut start = 0usize;

    // Pre-compute byte→line lookup for citation.  We track 1-based lines.
    let line_at = build_line_index(text);

    while start < chars.len() {
        let mut end = (start + CHUNK_CHARS).min(chars.len());

        // Try to back off to a paragraph break within last 200 chars.
        if end < chars.len() {
            let lookback_min = end.saturating_sub(200);
            if let Some(better) =
                find_paragraph_break(&chars, lookback_min, end)
            {
                end = better;
            }
        }

        let slice: String = chars[start..end].iter().collect();
        let trimmed = slice.trim().to_string();
        if !trimmed.is_empty() {
            let byte_start = byte_offset(&chars, start);
            let byte_end = byte_offset(&chars, end);
            let line_start = lookup_line(&line_at, byte_start);
            let line_end = lookup_line(&line_at, byte_end.saturating_sub(1));
            out.push(ChunkSlice {
                text: trimmed,
                line_start,
                line_end,
            });
        }

        // Advance with overlap.
        if end >= chars.len() {
            break;
        }
        let next = end.saturating_sub(CHUNK_OVERLAP_CHARS);
        start = if next > start { next } else { end };
    }
    out
}

/// One chunk produced by [`chunk_text`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSlice {
    pub text: String,
    pub line_start: i64,
    pub line_end: i64,
}

fn build_line_index(text: &str) -> Vec<usize> {
    // Returns ascending byte-offsets where new lines start (the offset
    // immediately AFTER each `\n`).  We add a sentinel 0 so line 1
    // begins at byte 0.
    let mut offsets = vec![0usize];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn lookup_line(line_offsets: &[usize], byte_offset: usize) -> i64 {
    // Binary-search for the largest offset ≤ byte_offset.
    match line_offsets.binary_search(&byte_offset) {
        Ok(idx) => (idx + 1) as i64,
        Err(idx) => idx as i64, // idx is the insertion point; line = idx (1-based starts at sentinel 0)
    }
    .max(1)
}

fn byte_offset(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

fn find_paragraph_break(chars: &[char], min: usize, max: usize) -> Option<usize> {
    if max < 2 {
        return None;
    }
    let limit = max.min(chars.len());
    if min >= limit {
        return None;
    }
    for i in (min..limit - 1).rev() {
        if chars[i] == '\n' && chars[i + 1] == '\n' {
            return Some(i + 2);
        }
    }
    None
}

// ─── Hashing ─────────────────────────────────────────────────────────────

fn file_hash(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path).map_err(AppError::Io)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

// ─── Ingest pipeline ─────────────────────────────────────────────────────

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

/// Internal ingest — used by the public path + tests.
fn ingest_one(conn: &mut Connection, path: &Path, parse: ParseResult) -> AppResult<IngestOutcome> {
    let canonical = path.canonicalize().map_err(AppError::Io)?;
    let path_str = canonical.to_string_lossy().to_string();
    let hash = file_hash(&canonical)?;
    let size = std::fs::metadata(&canonical).map_err(AppError::Io)?.len();

    // Dedup by hash — short-circuit if the exact bytes are already present.
    let existing_by_hash: Option<i64> = conn
        .query_row(
            "SELECT id FROM documents WHERE hash = ?1",
            params![&hash],
            |row| row.get(0),
        )
        .ok();
    if let Some(doc_id) = existing_by_hash {
        return Ok(IngestOutcome {
            doc_id,
            path: path_str,
            format: parse.format.as_str().to_string(),
            language: parse.language,
            chunk_count: 0,
            bytes: size as i64,
            skipped_duplicate: true,
        });
    }

    // Hard limits — count BEFORE writing.
    let bytes_total: i64 = conn
        .query_row("SELECT COALESCE(SUM(size_bytes), 0) FROM documents", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    if (bytes_total as u64) + size > MIT_MAX_BYTES {
        return Err(AppError::InvalidArgument(format!(
            "MIT FREE tier limit reached: {} MB indexed (cap = {} MB). \
             Upgrade to ImpForge Pro for unlimited ingest, vector + KG retrieval. \
             https://impforge.com",
            (bytes_total + size as i64) / (1024 * 1024),
            MIT_MAX_BYTES / (1024 * 1024),
        )));
    }
    let chunk_total: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks_porter", [], |row| row.get(0))
        .unwrap_or(0);
    if chunk_total >= MIT_MAX_CHUNKS {
        return Err(AppError::InvalidArgument(format!(
            "MIT FREE tier limit reached: {} chunks (cap = {}). \
             Upgrade to ImpForge Pro. https://impforge.com",
            chunk_total, MIT_MAX_CHUNKS,
        )));
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let chunks = chunk_text(&parse.text);
    let heading_path = parse.headings.join(" › ");

    let tx = conn
        .transaction()
        .map_err(|e| AppError::Internal(format!("tx start: {e}")))?;

    // Upsert by path.
    let existing_by_path: Option<i64> = tx
        .query_row(
            "SELECT id FROM documents WHERE path = ?1",
            params![&path_str],
            |row| row.get(0),
        )
        .ok();
    let doc_id = if let Some(id) = existing_by_path {
        tx.execute("DELETE FROM chunks_porter WHERE doc_id = ?1", params![id])
            .map_err(|e| AppError::Internal(format!("clear porter: {e}")))?;
        tx.execute("DELETE FROM chunks_trigram WHERE doc_id = ?1", params![id])
            .map_err(|e| AppError::Internal(format!("clear trigram: {e}")))?;
        tx.execute(
            "UPDATE documents SET format=?1, title=?2, language=?3, ingested_at=?4, \
             hash=?5, size_bytes=?6, page_count=?7, heading_path=?8 WHERE id=?9",
            params![
                parse.format.as_str(),
                &parse.title,
                &parse.language,
                now,
                &hash,
                size as i64,
                parse.page_count as i64,
                &heading_path,
                id,
            ],
        )
        .map_err(|e| AppError::Internal(format!("update doc: {e}")))?;
        id
    } else {
        tx.execute(
            "INSERT INTO documents(path, format, title, language, ingested_at, \
             hash, size_bytes, page_count, heading_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &path_str,
                parse.format.as_str(),
                &parse.title,
                &parse.language,
                now,
                &hash,
                size as i64,
                parse.page_count as i64,
                &heading_path,
            ],
        )
        .map_err(|e| AppError::Internal(format!("insert doc: {e}")))?;
        tx.last_insert_rowid()
    };

    let mut chunk_count = 0i64;
    {
        let mut porter_stmt = tx
            .prepare(
                "INSERT INTO chunks_porter(text, heading_path, doc_id, chunk_idx, \
                 line_start, line_end) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|e| AppError::Internal(format!("prepare porter: {e}")))?;
        let mut trigram_stmt = tx
            .prepare(
                "INSERT INTO chunks_trigram(text, doc_id, chunk_idx, line_start, line_end) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|e| AppError::Internal(format!("prepare trigram: {e}")))?;
        for (idx, chunk) in chunks.iter().enumerate() {
            porter_stmt
                .execute(params![
                    &chunk.text,
                    &heading_path,
                    doc_id,
                    idx as i64,
                    chunk.line_start,
                    chunk.line_end,
                ])
                .map_err(|e| AppError::Internal(format!("insert porter: {e}")))?;
            trigram_stmt
                .execute(params![
                    &chunk.text,
                    doc_id,
                    idx as i64,
                    chunk.line_start,
                    chunk.line_end,
                ])
                .map_err(|e| AppError::Internal(format!("insert trigram: {e}")))?;
            chunk_count += 1;
        }
    }
    tx.commit()
        .map_err(|e| AppError::Internal(format!("tx commit: {e}")))?;

    Ok(IngestOutcome {
        doc_id,
        path: path_str,
        format: parse.format.as_str().to_string(),
        language: parse.language,
        chunk_count,
        bytes: size as i64,
        skipped_duplicate: false,
    })
}

// ─── Hybrid search ───────────────────────────────────────────────────────

/// Run BM25 over the porter virtual table.  Returns `(chunk_id, score)`
/// pairs ordered ascending by BM25 score (lowest = best match).
fn run_porter(conn: &Connection, query: &str, limit: u32) -> AppResult<Vec<i64>> {
    let mut stmt = conn
        .prepare(
            "SELECT rowid FROM chunks_porter \
             WHERE chunks_porter MATCH ?1 \
             ORDER BY bm25(chunks_porter, 4.0, 1.0) ASC \
             LIMIT ?2",
        )
        .map_err(|e| AppError::Internal(format!("prepare porter query: {e}")))?;
    let rows: Vec<i64> = stmt
        .query_map(params![query, limit as i64 * 2], |row| row.get(0))
        .map_err(|e| AppError::Internal(format!("query porter: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Internal(format!("collect porter: {e}")))?;
    Ok(rows)
}

fn run_trigram(conn: &Connection, query: &str, limit: u32) -> AppResult<Vec<i64>> {
    let mut stmt = conn
        .prepare(
            "SELECT rowid FROM chunks_trigram \
             WHERE chunks_trigram MATCH ?1 \
             ORDER BY bm25(chunks_trigram) ASC \
             LIMIT ?2",
        )
        .map_err(|e| AppError::Internal(format!("prepare trigram query: {e}")))?;
    let rows: Vec<i64> = stmt
        .query_map(params![query, limit as i64 * 2], |row| row.get(0))
        .map_err(|e| AppError::Internal(format!("query trigram: {e}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Internal(format!("collect trigram: {e}")))?;
    Ok(rows)
}

/// Hybrid search: porter ⊕ trigram, fused with RRF.
pub fn hybrid_search(conn: &Connection, query: &str, limit: u32) -> AppResult<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    // Porter accepts a normal FTS5 query string directly.  Trigram needs
    // the SAME literal query because tokenisation is automatic.  We
    // intentionally avoid re-quoting — users get FTS5 syntax (AND/OR/NOT).
    // For trigram we strip syntax operators, since trigram tokeniser
    // can't parse `AND`/`OR`/`NOT` safely.
    let trigram_q = strip_fts_operators(query);

    let porter_ids = run_porter(conn, query, limit).unwrap_or_default();
    let trigram_ids = if trigram_q.is_empty() {
        Vec::new()
    } else {
        run_trigram(conn, &trigram_q, limit).unwrap_or_default()
    };

    let fused = rrf::fuse_top(&[porter_ids.clone(), trigram_ids.clone()], rrf::DEFAULT_K, limit as usize);

    if fused.is_empty() {
        return Ok(Vec::new());
    }

    // Hydrate chunk + document data.  We pick the porter table when the
    // chunk exists there, else trigram.
    let mut results = Vec::with_capacity(fused.len());
    for (rowid, score) in fused {
        if let Some(hit) = hydrate_hit(conn, rowid, score, &porter_ids, &trigram_ids)? {
            results.push(hit);
        }
    }
    Ok(results)
}

fn strip_fts_operators(q: &str) -> String {
    q.split_whitespace()
        .filter(|tok| {
            let upper = tok.to_uppercase();
            upper != "AND" && upper != "OR" && upper != "NOT"
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn hydrate_hit(
    conn: &Connection,
    rowid: i64,
    rrf_score: f64,
    porter_ids: &[i64],
    trigram_ids: &[i64],
) -> AppResult<Option<SearchResult>> {
    // Lookup chunk in porter first; trigram chunks share rowid space but
    // are stored in a different virtual table.
    let row = conn
        .query_row(
            "SELECT cp.text, cp.heading_path, cp.doc_id, cp.line_start, cp.line_end, \
                    d.path, d.title, d.format, d.ingested_at \
             FROM chunks_porter cp \
             JOIN documents d ON d.id = cp.doc_id \
             WHERE cp.rowid = ?1",
            params![rowid],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )
        .ok()
        .or_else(|| {
            // Try trigram table — separate rowid space.
            conn.query_row(
                "SELECT ct.text, '', ct.doc_id, ct.line_start, ct.line_end, \
                        d.path, d.title, d.format, d.ingested_at \
                 FROM chunks_trigram ct \
                 JOIN documents d ON d.id = ct.doc_id \
                 WHERE ct.rowid = ?1",
                params![rowid],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .ok()
        });

    let Some((text, _heading_path, doc_id, line_start, line_end, path, title, format, ingested_at)) =
        row
    else {
        return Ok(None);
    };

    let snippet = make_snippet(&text, 240);
    let porter_rank = porter_ids.iter().position(|&r| r == rowid).map(|p| p + 1);
    let trigram_rank = trigram_ids.iter().position(|&r| r == rowid).map(|p| p + 1);

    Ok(Some(SearchResult {
        entry: KnowledgeEntry {
            id: doc_id.to_string(),
            source: path,
            title,
            body: text.clone(),
            ingested_at: chrono::DateTime::<chrono::Utc>::from_timestamp(ingested_at, 0)
                .unwrap_or_else(chrono::Utc::now),
        },
        rank: rrf_score,
        snippet,
        sub_scores: serde_json::json!({
            "porter_rank": porter_rank,
            "trigram_rank": trigram_rank,
            "format": format,
        }),
        line_start,
        line_end,
    }))
}

fn make_snippet(text: &str, max: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let truncated: String = collapsed.chars().take(max).collect();
    format!("{}…", truncated)
}

// ─── PDF citation preview ────────────────────────────────────────────────

/// Render a single PDF page to PNG bytes via pdfium-render.  Returns
/// `None` if pdfium isn't available on the host (graceful fallback).
///
/// pdfium is dynamically linked — if the user doesn't have libpdfium.so /
/// pdfium.dll on PATH, we silently skip the bitmap and the UI shows
/// text-only citation.  This keeps cargo-install working on every
/// platform without requiring the heavy pdfium native dep at build time.
fn render_pdf_page_png(pdf_path: &Path, page_idx: usize) -> Option<Vec<u8>> {
    use pdfium_render::prelude::*;
    let bindings = Pdfium::bind_to_system_library().ok()?;
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium.load_pdf_from_file(pdf_path, None).ok()?;
    let page = doc.pages().get(page_idx as u16 as i32).ok()?;
    let render_cfg = PdfRenderConfig::new()
        .set_target_width(720)
        .render_form_data(false);
    let bitmap = page.render_with_config(&render_cfg).ok()?;
    let dyn_img = bitmap.as_image().ok()?;
    let mut buf = std::io::Cursor::new(Vec::new());
    dyn_img
        .write_to(&mut buf, image::ImageFormat::Png)
        .ok()?;
    Some(buf.into_inner())
}

// ─── Tauri commands (replace Phase-1 stubs + add new) ────────────────────

/// Ingest one entry into the knowledge index.  Treats `body` as the raw
/// text of an in-memory document (e.g. user-typed note).  For file
/// ingest use [`knowledge_ingest_path`].
#[tauri::command]
pub async fn knowledge_insert(entry: KnowledgeEntry) -> AppResult<()> {
    with_conn(|conn| {
        let parse = ParseResult {
            format: DocumentFormat::Plaintext,
            text: entry.body.clone(),
            title: entry.title.clone(),
            language: crate::document_parse::detect_language(&entry.body),
            page_count: 1,
            headings: Vec::new(),
            extra: serde_json::json!({}),
        };
        let temp_dir = std::env::temp_dir().join("impforge-app-virtual");
        std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;
        let path = temp_dir.join(format!("{}.txt", entry.id));
        std::fs::write(&path, &entry.body).map_err(AppError::Io)?;
        ingest_one(conn, &path, parse).map(|_| ())
    })
}

/// Ranked hybrid search via Reciprocal Rank Fusion (porter ⊕ trigram).
#[tauri::command]
pub async fn knowledge_search(query: String, limit: u32) -> AppResult<Vec<SearchResult>> {
    with_conn(|conn| hybrid_search(conn, &query, limit.clamp(1, 200)))
}

/// Total number of indexed documents.
#[tauri::command]
pub async fn knowledge_count() -> AppResult<u64> {
    with_conn(|conn| {
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
            .map_err(|e| AppError::Internal(format!("count: {e}")))?;
        Ok(n as u64)
    })
}

/// Ingest a single file from disk.  Returns the outcome including the
/// `doc_id` for downstream UI use.
#[tauri::command]
pub async fn knowledge_ingest_path(path: String) -> AppResult<IngestOutcome> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(AppError::NotFound(path));
    }
    let parse = parse_file(&p)?;
    with_conn(|conn| ingest_one(conn, &p, parse))
}

/// Walk a directory and ingest every supported file.  When `recursive`
/// is false, only the top-level directory entries are visited.
#[tauri::command]
pub async fn knowledge_ingest_dir(path: String, recursive: bool) -> AppResult<Vec<IngestOutcome>> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(AppError::InvalidArgument(format!("not a directory: {path}")));
    }
    let walker = if recursive {
        WalkDir::new(&dir).into_iter()
    } else {
        WalkDir::new(&dir).max_depth(1).into_iter()
    };
    let mut results = Vec::new();
    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path().to_path_buf();
        let fmt = detect_format_path(&p);
        if matches!(
            fmt,
            DocumentFormat::Unknown
                | DocumentFormat::Pptx
                | DocumentFormat::Odt
                | DocumentFormat::Ods
                | DocumentFormat::Odp
        ) {
            continue;
        }
        let parse = match parse_file(&p) {
            Ok(pr) => pr,
            Err(e) => {
                tracing::warn!("skip {}: {}", p.display(), e);
                continue;
            }
        };
        let outcome = with_conn(|conn| ingest_one(conn, &p, parse))?;
        results.push(outcome);
    }
    Ok(results)
}

/// Remove a document by `doc_id`.  Cascades to all chunks.
#[tauri::command]
pub async fn knowledge_delete_doc(doc_id: i64) -> AppResult<()> {
    with_conn(|conn| {
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Internal(format!("delete tx: {e}")))?;
        tx.execute("DELETE FROM chunks_porter WHERE doc_id = ?1", params![doc_id])
            .map_err(|e| AppError::Internal(format!("clear porter: {e}")))?;
        tx.execute("DELETE FROM chunks_trigram WHERE doc_id = ?1", params![doc_id])
            .map_err(|e| AppError::Internal(format!("clear trigram: {e}")))?;
        tx.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
            .map_err(|e| AppError::Internal(format!("delete doc: {e}")))?;
        tx.commit()
            .map_err(|e| AppError::Internal(format!("commit: {e}")))?;
        Ok(())
    })
}

/// Hydrate a citation preview from `(doc_id, chunk_idx)`.
///
/// For PDFs, attempts a pdfium-render pixel snapshot of the chunk's
/// starting page.  Falls back gracefully to text-only when pdfium is
/// not installed on the host.
#[tauri::command]
pub async fn knowledge_get_citation(doc_id: i64, chunk_idx: i64) -> AppResult<CitationPreview> {
    with_conn(|conn| {
        let (path, format, line_start, line_end, text): (String, String, i64, i64, String) = conn
            .query_row(
                "SELECT d.path, d.format, cp.line_start, cp.line_end, cp.text \
                 FROM chunks_porter cp JOIN documents d ON d.id = cp.doc_id \
                 WHERE cp.doc_id = ?1 AND cp.chunk_idx = ?2",
                params![doc_id, chunk_idx],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .map_err(|e| AppError::NotFound(format!("citation lookup: {e}")))?;

        let mut preview = CitationPreview {
            doc_path: path.clone(),
            format: format.clone(),
            line_start,
            line_end,
            text,
            page_image_png_b64: None,
        };

        if format == "pdf" {
            // Heuristic: page index ~ chunk_idx mapped against chunk_count.
            // For the MIT tier this is an approximation — Pro adds true
            // text-to-page anchoring during ingest.
            let pdf_path = PathBuf::from(&path);
            if pdf_path.exists() {
                let page_idx = chunk_idx.max(0) as usize;
                if let Some(png) = render_pdf_page_png(&pdf_path, page_idx) {
                    use base64::Engine;
                    preview.page_image_png_b64 =
                        Some(base64::engine::general_purpose::STANDARD.encode(png));
                }
            }
        }
        Ok(preview)
    })
}

/// Aggregate stats — used by the dashboard tile and the upgrade nudge.
#[tauri::command]
pub async fn knowledge_stats() -> AppResult<KnowledgeStats> {
    with_conn(|conn| {
        let document_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
            .unwrap_or(0);
        let chunk_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks_porter", [], |r| r.get(0))
            .unwrap_or(0);
        let total_bytes: i64 = conn
            .query_row("SELECT COALESCE(SUM(size_bytes), 0) FROM documents", [], |r| r.get(0))
            .unwrap_or(0);

        let mut langs = Vec::new();
        let mut stmt = conn
            .prepare("SELECT language, COUNT(*) FROM documents GROUP BY language")
            .map_err(|e| AppError::Internal(format!("stats prep: {e}")))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| AppError::Internal(format!("stats query: {e}")))?;
        while let Some(row) = rows
            .next()
            .map_err(|e| AppError::Internal(format!("stats row: {e}")))?
        {
            let lang: String = row.get(0).unwrap_or_else(|_| "und".into());
            let count: i64 = row.get(1).unwrap_or(0);
            langs.push((lang, count));
        }
        let remaining_bytes = (MIT_MAX_BYTES as i64).saturating_sub(total_bytes);
        let avg_doc_size = if document_count > 0 {
            (total_bytes / document_count).max(1)
        } else {
            32_768
        };
        let remaining_documents_estimate = remaining_bytes / avg_doc_size;

        Ok(KnowledgeStats {
            document_count,
            chunk_count,
            total_bytes,
            languages: langs,
            remaining_documents_estimate,
            remaining_bytes,
        })
    })
}

/// Count how many vector-only matches Pro would find for `query`.
///
/// At the MIT tier we have NO embeddings, so this is always 0.  But
/// the command exists so the frontend can show "+N Pro-only matches"
/// teaser badge once Pro is installed (Pro overrides this command via
/// its `knowledge_pro_*` modules).
#[tauri::command]
pub async fn knowledge_pro_teaser_count(_query: String) -> AppResult<u64> {
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn isolated_home() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var("IMPFORGE_APP_HOME", dir.path());
        // Reset singleton so each test gets a fresh DB.
        if let Ok(mut g) = CONN.lock() {
            *g = None;
        }
        dir
    }

    fn write_temp(suffix: &str, content: &[u8]) -> tempfile::NamedTempFile {
        let tmp = tempfile::NamedTempFile::with_suffix(suffix).expect("tmp");
        tmp.as_file().write_all(content).expect("write");
        tmp.as_file().sync_all().expect("sync");
        tmp
    }

    #[test]
    fn chunk_text_emits_at_least_one_chunk_for_short_text() {
        let chunks = chunk_text("Just a short snippet.");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].line_start, 1);
    }

    #[test]
    fn chunk_text_creates_overlapping_chunks_for_long_text() {
        let line = "This is a sentence with several words. ";
        let big = line.repeat(200); // ~7800 chars → multiple chunks
        let chunks = chunk_text(&big);
        assert!(chunks.len() >= 4, "got {} chunks", chunks.len());
        // Adjacent chunks must share text (overlap).
        if chunks.len() >= 2 {
            let tail = &chunks[0].text[chunks[0].text.len().saturating_sub(50)..];
            let head = &chunks[1].text[..chunks[1].text.len().min(200)];
            // Some of the overlap window should be present in chunk 1's
            // head.  Very loose assertion — just guarantees overlap > 0.
            let overlap_evidence = tail
                .split_whitespace()
                .any(|tok| head.contains(tok) && tok.len() > 4);
            assert!(overlap_evidence, "expected overlap evidence in adjacent chunks");
        }
    }

    #[test]
    fn chunk_text_empty_input_returns_no_chunks() {
        assert!(chunk_text("").is_empty());
        assert!(chunk_text("   \n\n  \n").is_empty());
    }

    #[test]
    fn ingest_real_text_file_creates_doc_and_searchable_chunks() {
        let _home = isolated_home();
        let tmp = write_temp(
            ".txt",
            b"The quick brown fox jumps over the lazy dog.\n\
              Reciprocal rank fusion is the moat that ImpForge ships free.\n\
              Hybrid retrieval merges porter and trigram FTS5 rankings.",
        );
        let parse = parse_file(tmp.path()).expect("parse");
        let outcome = with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");
        assert!(!outcome.skipped_duplicate);
        assert!(outcome.chunk_count >= 1);

        let hits = with_conn(|c| hybrid_search(c, "fox", 10)).expect("search");
        assert!(!hits.is_empty(), "expected at least one fox hit");
        assert!(hits[0].snippet.to_lowercase().contains("fox"));
        assert!(hits[0].entry.title.is_empty() || hits[0].entry.source.contains(".txt"));
    }

    #[test]
    fn ingest_dedups_by_hash() {
        let _home = isolated_home();
        let tmp = write_temp(".txt", b"deduplicate this exact content");
        let parse = parse_file(tmp.path()).expect("parse");
        let first = with_conn(|c| ingest_one(c, tmp.path(), parse.clone())).expect("first");
        assert!(!first.skipped_duplicate);
        let second = with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("second");
        assert!(second.skipped_duplicate);
        assert_eq!(first.doc_id, second.doc_id);
    }

    #[test]
    fn rrf_fusion_ranks_doc_present_in_both_indexes_higher() {
        let _home = isolated_home();
        // Doc A: contains "rust" once.
        let f1 = write_temp(
            ".txt",
            b"rust programming is good.  some other text.",
        );
        // Doc B: contains "rust" three times AND a partial word "rusty".
        let f2 = write_temp(
            ".txt",
            b"rust rust rust is dominant.  this rusty hammer matters too.",
        );
        let p1 = parse_file(f1.path()).expect("p1");
        let p2 = parse_file(f2.path()).expect("p2");
        with_conn(|c| ingest_one(c, f1.path(), p1)).expect("ingest 1");
        with_conn(|c| ingest_one(c, f2.path(), p2)).expect("ingest 2");

        let hits = with_conn(|c| hybrid_search(c, "rust", 10)).expect("search");
        assert!(!hits.is_empty(), "expected at least one hit");
        // RRF should rank doc B (with multiple rust occurrences) ahead.
        let top_path = &hits[0].entry.source;
        let f2_path = f2
            .path()
            .canonicalize()
            .expect("canon")
            .to_string_lossy()
            .to_string();
        assert_eq!(top_path, &f2_path, "doc with 3x rust should rank top");
    }

    #[test]
    fn search_returns_empty_for_unknown_term() {
        let _home = isolated_home();
        let tmp = write_temp(".txt", b"basic content here");
        let parse = parse_file(tmp.path()).expect("parse");
        with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");
        let hits = with_conn(|c| hybrid_search(c, "xyzimpossible1234", 10)).expect("search");
        assert!(hits.is_empty());
    }

    #[test]
    fn search_handles_german_umlauts_via_trigram() {
        let _home = isolated_home();
        let tmp = write_temp(
            ".txt",
            "Über die Bedeutung der Künstlichen Intelligenz heute.\n\
             Wir schätzen Privatsphäre und Datenschutz mehr als alles.\n\
             Diese Datei testet ä ö ü ß im FTS5 Trigramm-Tokenisierer."
                .as_bytes(),
        );
        let parse = parse_file(tmp.path()).expect("parse");
        with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");

        // Trigram tokeniser should match on partial Umlaute strings.
        // The porter tokeniser would butcher 'Künstlich' → 'kunstlich',
        // but trigram's character n-grams match either way.
        let hits = with_conn(|c| hybrid_search(c, "Künstlich", 10)).expect("search");
        assert!(!hits.is_empty(), "expected umlaut hit via trigram");
    }

    #[test]
    fn delete_doc_removes_from_both_tables() {
        let _home = isolated_home();
        let tmp = write_temp(".txt", b"to be deleted soon");
        let parse = parse_file(tmp.path()).expect("parse");
        let outcome = with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");

        // Verify it's there.
        let hits = with_conn(|c| hybrid_search(c, "deleted", 10)).expect("pre");
        assert!(!hits.is_empty());

        with_conn(|c| {
            c.execute("DELETE FROM chunks_porter WHERE doc_id = ?1", params![outcome.doc_id])
                .map_err(|e| AppError::Internal(e.to_string()))?;
            c.execute("DELETE FROM chunks_trigram WHERE doc_id = ?1", params![outcome.doc_id])
                .map_err(|e| AppError::Internal(e.to_string()))?;
            c.execute("DELETE FROM documents WHERE id = ?1", params![outcome.doc_id])
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(())
        })
        .expect("delete");

        let hits = with_conn(|c| hybrid_search(c, "deleted", 10)).expect("post");
        assert!(hits.is_empty(), "post-delete must be empty");
    }

    #[test]
    fn pro_teaser_count_is_zero_at_mit_tier() {
        let _home = isolated_home();
        // Synchronous wrapper for the async tauri command.
        let count = futures::executor::block_on(knowledge_pro_teaser_count("anything".into()))
            .expect("teaser");
        assert_eq!(count, 0);
    }

    #[test]
    fn make_snippet_truncates_long_text() {
        let long = "word ".repeat(200);
        let snip = make_snippet(&long, 50);
        assert!(snip.chars().count() <= 51);
        assert!(snip.ends_with('…'));
    }

    #[test]
    fn make_snippet_keeps_short_text_intact() {
        let snip = make_snippet("short", 240);
        assert_eq!(snip, "short");
    }
}
