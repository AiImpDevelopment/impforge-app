// SPDX-License-Identifier: MIT
//! Install / uninstall / version-pin / update lifecycle for MCP servers.
//!
//! ## Storage
//!
//! `~/.impforge/mcp-installed.json` (atomic write via `tempfile`).
//! Per-server config lives at `~/.impforge/mcp-installed/<id>/config.json`.
//!
//! ## What install does NOT do
//!
//! * It does NOT actually run `npm install` / `pip install` for you.
//!   The CLI's command + args are written verbatim — when you start the
//!   server, `npx` / `uvx` will resolve the package on first run.
//! * It does NOT validate OAuth tokens — `oauth.rs` handles that flow
//!   separately.
//! * It does NOT write secrets to the JSON config — env vars marked
//!   `secret = true` go through the keyring instead (handled by the
//!   keys module).

use crate::error::{AppError, AppResult};
use crate::mcp::marketplace;
use crate::mcp::types::{InstalledServer, MarketplaceEntry};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Path to the JSON file holding the installed-servers list.
pub fn installed_path() -> AppResult<PathBuf> {
    Ok(marketplace::app_home()?.join("mcp-installed.json"))
}

/// Per-server config + bundle directory.
pub fn server_dir(id: &str) -> AppResult<PathBuf> {
    let dir = marketplace::app_home()?.join("mcp-installed").join(id);
    fs::create_dir_all(&dir).map_err(AppError::from)?;
    Ok(dir)
}

/// Per-server config JSON path.
pub fn server_config_path(id: &str) -> AppResult<PathBuf> {
    Ok(server_dir(id)?.join("config.json"))
}

/// `Mutex` around the install state — prevents two parallel installs of
/// the same server clobbering each other's config.
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

/// `mcp_install_server` request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstallRequest {
    pub server_id: String,
    /// Pin to a specific version (defaults to entry's `version`).
    pub version: Option<String>,
    /// Pre-filled env values — keyed by var name.  Secret values written
    /// via `keys` module rather than this JSON config.
    pub env: BTreeMap<String, String>,
    /// Filesystem paths the user has granted (Landlock allowlist).
    pub fs_allowlist: Vec<PathBuf>,
    /// Network hosts the user has granted.
    pub net_allowlist: Vec<String>,
    /// Optional bundle id this install belongs to.
    pub bundle_source: Option<String>,
}

/// Result of an install — surfaces in the UI snackbar + install log.
#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub installed: InstalledServer,
    pub seeded_keyring_count: u32,
    pub warnings: Vec<String>,
}

/// Install one server.
pub fn install(req: &InstallRequest) -> AppResult<InstallResult> {
    let _g = INSTALL_LOCK
        .lock()
        .map_err(|e| AppError::Internal(format!("install lock poisoned: {e}")))?;

    let entry = marketplace::get(&req.server_id)?;
    let version = req.version.clone().unwrap_or_else(|| entry.version.clone());

    let installed = build_installed(&entry, &version, req)?;

    write_default_config(&installed)?;
    upsert_installed(&installed)?;

    let mut warnings = Vec::new();
    let mut seeded_keyring_count = 0u32;
    for spec in &entry.env_schema {
        if !spec.required {
            continue;
        }
        let provided = req.env.get(&spec.name).map(|s| !s.is_empty()).unwrap_or(false);
        if !provided {
            warnings.push(format!(
                "required env var '{}' not provided — server will fail until configured",
                spec.name
            ));
            continue;
        }
        if spec.secret {
            seeded_keyring_count += 1;
        }
    }

    Ok(InstallResult {
        installed,
        seeded_keyring_count,
        warnings,
    })
}

fn build_installed(
    entry: &MarketplaceEntry,
    version: &str,
    req: &InstallRequest,
) -> AppResult<InstalledServer> {
    let config_path = server_config_path(&entry.id)?;
    // Non-secret env vars travel in JSON; secrets are stored in the keyring.
    let mut env = BTreeMap::new();
    for (k, v) in &req.env {
        let is_secret = entry
            .env_schema
            .iter()
            .find(|spec| spec.name == *k)
            .map(|spec| spec.secret)
            .unwrap_or(false);
        if !is_secret {
            env.insert(k.clone(), v.clone());
        }
    }
    Ok(InstalledServer {
        id: entry.id.clone(),
        version: version.to_string(),
        installed_at: Utc::now(),
        config_path,
        transport: entry.transport,
        command: entry.command.clone(),
        args: entry.args.clone(),
        env,
        tool_allowlist: None,
        fs_allowlist: req.fs_allowlist.clone(),
        net_allowlist: req.net_allowlist.clone(),
        enabled: true,
        bundle_source: req.bundle_source.clone(),
    })
}

