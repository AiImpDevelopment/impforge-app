// SPDX-License-Identifier: MIT
//! Per-session budget accounting — memory + time + disk usage tracked
//! across cells and aggregated for the UI's `ResourceMonitor`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::types::SessionId;

/// One snapshot of a session's resource budget.  Returned to the UI.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub session_id_hash: u64,
    pub memory_used_bytes: u64,
    pub memory_peak_bytes: u64,
    pub fuel_consumed: u64,
    pub wall_time_ms_total: u128,
    pub disk_bytes_written: u64,
    pub cells_run: u64,
    pub cells_succeeded: u64,
    pub cells_failed: u64,
    pub cells_limit_exceeded: u64,
}

/// Mutable budget for one session — backs the snapshot.
#[derive(Debug, Default)]
pub struct Budget {
    pub session_id: String,
    pub snapshot: BudgetSnapshot,
}

impl Budget {
    pub fn new(session_id: String) -> Self {
        let snap = BudgetSnapshot {
            session_id_hash: hash_session(&session_id),
            ..Default::default()
        };
        Self {
            session_id,
            snapshot: snap,
        }
    }

    /// Add one cell's resource usage to the running total.
    pub fn account(&mut self, usage: &super::types::ResourceUsage, status: super::types::CellStatus) {
        self.snapshot.fuel_consumed = self
            .snapshot
            .fuel_consumed
            .saturating_add(usage.fuel_consumed);
        self.snapshot.wall_time_ms_total = self
            .snapshot
            .wall_time_ms_total
            .saturating_add(usage.wall_time_ms);
        self.snapshot.disk_bytes_written = self
            .snapshot
            .disk_bytes_written
            .saturating_add(usage.disk_bytes_written);
        self.snapshot.memory_used_bytes = usage.memory_peak_bytes;
        if usage.memory_peak_bytes > self.snapshot.memory_peak_bytes {
            self.snapshot.memory_peak_bytes = usage.memory_peak_bytes;
        }
        self.snapshot.cells_run = self.snapshot.cells_run.saturating_add(1);
        match status {
            super::types::CellStatus::Succeeded => {
                self.snapshot.cells_succeeded =
                    self.snapshot.cells_succeeded.saturating_add(1);
            }
            super::types::CellStatus::Failed => {
                self.snapshot.cells_failed = self.snapshot.cells_failed.saturating_add(1);
            }
            super::types::CellStatus::LimitExceeded => {
                self.snapshot.cells_limit_exceeded =
                    self.snapshot.cells_limit_exceeded.saturating_add(1);
            }
            _ => {}
        }
    }
}

fn hash_session(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn registry() -> &'static Mutex<HashMap<SessionId, Budget>> {
    static REG: OnceLock<Mutex<HashMap<SessionId, Budget>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Public snapshot accessor — returns a default if the session has
/// no recorded usage yet.
pub fn session_snapshot(session_id: &str) -> BudgetSnapshot {
    registry()
        .lock()
        .map(|g| {
            g.get(session_id)
                .map(|b| b.snapshot)
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Account a cell's usage into its session budget.  Idempotent on
/// poisoned-lock failures (drops the update silently — better than
/// killing the host).
pub fn account_cell(
    session_id: &str,
    usage: &super::types::ResourceUsage,
    status: super::types::CellStatus,
) {
    if let Ok(mut g) = registry().lock() {
        let entry = g
            .entry(session_id.to_string())
            .or_insert_with(|| Budget::new(session_id.to_string()));
        entry.account(usage, status);
    }
}

/// Reset a session's budget — used when the user clears + restarts.
pub fn reset_session(session_id: &str) {
    if let Ok(mut g) = registry().lock() {
        g.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_sandbox_lite::types::{CellStatus, ResourceUsage};

    fn fake_usage(fuel: u64, ms: u128, mem: u64) -> ResourceUsage {
        ResourceUsage {
            memory_peak_bytes: mem,
            fuel_consumed: fuel,
            wall_time_ms: ms,
            cpu_time_ms: ms,
            disk_bytes_written: 0,
            stdout_bytes: 0,
            stderr_bytes: 0,
        }
    }

    #[test]
    fn budget_default_zeroed() {
        let b = BudgetSnapshot::default();
        assert_eq!(b.fuel_consumed, 0);
        assert_eq!(b.cells_run, 0);
    }

    #[test]
    fn account_aggregates_across_cells() {
        let id = "test-session-budget-1";
        reset_session(id);
        account_cell(id, &fake_usage(100, 500, 1024), CellStatus::Succeeded);
        account_cell(id, &fake_usage(200, 700, 2048), CellStatus::Succeeded);
        let snap = session_snapshot(id);
        assert_eq!(snap.cells_run, 2);
        assert_eq!(snap.cells_succeeded, 2);
        assert_eq!(snap.fuel_consumed, 300);
        assert_eq!(snap.wall_time_ms_total, 1200);
        assert_eq!(snap.memory_peak_bytes, 2048);
        reset_session(id);
    }

    #[test]
    fn account_distinguishes_status() {
        let id = "test-session-budget-2";
        reset_session(id);
        account_cell(id, &fake_usage(10, 10, 0), CellStatus::Succeeded);
        account_cell(id, &fake_usage(10, 10, 0), CellStatus::Failed);
        account_cell(id, &fake_usage(10, 10, 0), CellStatus::LimitExceeded);
        let snap = session_snapshot(id);
        assert_eq!(snap.cells_succeeded, 1);
        assert_eq!(snap.cells_failed, 1);
        assert_eq!(snap.cells_limit_exceeded, 1);
        reset_session(id);
    }

    #[test]
    fn reset_clears_session_state() {
        let id = "test-session-budget-3";
        account_cell(id, &fake_usage(10, 10, 1), CellStatus::Succeeded);
        reset_session(id);
        let snap = session_snapshot(id);
        assert_eq!(snap.cells_run, 0);
    }

    #[test]
    fn snapshot_for_unknown_session_returns_default() {
        let snap = session_snapshot("never-touched-session-xyz");
        assert_eq!(snap.cells_run, 0);
    }

    #[test]
    fn hash_session_is_deterministic() {
        assert_eq!(hash_session("foo"), hash_session("foo"));
        assert_ne!(hash_session("foo"), hash_session("bar"));
    }
}
