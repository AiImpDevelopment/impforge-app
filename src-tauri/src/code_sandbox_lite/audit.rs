// SPDX-License-Identifier: MIT
//! Tamper-evident execution log — Merkle-chained audit entries.
//!
//! Every cell execution appends one `AuditEntry` whose hash includes
//! the previous entry's hash, producing a tamper-evident chain that
//! cannot be retroactively edited without invalidating downstream
//! signatures.
//!
//! ## Bridge architecture
//!
//! The audit log lives ON THE USER'S MACHINE.  Pro tier mirrors it
//! into the Knowledge Fabric SQLite + the Pro Merkle ledger.  No
//! audit data ever leaves the user's host.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use super::helpers::{hex_encode, now_unix, sha256_hex};
use super::types::{Cell, CellId, CellResult, ProvenanceEdge};

/// One audit entry — one cell execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub cell_id: CellId,
    pub session_id: String,
    pub source_sha256: String,
    pub output_sha256: String,
    pub status: super::types::CellStatus,
    pub exit_code: i32,
    pub fuel_consumed: u64,
    pub wall_time_ms: u128,
    pub timestamp_unix: i64,
    pub trust_level: super::types::TrustLevel,
    /// Hash of the previous entry — the Merkle chain link.
    pub prev_hash: String,
    /// Hash of THIS entry (computed at append time).
    pub self_hash: String,
}

impl AuditEntry {
    /// Build an entry from a `CellResult` — `prev_hash` and `self_hash`
    /// are filled in when appended to the chain.
    pub fn from_result(r: &CellResult) -> Self {
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        for o in &r.outputs {
            if let Ok(bytes) = serde_json::to_vec(o) {
                hasher.update(&bytes);
            }
        }
        let output_sha256 = hex_encode(&hasher.finalize());
        Self {
            id: format!("audit-{}-{}", r.cell_id, now_unix()),
            cell_id: r.cell_id.clone(),
            session_id: String::new(), // filled in by caller if known
            source_sha256: String::new(), // filled in by caller
            output_sha256,
            status: r.status,
            exit_code: r.exit_code,
            fuel_consumed: r.usage.fuel_consumed,
            wall_time_ms: r.usage.wall_time_ms,
            timestamp_unix: now_unix(),
            trust_level: super::types::TrustLevel::Low,
            prev_hash: String::new(),
            self_hash: String::new(),
        }
    }

    /// Compute the entry's hash (excluding `self_hash` itself).
    pub fn compute_self_hash(&self) -> String {
        let mut clone = self.clone();
        clone.self_hash = String::new();
        let bytes = serde_json::to_vec(&clone).unwrap_or_default();
        sha256_hex(&bytes)
    }
}