/// Write the per-server config JSON.
fn write_default_config(server: &InstalledServer) -> AppResult<()> {
    let cfg = serde_json::json!({
        "id": server.id,
        "version": server.version,
        "installed_at": server.installed_at,
        "transport": server.transport.as_str(),
        "command": server.command,
        "args": server.args,
        "env": server.env,
        "fs_allowlist": server.fs_allowlist,
        "net_allowlist": server.net_allowlist,
        "enabled": server.enabled,
        "bundle_source": server.bundle_source,
    });
    write_atomic(&server.config_path, &serde_json::to_vec_pretty(&cfg).map_err(|e| AppError::Internal(format!("encode config: {e}")))?)
}

/// Atomic write — write to `<path>.tmp` then rename.  Avoids torn JSON
/// if the app crashes mid-write.
fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Internal(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent).map_err(AppError::from)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(AppError::from)?;
    tmp.write_all(bytes).map_err(AppError::from)?;
    tmp.persist(path)
        .map_err(|e| AppError::Internal(format!("persist tempfile: {e}")))?;
    Ok(())
}

/// Uninstall one server.
pub fn uninstall(id: &str) -> AppResult<()> {
    let _g = INSTALL_LOCK
        .lock()
        .map_err(|e| AppError::Internal(format!("install lock poisoned: {e}")))?;
    let mut all = load()?;
    let before = all.len();
    all.retain(|s| s.id != id);
    if all.len() == before {
        return Err(AppError::NotFound(format!("server '{id}' is not installed")));
    }
    save(&all)?;
    let dir = server_dir(id)?;
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}

/// Pin an installed server to a different version.
pub fn pin_version(id: &str, version: &str) -> AppResult<InstalledServer> {
    let _g = INSTALL_LOCK
        .lock()
        .map_err(|e| AppError::Internal(format!("install lock poisoned: {e}")))?;
    let mut all = load()?;
    let server = all
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("server '{id}' is not installed")))?;
    server.version = version.to_string();
    let snapshot = server.clone();
    save(&all)?;
    write_default_config(&snapshot)?;
    Ok(snapshot)
}

/// Toggle a server enabled/disabled (no restart).
pub fn set_enabled(id: &str, enabled: bool) -> AppResult<InstalledServer> {
    let _g = INSTALL_LOCK
        .lock()
        .map_err(|e| AppError::Internal(format!("install lock poisoned: {e}")))?;
    let mut all = load()?;
    let server = all
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("server '{id}' is not installed")))?;
    server.enabled = enabled;
    let snapshot = server.clone();
    save(&all)?;
    write_default_config(&snapshot)?;
    Ok(snapshot)
}

/// Update one server's pinned version to whatever the marketplace
/// currently advertises.  The capability-diff is computed by
/// `capability_diff::compare` — call that BEFORE this to surface the
/// rug-pull defense diff in the UI.
pub fn update_to_latest(id: &str) -> AppResult<InstalledServer> {
    let entry = marketplace::get(id)?;
    pin_version(id, &entry.version)
}

/// Load the installed list (empty if file missing).
pub fn load() -> AppResult<Vec<InstalledServer>> {
    let path = installed_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&path).map_err(AppError::from)?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<InstalledServer> = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Internal(format!("decode installed: {e}")))?;
    Ok(list)
}

/// Persist the installed list atomically.
pub fn save(servers: &[InstalledServer]) -> AppResult<()> {
    let path = installed_path()?;
    let bytes = serde_json::to_vec_pretty(servers)
        .map_err(|e| AppError::Internal(format!("encode installed: {e}")))?;
    write_atomic(&path, &bytes)
}

/// Upsert (insert-or-update) one entry in the installed list.
pub fn upsert_installed(server: &InstalledServer) -> AppResult<()> {
    let mut all = load()?;
    if let Some(existing) = all.iter_mut().find(|s| s.id == server.id) {
        *existing = server.clone();
    } else {
        all.push(server.clone());
    }
    save(&all)
}

/// One server by id, or `None`.
pub fn find(id: &str) -> AppResult<Option<InstalledServer>> {
    Ok(load()?.into_iter().find(|s| s.id == id))
}

/// Update tool allowlist for an installed server.  `None` means "all
/// tools allowed".  `Some(Vec)` enforces per-tool capability allowlists.
pub fn set_tool_allowlist(id: &str, allowlist: Option<Vec<String>>) -> AppResult<InstalledServer> {
    let _g = INSTALL_LOCK
        .lock()
        .map_err(|e| AppError::Internal(format!("install lock poisoned: {e}")))?;
    let mut all = load()?;
    let server = all
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::NotFound(format!("server '{id}' is not installed")))?;
    server.tool_allowlist = allowlist;
    let snapshot = server.clone();
    save(&all)?;
    write_default_config(&snapshot)?;
    Ok(snapshot)
}

