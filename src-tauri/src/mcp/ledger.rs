// SPDX-License-Identifier: MIT
//! Tamper-evident SQLite ledger of every MCP tool invocation.
//!
//! ## Schema
//!
//! ```sql
//! CREATE TABLE invocations (
//!   seq          INTEGER PRIMARY KEY AUTOINCREMENT,
//!   timestamp    TEXT NOT NULL,
//!   server_id    TEXT NOT NULL,
//!   tool_name    TEXT NOT NULL,
//!   args_hash    TEXT NOT NULL,
//!   result_hash  TEXT NOT NULL,
//!   prev_hash    TEXT NOT NULL,
//!   row_hash     TEXT NOT NULL,
//!   success      INTEGER NOT NULL,
//!   latency_ms   INTEGER NOT NULL
//! );
//! ```
//!
//! `row_hash` = SHA-256 over the canonical
//! `(timestamp, server_id, tool_name, args_hash, result_hash, prev_hash)`.
//! `prev_hash` chains every row to the previous one — `verify_chain`
//! recomputes hashes and flags the first broken row.
//!
//! ## What MIT shows
//!
//! Green badge: **Tamper-evident · 1 234 calls**
//!
//! ## What Pro shows on top
//!
//! * Merkle root (computed in Pro's `mcp_pro_audit::merkle_root`)
//! * Ed25519 signature on the rotated chain head
//! * In-toto attestation export

use crate::error::{AppError, AppResult};
use crate::mcp::marketplace;
use crate::mcp::types::{LedgerEntry, LedgerVerification};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;

/// Path to the ledger SQLite DB.
pub fn ledger_db_path() -> AppResult<PathBuf> {
    Ok(marketplace::app_home()?.join("mcp-ledger.db"))
}

/// Genesis hash — used for the very first row's prev_hash.
const GENESIS_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Global ledger connection.
pub(crate) static LEDGER_CONN: Mutex<Option<Connection>> = Mutex::new(None);

fn open_ledger() -> AppResult<()> {
    let mut g = LEDGER_CONN
        .lock()
        .map_err(|e| AppError::Internal(format!("ledger mutex poisoned: {e}")))?;
    if g.is_none() {
        let path = ledger_db_path()?;
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| AppError::Internal(format!("open ledger: {e}")))?;
        conn.execute_batch(
            r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS invocations (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp    TEXT NOT NULL,
    server_id    TEXT NOT NULL,
    tool_name    TEXT NOT NULL,
    args_hash    TEXT NOT NULL,
    result_hash  TEXT NOT NULL,
    prev_hash    TEXT NOT NULL,
    row_hash     TEXT NOT NULL,
    success      INTEGER NOT NULL,
    latency_ms   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_invocations_server
    ON invocations (server_id, timestamp DESC);
"#,
        )
        .map_err(|e| AppError::Internal(format!("init ledger schema: {e}")))?;
        *g = Some(conn);
    }
    Ok(())
}

fn with_ledger<F, R>(f: F) -> AppResult<R>
where
    F: FnOnce(&Connection) -> AppResult<R>,
{
    open_ledger()?;
    let g = LEDGER_CONN
        .lock()
        .map_err(|e| AppError::Internal(format!("ledger mutex poisoned: {e}")))?;
    let conn = g
        .as_ref()
        .ok_or_else(|| AppError::Internal("ledger conn missing".into()))?;
    f(conn)
}

/// SHA-256 hex of a JSON-canonical string.
pub fn hash_json(value: &serde_json::Value) -> String {
    let canonical = canonical_json(value);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex_encode(&hasher.finalize())
}

/// Canonical JSON serialisation — sorts object keys lexicographically
/// and uses default float formatting.  Matches the canonical JSON form
/// used by Sigstore / in-toto.
pub fn canonical_json(value: &serde_json::Value) -> String {
    fn write(value: &serde_json::Value, out: &mut String) {
        use serde_json::Value;
        match value {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => {
                let escaped = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
                out.push_str(&escaped);
            }
            Value::Array(arr) => {
                out.push('[');
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write(v, out);
                }
                out.push(']');
            }
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                out.push('{');
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    let key_escaped = serde_json::to_string(k).unwrap_or_else(|_| "\"\"".into());
                    out.push_str(&key_escaped);
                    out.push(':');
                    if let Some(v) = map.get(*k) {
                        write(v, out);
                    } else {
                        out.push_str("null");
                    }
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    write(value, &mut out);
    out
}

/// Lower-case hex encoding of a byte slice.
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xF) as usize] as char);
    }
    out
}

