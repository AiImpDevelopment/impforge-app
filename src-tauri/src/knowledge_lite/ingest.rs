// SPDX-License-Identifier: MIT
//! Ingest pipeline — turns a parsed document into rows in `documents`,
//! `chunks_porter`, and `chunks_trigram`.  Enforces the MIT-tier byte
//! and chunk caps before any data lands on disk.

use crate::document_parse::{parse_file, ParseResult};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::path::Path;
use std::time::SystemTime;

use super::chunk::{chunk_text, file_hash};
use super::schema::with_conn;
use super::types::{IngestOutcome, MIT_MAX_BYTES, MIT_MAX_CHUNKS};

/// Internal ingest — used by the public path + tests.
pub(super) fn ingest_one(
    conn: &mut Connection,
    path: &Path,
    parse: ParseResult,
) -> AppResult<IngestOutcome> {
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

/// Synchronous (blocking) ingest helper — used by Feature 3's
/// `auto_digest_lite` so the watcher / RSS daemon can ingest without
/// requiring a tokio runtime.  Same body as `knowledge_ingest_path`
/// minus the `tauri::command` machinery.
pub fn ingest_path_blocking(path: &Path) -> AppResult<IngestOutcome> {
    if !path.exists() {
        return Err(AppError::NotFound(path.display().to_string()));
    }
    let parse = parse_file(path)?;
    with_conn(|conn| ingest_one(conn, path, parse))
}