/// Cheap helper for tests + diagnostics — counts installed servers.
pub fn count() -> AppResult<usize> {
    Ok(load()?.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::{Category, Transport};
    use tempfile::TempDir;

    struct HomeGuard {
        _tmp: TempDir,
        prev: Option<String>,
    }

    fn set_home() -> HomeGuard {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var("IMPFORGE_APP_HOME").ok();
        std::env::set_var("IMPFORGE_APP_HOME", tmp.path());
        // Reset marketplace cache so tests get a fresh DB.
        let mut g = crate::mcp::marketplace::MARKETPLACE_CONN
            .lock()
            .expect("lock");
        *g = None;
        HomeGuard { _tmp: tmp, prev }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var("IMPFORGE_APP_HOME", v),
                None => std::env::remove_var("IMPFORGE_APP_HOME"),
            }
            let mut g = crate::mcp::marketplace::MARKETPLACE_CONN
                .lock()
                .expect("lock");
            *g = None;
        }
    }

    #[test]
    fn install_then_load_finds_server() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let result = install(&InstallRequest {
            server_id: "filesystem".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: None,
        })
        .expect("install");
        assert_eq!(result.installed.id, "filesystem");
        assert_eq!(result.installed.transport, Transport::Stdio);
        let all = load().expect("load");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "filesystem");
    }

    #[test]
    fn install_secret_env_does_not_leak_into_json_config() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let mut env = BTreeMap::new();
        env.insert("GITHUB_PERSONAL_ACCESS_TOKEN".into(), "secret-pat".into());
        env.insert("PUBLIC_VAR".into(), "ok".into());
        let result = install(&InstallRequest {
            server_id: "github".into(),
            version: None,
            env,
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: None,
        })
        .expect("install");
        assert_eq!(result.seeded_keyring_count, 1, "PAT must route through keyring counter");
        // Secret must NOT appear in the JSON config.
        let cfg = fs::read_to_string(&result.installed.config_path).expect("read cfg");
        assert!(!cfg.contains("secret-pat"), "secret leaked into config JSON");
    }

    #[test]
    fn install_missing_required_env_emits_warning() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let result = install(&InstallRequest {
            server_id: "github".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: None,
        })
        .expect("install");
        assert!(!result.warnings.is_empty(), "missing required env must produce warning");
    }

    #[test]
    fn uninstall_round_trips_with_install() {
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
        uninstall("memory").expect("uninstall");
        assert_eq!(count().expect("count"), 0);
    }

    #[test]
    fn uninstall_unknown_id_returns_not_found() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let err = uninstall("nope").expect_err("must fail");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn pin_version_overrides_default() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        install(&InstallRequest {
            server_id: "git".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: None,
        })
        .expect("install");
        let pinned = pin_version("git", "9.9.9").expect("pin");
        assert_eq!(pinned.version, "9.9.9");
        let loaded = find("git").expect("find").expect("present");
        assert_eq!(loaded.version, "9.9.9");
    }

    #[test]
    fn set_enabled_toggles_flag() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        install(&InstallRequest {
            server_id: "fetch".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: None,
        })
        .expect("install");
        let toggled = set_enabled("fetch", false).expect("toggle");
        assert!(!toggled.enabled);
        let loaded = find("fetch").expect("find").expect("present");
        assert!(!loaded.enabled);
    }

    #[test]
    fn set_tool_allowlist_persists() {
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
        set_tool_allowlist(
            "filesystem",
            Some(vec!["read_file".into(), "write_file".into()]),
        )
        .expect("set allowlist");
        let loaded = find("filesystem").expect("find").expect("present");
        assert_eq!(
            loaded.tool_allowlist,
            Some(vec!["read_file".into(), "write_file".into()])
        );
    }

    #[test]
    fn install_bundle_source_propagates() {
        let _g = set_home();
        marketplace::ensure_seeded().expect("seed");
        let result = install(&InstallRequest {
            server_id: "filesystem".into(),
            version: None,
            env: BTreeMap::new(),
            fs_allowlist: vec![],
            net_allowlist: vec![],
            bundle_source: Some("coding".into()),
        })
        .expect("install");
        assert_eq!(result.installed.bundle_source.as_deref(), Some("coding"));
    }

    #[test]
    fn config_path_lives_under_app_home() {
        let _g = set_home();
        let p = server_config_path("filesystem").expect("config path");
        assert!(p.to_string_lossy().contains("mcp-installed/filesystem"));
        // Suppress lint for unused-must-use of intermediate Category; keeps the
        // module-level `use` paths obviously connected to its callers.
        let _ = Category::Code;
    }
}
