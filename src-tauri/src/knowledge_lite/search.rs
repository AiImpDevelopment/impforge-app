// SPDX-License-Identifier: MIT
//! Hybrid search: BM25 over both FTS5 tables fused via Reciprocal Rank
//! Fusion, plus PDF page rendering for citation previews.

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::path::Path;

use super::types::{KnowledgeEntry, SearchResult};

/// Run BM25 over the porter virtual table.  Returns `(chunk_id, score)`
/// pairs ordered ascending by BM25 score (lowest = best match).
pub(super) fn run_porter(conn: &Connection, query: &str, limit: u32) -> AppResult<Vec<i64>> {
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

pub(super) fn run_trigram(conn: &Connection, query: &str, limit: u32) -> AppResult<Vec<i64>> {
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

    let fused = rrf::fuse_top(
        &[porter_ids.clone(), trigram_ids.clone()],
        rrf::DEFAULT_K,
        limit as usize,
    );

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

pub(super) fn strip_fts_operators(q: &str) -> String {
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

pub(super) fn make_snippet(text: &str, max: usize) -> String {
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
pub(super) fn render_pdf_page_png(pdf_path: &Path, page_idx: usize) -> Option<Vec<u8>> {
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
