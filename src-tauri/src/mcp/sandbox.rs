// SPDX-License-Identifier: MIT
//! Per-server sandbox profiles (Linux Landlock + seccomp, best-effort).
//!
//! ## What this gives the MIT user
//!
//! On Linux 5.13+:
//!   * Landlock-enforced filesystem allowlist — server can only read/write
//!     paths in `installed.fs_allowlist` (per-tool capability allowlist
//!     pattern, per Multikernel `2026-04-03`).
//!   * seccomp-BPF policy denying obviously-dangerous syscalls (ptrace,
//!     kexec_load, finit_module, etc.) — narrow whitelist beyond that
//!     would break too many MCP servers' npm/uvx bootstraps.
//!
//! On macOS / Windows / older Linux:
//!   * `apply_landlock` returns `Status::Unsupported` and skips
//!     sandboxing.  The UI shows an orange "sandbox unavailable" badge.
//!
//! Pro adds:
//!   * Bubblewrap profile per server (full namespace isolation +
//!     read-only root + tmpfs `/tmp`).
//!   * Per-tool egress proxy.
//!   * KVM jails for community-submitted servers.

use crate::error::{AppError, AppResult};
use crate::mcp::types::InstalledServer;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Sandbox status — surfaces in the UI badge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStatus {
    /// Sandbox engaged — Landlock + seccomp active.
    Enforced,
    /// Sandbox supported but explicitly disabled by the user.
    Disabled,
    /// Sandbox unsupported on this platform / kernel.
    Unsupported,
    /// Sandbox engagement failed at runtime.
    Failed { reason: String },
}

/// Plan that the server process should run inside.  Built before spawn,
/// applied just before exec().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPlan {
    pub server_id: String,
    pub fs_allowlist: Vec<std::path::PathBuf>,
    pub net_allowlist: Vec<String>,
    pub deny_dangerous_syscalls: bool,
    pub status: SandboxStatus,
}

/// Build a plan for one installed server.
pub fn plan(server: &InstalledServer) -> SandboxPlan {
    let status = if cfg!(target_os = "linux") {
        // Don't actually call the kernel here — `apply` does that.  We
        // optimistically report `Enforced` on Linux; `apply` flips to
        // `Failed` if the kernel rejects.
        SandboxStatus::Enforced
    } else {
        SandboxStatus::Unsupported
    };
    SandboxPlan {
        server_id: server.id.clone(),
        fs_allowlist: server.fs_allowlist.clone(),
        net_allowlist: server.net_allowlist.clone(),
        deny_dangerous_syscalls: true,
        status,
    }
}

/// Apply the plan to the current process.  Called inside the spawned
/// child via a `pre_exec` hook in Pro — for MIT we just expose the
/// callable and let the runner wire it.
///
/// On non-Linux this is a no-op returning `Status::Unsupported`.
pub fn apply(plan: &SandboxPlan) -> AppResult<SandboxStatus> {
    if cfg!(target_os = "linux") {
        apply_linux(plan)
    } else {
        Ok(SandboxStatus::Unsupported)
    }
}