fn compute_row_hash(
    timestamp: &str,
    server_id: &str,
    tool_name: &str,
    args_hash: &str,
    result_hash: &str,
    prev_hash: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(b"|");
    hasher.update(server_id.as_bytes());
    hasher.update(b"|");
    hasher.update(tool_name.as_bytes());
    hasher.update(b"|");
    hasher.update(args_hash.as_bytes());
    hasher.update(b"|");
    hasher.update(result_hash.as_bytes());
    hasher.update(b"|");
    hasher.update(prev_hash.as_bytes());
    hex_encode(&hasher.finalize())
}

/// Append one invocation to the ledger.  Computes prev_hash + row_hash
/// inside the transaction so concurrent appends never collide.
pub fn append(
    server_id: &str,
    tool_name: &str,
    arguments: &serde_json::Value,
    result: &serde_json::Value,
    success: bool,
    latency_ms: u64,
) -> AppResult<LedgerEntry> {
    let timestamp = Utc::now();
    let timestamp_str = timestamp.to_rfc3339();
    let args_hash = hash_json(arguments);
    let result_hash = hash_json(result);

    with_ledger(|conn| {
        let prev_hash: String = conn
            .query_row(
                "SELECT row_hash FROM invocations ORDER BY seq DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| GENESIS_HASH.to_string());
        let row_hash = compute_row_hash(
            &timestamp_str,
            server_id,
            tool_name,
            &args_hash,
            &result_hash,
            &prev_hash,
        );
        conn.execute(
            r#"INSERT INTO invocations (timestamp, server_id, tool_name, args_hash,
                                          result_hash, prev_hash, row_hash, success, latency_ms)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                timestamp_str,
                server_id,
                tool_name,
                args_hash,
                result_hash,
                prev_hash,
                row_hash,
                success as i32,
                latency_ms as i64,
            ],
        )
        .map_err(|e| AppError::Internal(format!("insert invocation: {e}")))?;
        let seq: u64 = conn
            .query_row("SELECT last_insert_rowid()", [], |row| row.get::<_, i64>(0))
            .map_err(|e| AppError::Internal(format!("last insert rowid: {e}")))? as u64;
        Ok(LedgerEntry {
            seq,
            timestamp,
            server_id: server_id.to_string(),
            tool_name: tool_name.to_string(),
            args_hash,
            result_hash,
            prev_hash,
            row_hash,
            success,
            latency_ms,
        })
    })
}

