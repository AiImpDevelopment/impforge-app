// SPDX-License-Identifier: MIT
//! Tauri command surface for `knowledge_lite`.  Every wire-level
//! operation the frontend invokes lives here so the rest of the
//! module stays Tauri-free and unit-testable.

use crate::document_parse::{detect_format_path, parse_file, DocumentFormat, ParseResult};
use crate::error::{AppError, AppResult};
use rusqlite::params;
use std::path::PathBuf;
use walkdir::WalkDir;

use super::ingest::ingest_one;
use super::schema::with_conn;
use super::search::{hybrid_search, render_pdf_page_png};
use super::types::{
    CitationPreview, IngestOutcome, KnowledgeEntry, KnowledgeStats, SearchResult, MIT_MAX_BYTES,
};

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