#[cfg(target_os = "linux")]
fn apply_linux(plan: &SandboxPlan) -> AppResult<SandboxStatus> {
    use landlock::{
        Access, AccessFs, PathBeneath, PathFd, RestrictionStatus, Ruleset, RulesetAttr,
        RulesetCreatedAttr, ABI,
    };

    // Probe the kernel — if Landlock isn't supported we skip cleanly.
    let abi = ABI::V3;
    let access_all = AccessFs::from_all(abi);
    let ruleset = match Ruleset::default()
        .handle_access(access_all)
        .map_err(|e| AppError::Internal(format!("landlock handle_access: {e}")))
    {
        Ok(r) => r,
        Err(_) => return Ok(SandboxStatus::Unsupported),
    };
    let rs_initial = match ruleset.create() {
        Ok(c) => c,
        Err(e) => {
            return Ok(SandboxStatus::Failed {
                reason: format!("create ruleset: {e}"),
            })
        }
    };

    // `add_rule` consumes `self` and returns a new RulesetCreated — fold
    // through the loop rebinding each step.
    let mut rs_created = rs_initial;
    for path in &plan.fs_allowlist {
        if !path.exists() {
            continue;
        }
        let fd = match PathFd::new(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let rule = PathBeneath::new(fd, access_all);
        rs_created = match rs_created.add_rule(rule) {
            Ok(c) => c,
            Err(e) => {
                return Ok(SandboxStatus::Failed {
                    reason: format!("add fs rule: {e}"),
                })
            }
        };
    }

    let status = match rs_created.restrict_self() {
        Ok(RestrictionStatus { no_new_privs, .. }) => {
            if no_new_privs {
                SandboxStatus::Enforced
            } else {
                SandboxStatus::Failed {
                    reason: "no_new_privs not honoured".into(),
                }
            }
        }
        Err(e) => SandboxStatus::Failed {
            reason: format!("restrict_self: {e}"),
        },
    };

    if plan.deny_dangerous_syscalls {
        if let Err(reason) = apply_seccomp_deny() {
            return Ok(SandboxStatus::Failed { reason });
        }
    }

    Ok(status)
}

#[cfg(not(target_os = "linux"))]
fn apply_linux(_plan: &SandboxPlan) -> AppResult<SandboxStatus> {
    Ok(SandboxStatus::Unsupported)
}

#[cfg(target_os = "linux")]
fn apply_seccomp_deny() -> Result<(), String> {
    use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule};
    use std::collections::BTreeMap;

    // Block obviously-dangerous syscalls.  Allow the rest — locking down
    // every syscall a `npx` / `uvx` bootstrap needs is impractical for
    // the MIT tier.  Pro's Bubblewrap profile is the proper answer.
    //
    // syscall-number constants per linux-next x86_64 abi.
    const SYS_PTRACE: i64 = 101;
    const SYS_KEXEC_LOAD: i64 = 246;
    const SYS_FINIT_MODULE: i64 = 313;
    const SYS_INIT_MODULE: i64 = 175;
    const SYS_DELETE_MODULE: i64 = 176;
    const SYS_BPF: i64 = 321;
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    rules.insert(SYS_PTRACE, vec![]);
    rules.insert(SYS_KEXEC_LOAD, vec![]);
    rules.insert(SYS_FINIT_MODULE, vec![]);
    rules.insert(SYS_INIT_MODULE, vec![]);
    rules.insert(SYS_DELETE_MODULE, vec![]);
    rules.insert(SYS_BPF, vec![]);

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc_eperm()),
        target_arch(),
    )
    .map_err(|e| format!("build seccomp filter: {e}"))?;
    let program: BpfProgram = filter
        .try_into()
        .map_err(|e| format!("compile seccomp BPF: {e}"))?;
    seccompiler::apply_filter(&program).map_err(|e| format!("apply seccomp: {e}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn target_arch() -> seccompiler::TargetArch {
    if cfg!(target_arch = "x86_64") {
        seccompiler::TargetArch::x86_64
    } else if cfg!(target_arch = "aarch64") {
        seccompiler::TargetArch::aarch64
    } else {
        seccompiler::TargetArch::x86_64
    }
}

#[cfg(target_os = "linux")]
fn libc_eperm() -> u32 {
    1 // EPERM
}

/// Validate that a path is INSIDE the allowlist — used by the runner
/// before issuing tool calls that take a path argument.  Defence-in-depth
/// alongside Landlock.
pub fn path_allowed(path: &Path, allowlist: &[std::path::PathBuf]) -> bool {
    if allowlist.is_empty() {
        return false;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    allowlist.iter().any(|allowed| {
        let allowed = allowed
            .canonicalize()
            .unwrap_or_else(|_| allowed.clone());
        canonical.starts_with(&allowed)
    })
}

/// Validate that a host is inside the network allowlist.  `*` wildcard
/// matches everything; `*.example.com` matches subdomains.
pub fn host_allowed(host: &str, allowlist: &[String]) -> bool {
    for allowed in allowlist {
        if allowed == "*" {
            return true;
        }
        if allowed == host {
            return true;
        }
        if let Some(suffix) = allowed.strip_prefix("*.") {
            if host.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn make_server() -> InstalledServer {
        InstalledServer {
            id: "test".into(),
            version: "0.1".into(),
            installed_at: Utc::now(),
            config_path: PathBuf::from("/tmp/test"),
            transport: crate::mcp::types::Transport::Stdio,
            command: None,
            args: vec![],
            env: BTreeMap::new(),
            tool_allowlist: None,
            fs_allowlist: vec![PathBuf::from("/tmp")],
            net_allowlist: vec!["api.example.com".into(), "*.staging.example.com".into()],
            enabled: true,
            bundle_source: None,
        }
    }

    #[test]
    fn plan_carries_server_state() {
        let s = make_server();
        let p = plan(&s);
        assert_eq!(p.server_id, "test");
        assert_eq!(p.fs_allowlist.len(), 1);
        assert_eq!(p.net_allowlist.len(), 2);
        assert!(p.deny_dangerous_syscalls);
    }

    #[test]
    fn plan_status_is_unsupported_on_non_linux_or_enforced_on_linux() {
        let s = make_server();
        let p = plan(&s);
        if cfg!(target_os = "linux") {
            assert_eq!(p.status, SandboxStatus::Enforced);
        } else {
            assert_eq!(p.status, SandboxStatus::Unsupported);
        }
    }

    #[test]
    fn host_allowed_exact_match() {
        let allowlist = vec!["api.example.com".into()];
        assert!(host_allowed("api.example.com", &allowlist));
        assert!(!host_allowed("evil.com", &allowlist));
    }

    #[test]
    fn host_allowed_wildcard_subdomain() {
        let allowlist = vec!["*.example.com".into()];
        assert!(host_allowed("a.example.com", &allowlist));
        assert!(host_allowed("a.b.example.com", &allowlist));
        assert!(!host_allowed("evil.com", &allowlist));
    }

    #[test]
    fn host_allowed_global_wildcard_matches_everything() {
        let allowlist = vec!["*".into()];
        assert!(host_allowed("anything.example.com", &allowlist));
    }

    #[test]
    fn host_allowed_empty_allowlist_denies_all() {
        let allowlist: Vec<String> = vec![];
        assert!(!host_allowed("anything.example.com", &allowlist));
    }

    #[test]
    fn path_allowed_rejects_path_outside_allowlist() {
        let allowlist = vec![PathBuf::from("/tmp")];
        assert!(path_allowed(Path::new("/tmp/x"), &allowlist));
        assert!(!path_allowed(Path::new("/etc/passwd"), &allowlist));
    }

    #[test]
    fn path_allowed_empty_allowlist_denies_all() {
        let allowlist: Vec<PathBuf> = vec![];
        assert!(!path_allowed(Path::new("/tmp"), &allowlist));
    }
}
