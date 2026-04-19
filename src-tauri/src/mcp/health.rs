// SPDX-License-Identifier: MIT
//! Per-server health snapshots (latency, success rate, restart count).
//!
//! Pulls from the in-memory `runner::snapshot_all` and pairs each
//! running server with the per-server installed metadata (so the UI can
//! show "running" / "stopped" / "disabled" badges in the same data
//! frame).

use crate::error::AppResult;
use crate::mcp::installer;
use crate::mcp::runner::{self, RuntimeSnapshot};
use crate::mcp::types::{InstalledServer, ServerHealth};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// One row in the HealthDashboard.
#[derive(Debug, Clone, Serialize)]
pub struct HealthRow {
    pub server: InstalledServer,
    pub health: ServerHealth,
    pub running: bool,
    pub last_started: Option<DateTime<Utc>>,
}

/// Build a `HealthRow` per installed server.  Disabled servers + servers
/// that have never been started still appear, with `running = false` and
/// zeroed metrics.
pub fn dashboard() -> AppResult<Vec<HealthRow>> {
    let installed = installer::load()?;
    let runtime: std::collections::HashMap<String, RuntimeSnapshot> = runner::snapshot_all()?
        .into_iter()
        .map(|s| (s.server_id.clone(), s))
        .collect();

    let mut out = Vec::with_capacity(installed.len());
    for server in installed {
        let snap = runtime.get(&server.id);
        let running = snap.map(|s| s.running).unwrap_or(false);
        let h = ServerHealth {
            server_id: server.id.clone(),
            running,
            p95_latency_ms: snap.and_then(|s| s.p95_latency_ms),
            success_rate: snap.map(|s| s.success_rate).unwrap_or(1.0),
            calls_last_hour: snap.map(|s| s.calls_last_second * 60 * 60).unwrap_or(0),
            restart_count: snap.map(|s| s.restart_count as u32).unwrap_or(0),
            last_error: None,
            uptime_seconds: snap.map(|s| s.uptime_seconds).unwrap_or(0),
        };
        out.push(HealthRow {
            server,
            health: h,
            running,
            last_started: snap.and_then(|s| s.last_started),
        });
    }
    Ok(out)
}

/// Cheap health check for one server — returns the row even if not
/// installed (so the UI can render "uninstalled but cached" gracefully).
pub fn ping(server_id: &str) -> AppResult<Option<HealthRow>> {
    let installed = installer::find(server_id)?;
    let Some(server) = installed else {
        return Ok(None);
    };
    let snap = runner::snapshot(&server.id)?;
    let running = snap.as_ref().map(|s| s.running).unwrap_or(false);
    let h = ServerHealth {
        server_id: server.id.clone(),
        running,
        p95_latency_ms: snap.as_ref().and_then(|s| s.p95_latency_ms),
        success_rate: snap.as_ref().map(|s| s.success_rate).unwrap_or(1.0),
        calls_last_hour: snap
            .as_ref()
            .map(|s| s.calls_last_second * 60 * 60)
            .unwrap_or(0),
        restart_count: snap.as_ref().map(|s| s.restart_count as u32).unwrap_or(0),
        last_error: None,
        uptime_seconds: snap.as_ref().map(|s| s.uptime_seconds).unwrap_or(0),
    };
    Ok(Some(HealthRow {
        server,
        health: h,
        running,
        last_started: snap.as_ref().and_then(|s| s.last_started),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::installer::{install, InstallRequest};
    use crate::mcp::marketplace;
    use std::collections::BTreeMap;
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
        let _ = runner::reset_for_tests();
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
            let _ = runner::reset_for_tests();
        }
    }

    #[test]
    fn dashboard_lists_installed_servers_even_if_not_started() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        install(&InstallRequest {
            server_id: "filesystem".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: None,
        })
        .expect("install");
        let rows = dashboard().expect("dashboard");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].server.id, "filesystem");
        assert!(!rows[0].running);
        assert!((rows[0].health.success_rate - 1.0).abs() < 0.01);
    }

    #[test]
    fn ping_uninstalled_server_returns_none() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let row = ping("not-installed").expect("ping");
        assert!(row.is_none());
    }

    #[test]
    fn ping_installed_returns_some_even_without_runtime() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        install(&InstallRequest {
            server_id: "memory".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: None,
        })
        .expect("install");
        let row = ping("memory").expect("ping").expect("present");
        assert_eq!(row.server.id, "memory");
        assert!(!row.running);
    }
}
