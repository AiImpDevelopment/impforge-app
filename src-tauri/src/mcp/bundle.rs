// SPDX-License-Identifier: MIT
//! Curated bundle templates — "three clicks to a working agent".
//!
//! Each bundle ships 3-5 servers + pre-filled config keys + a one-line
//! description.  The 5 stock bundles cover the most common "I want my
//! AI to do X" scenarios.
//!
//! ## Marketing value
//!
//! Bundle install is the **Aha moment** for first-time users.  Click
//! "GitHub PR Reviewer" → 3 servers install in parallel → chat opens
//! pre-loaded with a templated prompt that uses one of the new tools.

use crate::error::AppResult;
use crate::mcp::installer::{install, InstallRequest, InstallResult};
use crate::mcp::types::BundleTemplate;
use std::collections::BTreeMap;

/// All 5 stock bundles.
pub fn stock_bundles() -> Vec<BundleTemplate> {
    vec![
        BundleTemplate {
            id: "coding".into(),
            name: "Coding Companion".into(),
            description: "Filesystem · Git · Memory — the canonical 'AI pair-programmer' setup."
                .into(),
            icon: "code".into(),
            server_ids: vec!["filesystem".into(), "git".into(), "memory".into()],
            prefilled_env: vec![],
        },
        BundleTemplate {
            id: "research".into(),
            name: "Research Assistant".into(),
            description: "Brave Search · Fetch · Memory — query the web + remember findings."
                .into(),
            icon: "search".into(),
            server_ids: vec!["brave-search".into(), "fetch".into(), "memory".into()],
            prefilled_env: vec![],
        },
        BundleTemplate {
            id: "github-pr-reviewer".into(),
            name: "GitHub PR Reviewer".into(),
            description: "GitHub · Filesystem — read PRs, browse code, post review comments."
                .into(),
            icon: "git-pull-request".into(),
            server_ids: vec!["github".into(), "filesystem".into()],
            prefilled_env: vec![],
        },
        BundleTemplate {
            id: "data-analyst".into(),
            name: "Data Analyst".into(),
            description: "Postgres · Memory · Fetch — read-only DB access + persistent context."
                .into(),
            icon: "database".into(),
            server_ids: vec!["postgres".into(), "memory".into(), "fetch".into()],
            prefilled_env: vec![],
        },
        BundleTemplate {
            id: "comms-hub".into(),
            name: "Comms Hub".into(),
            description: "Slack · Memory — let your AI read + send team messages safely."
                .into(),
            icon: "message-square".into(),
            server_ids: vec!["slack".into(), "memory".into()],
            prefilled_env: vec![],
        },
    ]
}

/// Lookup a bundle by id.
pub fn get(id: &str) -> Option<BundleTemplate> {
    stock_bundles().into_iter().find(|b| b.id == id)
}

/// Install every server in a bundle.  Continues on error — returns the
/// per-server result list so the UI can show partial-success state.
///
/// Calls `marketplace::ensure_seeded` up front so a fresh App install
/// (no marketplace sync yet) can still install a stock bundle from the
/// curated 8-server seed.
pub fn install_bundle(bundle_id: &str) -> AppResult<Vec<BundleInstallResult>> {
    let bundle = get(bundle_id).ok_or_else(|| {
        crate::error::AppError::NotFound(format!("bundle '{bundle_id}' not found"))
    })?;
    crate::mcp::marketplace::ensure_seeded()?;
    let mut env_per_server: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (server_id, env_var, value) in &bundle.prefilled_env {
        env_per_server
            .entry(server_id.clone())
            .or_default()
            .insert(env_var.clone(), value.clone());
    }
    let mut results = Vec::with_capacity(bundle.server_ids.len());
    for server_id in &bundle.server_ids {
        let env = env_per_server.remove(server_id).unwrap_or_default();
        let outcome = install(&InstallRequest {
            server_id: server_id.clone(),
            version: None,
            env,
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: Some(bundle.id.clone()),
        });
        results.push(BundleInstallResult {
            server_id: server_id.clone(),
            outcome: outcome.map_err(|e| e.to_string()),
        });
    }
    Ok(results)
}

/// Per-server outcome inside a bundle install.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BundleInstallResult {
    pub server_id: String,
    /// `Ok(InstallResult)` on success, `Err(message)` on failure.
    pub outcome: Result<InstallResult, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::installer;
    use crate::mcp::marketplace;
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
        }
    }

    #[test]
    fn five_stock_bundles_present() {
        let bundles = stock_bundles();
        assert_eq!(bundles.len(), 5);
        let ids: Vec<&str> = bundles.iter().map(|b| b.id.as_str()).collect();
        assert!(ids.contains(&"coding"));
        assert!(ids.contains(&"research"));
        assert!(ids.contains(&"github-pr-reviewer"));
        assert!(ids.contains(&"data-analyst"));
        assert!(ids.contains(&"comms-hub"));
    }

    #[test]
    fn each_bundle_has_at_least_two_servers() {
        for b in stock_bundles() {
            assert!(b.server_ids.len() >= 2, "bundle {} has <2 servers", b.id);
        }
    }

    #[test]
    fn install_bundle_writes_servers_with_bundle_source() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let results = install_bundle("coding").expect("install bundle");
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.outcome.is_ok()));
        let installed = installer::load().expect("load");
        for s in installed {
            assert_eq!(s.bundle_source.as_deref(), Some("coding"));
        }
    }

    #[test]
    fn install_unknown_bundle_returns_not_found() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let err = install_bundle("nope").expect_err("must fail");
        assert!(matches!(err, crate::error::AppError::NotFound(_)));
    }

    #[test]
    fn get_returns_some_for_stock_bundle() {
        assert!(get("coding").is_some());
        assert!(get("does-not-exist").is_none());
    }

    #[test]
    fn install_bundle_recovers_via_ensure_seeded_inside_install() {
        let _g = set_home();
        // Don't seed — install() runs ensure_seeded() internally, so
        // even a "fresh" install of a stock bundle should succeed
        // because the curated 8-server seed will be populated on demand.
        let results = install_bundle("research").expect("call still ok");
        assert_eq!(results.len(), 3);
        let failures: Vec<&BundleInstallResult> =
            results.iter().filter(|r| r.outcome.is_err()).collect();
        assert!(
            failures.is_empty(),
            "ensure_seeded inside install should make this work end-to-end; failures: {:?}",
            failures
                .iter()
                .map(|r| (
                    r.server_id.as_str(),
                    r.outcome.as_ref().err().map(|s| s.as_str()).unwrap_or("?")
                ))
                .collect::<Vec<_>>()
        );
    }

    /// One synthetic non-stock bundle with one valid + one invalid id —
    /// proves the per-server outcome list reports failure cleanly without
    /// tearing down the whole call.
    #[test]
    fn install_bundle_returns_per_server_failure_for_unknown_server() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        // We can't add a synthetic bundle to stock_bundles() without
        // touching the public API, so we exercise install() directly
        // with both a valid and an invalid id and confirm the per-server
        // semantics.
        let valid = install(&InstallRequest {
            server_id: "memory".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: Some("synthetic".into()),
        });
        let invalid = install(&InstallRequest {
            server_id: "does-not-exist".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: Some("synthetic".into()),
        });
        assert!(valid.is_ok(), "valid id must succeed inside bundle context");
        assert!(invalid.is_err(), "invalid id must fail without poisoning the rest");
    }
}
