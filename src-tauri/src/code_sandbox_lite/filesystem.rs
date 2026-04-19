// SPDX-License-Identifier: MIT
//! Filesystem isolation — preopen registry + landlock policy.
//!
//! ## What it does
//!
//! * Tracks per-session file mounts (`MountSpec`).
//! * On Linux 5.13+, applies a Landlock policy to confine the host
//!   process if the user upgrades a session to High trust.  No-op on
//!   non-Linux platforms.
//!
//! ## Cybercrime liability defence
//!
//! Mounts are EXPLICIT (the user picks every host_path).  No mount
//! is created by the LLM — only by direct user action.  Read-only by
//! default.  Trust elevation is per-session and undoable.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// One mount specification — host_path → guest_path with permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountSpec {
    pub host_path: PathBuf,
    pub guest_path: String,
    pub read_only: bool,
}

/// Filesystem policy for a session — list of mounts + landlock state.
#[derive(Debug, Default)]
pub struct FsPolicy {
    pub mounts: Vec<MountSpec>,
    pub landlock_applied: bool,
}

fn registry() -> &'static Mutex<HashMap<String, FsPolicy>> {
    static REG: OnceLock<Mutex<HashMap<String, FsPolicy>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Validate a mount before adding — host_path must exist and be a
/// directory; guest_path must be absolute and not '..' anywhere.
pub fn validate_mount(spec: &MountSpec) -> AppResult<()> {
    if !spec.host_path.exists() {
        return Err(AppError::InvalidArgument(format!(
            "host_path {} does not exist",
            spec.host_path.display()
        )));
    }
    if !spec.host_path.is_dir() {
        return Err(AppError::InvalidArgument(format!(
            "host_path {} is not a directory",
            spec.host_path.display()
        )));
    }
    if !spec.guest_path.starts_with('/') {
        return Err(AppError::InvalidArgument(format!(
            "guest_path {} must be absolute",
            spec.guest_path
        )));
    }
    if spec.guest_path.contains("..") {
        return Err(AppError::InvalidArgument(format!(
            "guest_path {} contains forbidden ..",
            spec.guest_path
        )));
    }
    Ok(())
}

/// Attach a mount to a session.
pub fn attach(session_id: &str, spec: MountSpec) -> AppResult<()> {
    validate_mount(&spec)?;
    let mut g = registry()
        .lock()
        .map_err(|e| AppError::Internal(format!("fs reg lock: {e}")))?;
    let entry = g
        .entry(session_id.to_string())
        .or_default();
    // Replace by guest_path — last mount wins.
    entry.mounts.retain(|m| m.guest_path != spec.guest_path);
    entry.mounts.push(spec);
    Ok(())
}

/// Detach a mount.
pub fn detach(session_id: &str, guest_path: &str) -> AppResult<()> {
    let mut g = registry()
        .lock()
        .map_err(|e| AppError::Internal(format!("fs reg lock: {e}")))?;
    if let Some(entry) = g.get_mut(session_id) {
        entry.mounts.retain(|m| m.guest_path != guest_path);
    }
    Ok(())
}

/// List all mounts for a session.
pub fn list(session_id: &str) -> Vec<MountSpec> {
    registry()
        .lock()
        .map(|g| g.get(session_id).map(|e| e.mounts.clone()).unwrap_or_default())
        .unwrap_or_default()
}

/// Apply Landlock to confine the *host* process when High trust is
/// granted.  No-op on non-Linux or if Landlock is unavailable.
#[cfg(target_os = "linux")]
pub fn apply_landlock(session_id: &str) -> AppResult<bool> {
    use landlock::{
        Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
    };
    let mounts = list(session_id);
    let abi = ABI::V2;
    let access_all = AccessFs::from_all(abi);
    let mut ruleset = Ruleset::default()
        .handle_access(access_all)
        .map_err(|e| AppError::Internal(format!("landlock handle: {e}")))?
        .create()
        .map_err(|e| AppError::Internal(format!("landlock create: {e}")))?;
    for m in &mounts {
        let allowed = if m.read_only {
            AccessFs::from_read(abi)
        } else {
            access_all
        };
        let fd = PathFd::new(&m.host_path)
            .map_err(|e| AppError::Internal(format!("landlock open {}: {e}", m.host_path.display())))?;
        ruleset = ruleset
            .add_rule(PathBeneath::new(fd, allowed))
            .map_err(|e| AppError::Internal(format!("landlock add rule: {e}")))?;
    }
    let status = ruleset
        .restrict_self()
        .map_err(|e| AppError::Internal(format!("landlock restrict: {e}")))?;
    let applied = matches!(
        status.ruleset,
        landlock::RulesetStatus::FullyEnforced | landlock::RulesetStatus::PartiallyEnforced
    );
    if let Ok(mut g) = registry().lock() {
        if let Some(entry) = g.get_mut(session_id) {
            entry.landlock_applied = applied;
        }
    }
    Ok(applied)
}

#[cfg(not(target_os = "linux"))]
pub fn apply_landlock(_session_id: &str) -> AppResult<bool> {
    // Landlock is Linux-only.  On macOS/Windows, mounts still apply
    // via WASI preopens — we just don't get host-process confinement.
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn validate_mount_rejects_missing_path() {
        let bad = MountSpec {
            host_path: PathBuf::from("/nonexistent/xyz/abc/123"),
            guest_path: "/data".into(),
            read_only: true,
        };
        assert!(validate_mount(&bad).is_err());
    }

    #[test]
    fn validate_mount_rejects_relative_guest() {
        let tmp = tempdir().expect("tempdir");
        let bad = MountSpec {
            host_path: tmp.path().to_path_buf(),
            guest_path: "data".into(),
            read_only: true,
        };
        assert!(validate_mount(&bad).is_err());
    }

    #[test]
    fn validate_mount_rejects_dotdot() {
        let tmp = tempdir().expect("tempdir");
        let bad = MountSpec {
            host_path: tmp.path().to_path_buf(),
            guest_path: "/foo/../bar".into(),
            read_only: true,
        };
        assert!(validate_mount(&bad).is_err());
    }

    #[test]
    fn attach_then_list_returns_mount() {
        let tmp = tempdir().expect("tempdir");
        let id = "test-fs-1";
        attach(
            id,
            MountSpec {
                host_path: tmp.path().to_path_buf(),
                guest_path: "/data".into(),
                read_only: true,
            },
        )
        .expect("attach ok");
        let mounts = list(id);
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].guest_path, "/data");
    }

    #[test]
    fn attach_replaces_by_guest_path() {
        let tmp = tempdir().expect("tempdir");
        let id = "test-fs-2";
        let spec = MountSpec {
            host_path: tmp.path().to_path_buf(),
            guest_path: "/data".into(),
            read_only: true,
        };
        attach(id, spec.clone()).expect("ok 1");
        attach(id, spec).expect("ok 2 — replace");
        assert_eq!(list(id).len(), 1, "should not duplicate");
    }

    #[test]
    fn detach_removes_mount() {
        let tmp = tempdir().expect("tempdir");
        let id = "test-fs-3";
        attach(
            id,
            MountSpec {
                host_path: tmp.path().to_path_buf(),
                guest_path: "/data".into(),
                read_only: true,
            },
        )
        .expect("attach");
        detach(id, "/data").expect("detach");
        assert!(list(id).is_empty());
    }

    #[test]
    fn detach_unknown_guest_path_is_noop() {
        let id = "test-fs-4";
        detach(id, "/never-attached").expect("noop ok");
    }

    #[test]
    fn list_for_unknown_session_returns_empty() {
        assert!(list("never-touched-fs-session").is_empty());
    }
}