/// Merkle-chained audit log — append-only.
pub struct AuditLog {
    entries: VecDeque<AuditEntry>,
    /// Cap to avoid unbounded growth — last 4096 entries only kept
    /// in memory.  Pro tier persists older entries to SQLite.
    max_in_memory: usize,
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_in_memory: 4096,
        }
    }

    /// Append an entry, computing prev_hash from the tail.
    pub fn append(&mut self, mut entry: AuditEntry) -> String {
        entry.prev_hash = self
            .entries
            .back()
            .map(|t| t.self_hash.clone())
            .unwrap_or_else(|| "0".repeat(64));
        entry.self_hash = entry.compute_self_hash();
        let id = entry.id.clone();
        if self.entries.len() >= self.max_in_memory {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        id
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn recent(&self, n: usize) -> Vec<AuditEntry> {
        self.entries
            .iter()
            .rev()
            .take(n)
            .cloned()
            .collect()
    }

    /// Verify the chain: every entry's prev_hash matches the predecessor's
    /// self_hash, every self_hash recomputes correctly.
    pub fn verify(&self) -> Result<(), String> {
        let mut expected_prev = "0".repeat(64);
        for (i, e) in self.entries.iter().enumerate() {
            if e.prev_hash != expected_prev {
                return Err(format!(
                    "entry {i}: prev_hash mismatch (expected {expected_prev}, got {})",
                    e.prev_hash
                ));
            }
            let recomputed = e.compute_self_hash();
            if recomputed != e.self_hash {
                return Err(format!(
                    "entry {i}: self_hash mismatch (expected {recomputed}, got {})",
                    e.self_hash
                ));
            }
            expected_prev = e.self_hash.clone();
        }
        Ok(())
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the Merkle root of the chain (= last entry's self_hash).
pub fn audit_chain_root(log: &AuditLog) -> String {
    log.entries
        .back()
        .map(|e| e.self_hash.clone())
        .unwrap_or_else(|| "0".repeat(64))
}

// ── Provenance edges ──────────────────────────────────────────────────────

use std::sync::{Mutex, OnceLock};

fn provenance_store() -> &'static Mutex<Vec<ProvenanceEdge>> {
    static STORE: OnceLock<Mutex<Vec<ProvenanceEdge>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Record one provenance edge for `cell` → `result`.  Stored alongside
/// the audit log so the UI can show the lineage of every cell output.
pub fn record_provenance(cell: &Cell, result: &CellResult) {
    if let Ok(mut store) = provenance_store().lock() {
        let edge = ProvenanceEdge {
            cell_id: cell.id.clone(),
            session_id: cell.session_id.clone(),
            input_ids: vec![cell.source_sha256.clone()],
            output_sha256: {
                let mut h = sha2::Sha256::new();
                use sha2::Digest;
                for o in &result.outputs {
                    if let Ok(b) = serde_json::to_vec(o) {
                        h.update(&b);
                    }
                }
                hex_encode(&h.finalize())
            },
            model_name: None,
            timestamp_unix: now_unix(),
            trust_level: cell.trust_level,
        };
        store.push(edge);
        // Cap.
        if store.len() > 16_384 {
            let drop = store.len() - 16_384;
            store.drain(..drop);
        }
    }
}

/// Return all provenance edges for a given cell — usually 1.
pub fn provenance_for_cell(cell_id: &str) -> Vec<ProvenanceEdge> {
    provenance_store()
        .lock()
        .map(|g| g.iter().filter(|e| e.cell_id == cell_id).cloned().collect())
        .unwrap_or_default()
}

/// Return all provenance edges for a session — full graph view.
pub fn provenance_for_session(session_id: &str) -> Vec<ProvenanceEdge> {
    provenance_store()
        .lock()
        .map(|g| {
            g.iter()
                .filter(|e| e.session_id == session_id)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_sandbox_lite::types::{CellStatus, Output, ResourceUsage, TrustLevel};

    fn fake_result(id: &str) -> CellResult {
        CellResult {
            cell_id: id.into(),
            status: CellStatus::Succeeded,
            exit_code: 0,
            outputs: vec![Output::Text {
                stream: super::super::types::OutputStream::Stdout,
                data: format!("output-of-{id}"),
                ts_unix: 0,
            }],
            usage: ResourceUsage::default(),
            started_unix: 100,
            finished_unix: 200,
            error: None,
            audit_entry_id: None,
        }
    }

    #[test]
    fn audit_entry_self_hash_excludes_self_field() {
        let r = fake_result("c1");
        let mut e = AuditEntry::from_result(&r);
        e.prev_hash = "0".repeat(64);
        e.self_hash = String::new();
        let h1 = e.compute_self_hash();
        e.self_hash = "should-not-affect-hash".into();
        let h2 = e.compute_self_hash();
        assert_eq!(h1, h2, "hash must exclude self_hash field");
    }

    #[test]
    fn audit_log_append_chains_prev_hash() {
        let mut log = AuditLog::new();
        let id1 = log.append(AuditEntry::from_result(&fake_result("c1")));
        let id2 = log.append(AuditEntry::from_result(&fake_result("c2")));
        assert_ne!(id1, id2);
        let entries = log.recent(2);
        assert_eq!(entries.len(), 2);
        // The most recent is at index 0.
        assert_ne!(entries[0].prev_hash, "0".repeat(64));
    }

    #[test]
    fn audit_log_verify_passes_on_clean_chain() {
        let mut log = AuditLog::new();
        log.append(AuditEntry::from_result(&fake_result("a")));
        log.append(AuditEntry::from_result(&fake_result("b")));
        log.append(AuditEntry::from_result(&fake_result("c")));
        log.verify().expect("clean chain verifies");
    }

    #[test]
    fn audit_log_verify_fails_on_tampered_entry() {
        let mut log = AuditLog::new();
        log.append(AuditEntry::from_result(&fake_result("a")));
        log.append(AuditEntry::from_result(&fake_result("b")));
        // Mutate the second entry's exit_code without recomputing hash.
        if let Some(entry) = log.entries.back_mut() {
            entry.exit_code = 99;
        }
        assert!(log.verify().is_err(), "tampered chain must fail verify");
    }

    #[test]
    fn chain_root_is_last_entry_hash() {
        let mut log = AuditLog::new();
        assert_eq!(audit_chain_root(&log), "0".repeat(64));
        log.append(AuditEntry::from_result(&fake_result("c1")));
        let root = audit_chain_root(&log);
        assert_ne!(root, "0".repeat(64));
        assert_eq!(root.len(), 64);
    }

    #[test]
    fn record_provenance_writes_edge() {
        let cell = Cell {
            id: "p-cell-1".into(),
            session_id: "p-session-1".into(),
            lang: super::super::types::Lang::Python,
            source: "print(1)".into(),
            created_unix: 0,
            trust_level: TrustLevel::Low,
            preopen: None,
            env: vec![],
            source_sha256: "abc".into(),
            tags: vec![],
        };
        let result = fake_result("p-cell-1");
        record_provenance(&cell, &result);
        let edges = provenance_for_cell("p-cell-1");
        assert!(!edges.is_empty());
        assert_eq!(edges[0].session_id, "p-session-1");
    }

    #[test]
    fn audit_log_caps_at_max_in_memory() {
        let mut log = AuditLog::new();
        log.max_in_memory = 5;
        for i in 0..10 {
            log.append(AuditEntry::from_result(&fake_result(&format!("c{i}"))));
        }
        assert_eq!(log.len(), 5);
    }

    #[test]
    fn audit_recent_returns_newest_first() {
        let mut log = AuditLog::new();
        log.append(AuditEntry::from_result(&fake_result("first")));
        log.append(AuditEntry::from_result(&fake_result("second")));
        log.append(AuditEntry::from_result(&fake_result("third")));
        let recent = log.recent(2);
        assert_eq!(recent[0].cell_id, "third");
        assert_eq!(recent[1].cell_id, "second");
    }
}
