// SPDX-License-Identifier: MIT
//! Singleton SQLite connection + schema bootstrap.  rusqlite connections
//! are `!Sync`, but our Tauri commands are async + may be invoked
//! concurrently — the Mutex serialises writes (FTS5 doesn't tolerate
//! concurrent writers anyway).

use crate::error::{AppError, AppResult};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Process-wide singleton connection — guarded by a `Mutex`.
pub(super) static CONN: Mutex<Option<Connection>> = Mutex::new(None);

pub(super) fn knowledge_db_path() -> AppResult<PathBuf> {
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

pub(super) fn open_or_create(path: &Path) -> AppResult<Connection> {
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

/// Acquire the singleton connection, lazily opening the on-disk DB on
/// first use.  All public Tauri commands flow through this so the FTS5
/// writer mutex is held for the duration of one operation.
pub(super) fn with_conn<F, R>(f: F) -> AppResult<R>
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