/// Query the ledger.  `server_id = None` returns rows from every server.
pub fn query(
    server_id: Option<&str>,
    limit: u32,
    offset: u32,
) -> AppResult<Vec<LedgerEntry>> {
    with_ledger(|conn| {
        if let Some(id) = server_id {
            let mut stmt = conn
                .prepare(
                    r#"SELECT seq, timestamp, server_id, tool_name, args_hash, result_hash,
                                prev_hash, row_hash, success, latency_ms
                       FROM invocations WHERE server_id = ?1
                       ORDER BY seq DESC LIMIT ?2 OFFSET ?3"#,
                )
                .map_err(|e| AppError::Internal(format!("prepare query: {e}")))?;
            let rows: rusqlite::Result<Vec<LedgerEntry>> = stmt
                .query_map(params![id, limit as i64, offset as i64], row_to_entry)
                .map_err(|e| AppError::Internal(format!("query map: {e}")))?
                .collect();
            rows.map_err(|e| AppError::Internal(format!("collect rows: {e}")))
        } else {
            let mut stmt = conn
                .prepare(
                    r#"SELECT seq, timestamp, server_id, tool_name, args_hash, result_hash,
                                prev_hash, row_hash, success, latency_ms
                       FROM invocations ORDER BY seq DESC LIMIT ?1 OFFSET ?2"#,
                )
                .map_err(|e| AppError::Internal(format!("prepare query: {e}")))?;
            let rows: rusqlite::Result<Vec<LedgerEntry>> = stmt
                .query_map(params![limit as i64, offset as i64], row_to_entry)
                .map_err(|e| AppError::Internal(format!("query map: {e}")))?
                .collect();
            rows.map_err(|e| AppError::Internal(format!("collect rows: {e}")))
        }
    })
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<LedgerEntry> {
    let timestamp_str: String = row.get(1)?;
    let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let success_i: i64 = row.get(8)?;
    let latency_i: i64 = row.get(9)?;
    Ok(LedgerEntry {
        seq: row.get::<_, i64>(0)? as u64,
        timestamp,
        server_id: row.get(2)?,
        tool_name: row.get(3)?,
        args_hash: row.get(4)?,
        result_hash: row.get(5)?,
        prev_hash: row.get(6)?,
        row_hash: row.get(7)?,
        success: success_i != 0,
        latency_ms: latency_i.max(0) as u64,
    })
}

/// Verify the chain.  Walks every row, recomputes its expected hash, and
/// stops at the first broken chain or row.
pub fn verify_chain() -> AppResult<LedgerVerification> {
    with_ledger(|conn| {
        let mut stmt = conn
            .prepare(
                r#"SELECT seq, timestamp, server_id, tool_name, args_hash, result_hash,
                          prev_hash, row_hash, success, latency_ms
                   FROM invocations ORDER BY seq ASC"#,
            )
            .map_err(|e| AppError::Internal(format!("prepare verify: {e}")))?;
        let rows: rusqlite::Result<Vec<LedgerEntry>> = stmt
            .query_map([], row_to_entry)
            .map_err(|e| AppError::Internal(format!("verify query: {e}")))?
            .collect();
        let rows = rows.map_err(|e| AppError::Internal(format!("verify collect: {e}")))?;
        if rows.is_empty() {
            return Ok(LedgerVerification::Empty);
        }
        let mut expected_prev = GENESIS_HASH.to_string();
        for row in &rows {
            if row.prev_hash != expected_prev {
                return Ok(LedgerVerification::Tampered {
                    broken_seq: row.seq,
                    reason: format!(
                        "prev_hash mismatch (expected {}, got {})",
                        expected_prev, row.prev_hash
                    ),
                });
            }
            let recomputed = compute_row_hash(
                &row.timestamp.to_rfc3339(),
                &row.server_id,
                &row.tool_name,
                &row.args_hash,
                &row.result_hash,
                &row.prev_hash,
            );
            if recomputed != row.row_hash {
                return Ok(LedgerVerification::Tampered {
                    broken_seq: row.seq,
                    reason: format!(
                        "row_hash mismatch (expected {}, got {})",
                        recomputed, row.row_hash
                    ),
                });
            }
            expected_prev = row.row_hash.clone();
        }
        Ok(LedgerVerification::Ok {
            rows: rows.len() as u64,
        })
    })
}

/// Export the entire chain as JSON (one entry per line — JSONL format).
pub fn export_jsonl() -> AppResult<String> {
    with_ledger(|conn| {
        let mut stmt = conn
            .prepare(
                r#"SELECT seq, timestamp, server_id, tool_name, args_hash, result_hash,
                          prev_hash, row_hash, success, latency_ms
                   FROM invocations ORDER BY seq ASC"#,
            )
            .map_err(|e| AppError::Internal(format!("prepare export: {e}")))?;
        let rows: rusqlite::Result<Vec<LedgerEntry>> = stmt
            .query_map([], row_to_entry)
            .map_err(|e| AppError::Internal(format!("export query: {e}")))?
            .collect();
        let rows = rows.map_err(|e| AppError::Internal(format!("export collect: {e}")))?;
        let mut out = String::new();
        for row in rows {
            let json = serde_json::to_string(&row)
                .map_err(|e| AppError::Internal(format!("encode row: {e}")))?;
            out.push_str(&json);
            out.push('\n');
        }
        Ok(out)
    })
}

/// Reset connection — used by tests.
pub fn reset_for_tests() {
    if let Ok(mut g) = LEDGER_CONN.lock() {
        *g = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct HomeGuard {
        _tmp: TempDir,
        prev: Option<String>,
        _env_lock: std::sync::MutexGuard<'static, ()>,
    }

    fn set_home() -> HomeGuard {
        let lock = marketplace::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var("IMPFORGE_APP_HOME").ok();
        std::env::set_var("IMPFORGE_APP_HOME", tmp.path());
        let mut g = marketplace::MARKETPLACE_CONN.lock().expect("lock");
        *g = None;
        reset_for_tests();
        HomeGuard {
            _tmp: tmp,
            prev,
            _env_lock: lock,
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var("IMPFORGE_APP_HOME", v),
                None => std::env::remove_var("IMPFORGE_APP_HOME"),
            }
            let mut g = marketplace::MARKETPLACE_CONN.lock().expect("lock");
            *g = None;
            reset_for_tests();
        }
    }

    #[test]
    fn append_then_query_returns_entry() {
        let _g = set_home();
        let entry = append(
            "filesystem",
            "read_file",
            &serde_json::json!({ "path": "/tmp/x.txt" }),
            &serde_json::json!({ "content": "hello" }),
            true,
            42,
        )
        .expect("append");
        assert_eq!(entry.server_id, "filesystem");
        assert_eq!(entry.prev_hash, GENESIS_HASH);
        assert_ne!(entry.row_hash, GENESIS_HASH);
        let q = query(None, 10, 0).expect("query");
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].seq, entry.seq);
    }

    #[test]
    fn chain_links_rows_and_verify_succeeds() {
        let _g = set_home();
        let a = append(
            "fs",
            "read",
            &serde_json::json!({ "path": "a" }),
            &serde_json::json!({ "ok": 1 }),
            true,
            10,
        )
        .expect("append a");
        let b = append(
            "fs",
            "read",
            &serde_json::json!({ "path": "b" }),
            &serde_json::json!({ "ok": 2 }),
            true,
            12,
        )
        .expect("append b");
        let c = append(
            "fs",
            "write",
            &serde_json::json!({ "path": "c" }),
            &serde_json::json!({ "ok": 3 }),
            true,
            20,
        )
        .expect("append c");
        assert_eq!(b.prev_hash, a.row_hash, "b must chain to a");
        assert_eq!(c.prev_hash, b.row_hash, "c must chain to b");
        let v = verify_chain().expect("verify");
        assert!(matches!(v, LedgerVerification::Ok { rows: 3 }));
    }

    #[test]
    fn verify_detects_tampering_in_args_hash() {
        let _g = set_home();
        let entry = append(
            "fs",
            "read",
            &serde_json::json!({ "path": "x" }),
            &serde_json::json!({ "ok": 1 }),
            true,
            10,
        )
        .expect("append");
        // Tamper with args_hash directly via SQL — simulates a malicious
        // disk-level edit.
        with_ledger(|conn| {
            conn.execute(
                "UPDATE invocations SET args_hash = ?1 WHERE seq = ?2",
                params!["deadbeef".repeat(8), entry.seq as i64],
            )
            .map_err(|e| AppError::Internal(format!("tamper: {e}")))?;
            Ok(())
        })
        .expect("tamper");
        let v = verify_chain().expect("verify after tamper");
        assert!(matches!(v, LedgerVerification::Tampered { .. }));
    }

    #[test]
    fn verify_empty_returns_empty_variant() {
        let _g = set_home();
        let v = verify_chain().expect("verify");
        assert!(matches!(v, LedgerVerification::Empty));
    }

    #[test]
    fn export_jsonl_round_trips() {
        let _g = set_home();
        for i in 0..3 {
            append(
                "fs",
                "read",
                &serde_json::json!({ "i": i }),
                &serde_json::json!({ "ok": i }),
                true,
                5,
            )
            .expect("append");
        }
        let jsonl = export_jsonl().expect("export");
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let _: LedgerEntry = serde_json::from_str(line).expect("decode line");
        }
    }

    #[test]
    fn canonical_json_sorts_keys() {
        let v = serde_json::json!({ "z": 1, "a": 2, "m": 3 });
        let canonical = canonical_json(&v);
        assert_eq!(canonical, r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn hash_json_deterministic_across_key_order() {
        let v1 = serde_json::json!({ "a": 1, "b": 2 });
        let v2 = serde_json::json!({ "b": 2, "a": 1 });
        assert_eq!(hash_json(&v1), hash_json(&v2));
    }

    #[test]
    fn query_filters_by_server_id() {
        let _g = set_home();
        for id in ["fs", "github", "fs", "memory", "fs"] {
            append(
                id,
                "x",
                &serde_json::json!(null),
                &serde_json::json!(null),
                true,
                1,
            )
            .expect("append");
        }
        let fs_only = query(Some("fs"), 100, 0).expect("query fs");
        assert_eq!(fs_only.len(), 3);
        assert!(fs_only.iter().all(|e| e.server_id == "fs"));
    }
}
