// SPDX-License-Identifier: MIT
//! 22 Tauri commands wired through `lib.rs`.  Each command is a thin
//! wrapper over the matching `mcp::*` API — keeps `lib.rs` lean.

use crate::error::{AppError, AppResult};
use crate::mcp::bundle::{self, BundleInstallResult};
use crate::mcp::capability_diff::{self};
use crate::mcp::health::{self, HealthRow};
use crate::mcp::installer::{self, InstallRequest, InstallResult};
use crate::mcp::ledger;
use crate::mcp::marketplace::{self, BrowseFilters, OfflineStatus};
use crate::mcp::oauth::{self, CompleteFlowResult, PendingFlow, StartFlowRequest, StoredTokens};
use crate::mcp::runner::{self, RuntimeSnapshot};
use crate::mcp::sandbox::{self, SandboxPlan, SandboxStatus};
use crate::mcp::types::{
    BundleTemplate, CapabilityDiff, InstalledServer, LedgerEntry, LedgerVerification,
    MarketplaceEntry, ServerCapabilities, ToolCall, ToolCallResult, ToolSpec, TrustScore,
};

// ─── Marketplace ────────────────────────────────────────────────────────────

/// Browse the offline-first marketplace mirror.
#[tauri::command]
pub fn mcp_marketplace_browse(filters: BrowseFilters) -> AppResult<Vec<MarketplaceEntry>> {
    marketplace::browse(&filters)
}

/// Browse + filter by free-text search (BrowseView search input).
#[tauri::command]
pub fn mcp_marketplace_search(query: String, limit: Option<u32>) -> AppResult<Vec<MarketplaceEntry>> {
    marketplace::browse(&BrowseFilters {
        search: Some(query),
        limit,
        ..Default::default()
    })
}

/// Sync the marketplace mirror from the public CDN.
#[tauri::command]
pub async fn mcp_marketplace_sync_mirror(url: Option<String>) -> AppResult<usize> {
    marketplace::sync_mirror(url).await
}

/// Offline status — entry count + last refresh + DB path.
#[tauri::command]
pub fn mcp_marketplace_get_offline_status() -> AppResult<OfflineStatus> {
    marketplace::offline_status()
}

/// Fetch one entry by id (DetailView).
#[tauri::command]
pub fn mcp_marketplace_get(id: String) -> AppResult<MarketplaceEntry> {
    marketplace::get(&id)
}

/// Compute trust score for one entry (drives TrustScoreRing).
#[tauri::command]
pub fn mcp_trust_score(id: String) -> AppResult<TrustScore> {
    let entry = marketplace::get(&id)?;
    Ok(TrustScore::compute(&entry))
}

// ─── Install / lifecycle ────────────────────────────────────────────────────

/// Install one server.
#[tauri::command]
pub fn mcp_install_server(req: InstallRequest) -> AppResult<InstallResult> {
    installer::install(&req)
}

/// Uninstall one server.
#[tauri::command]
pub fn mcp_uninstall_server(id: String) -> AppResult<()> {
    installer::uninstall(&id)
}

/// Pin a server to a specific version.
#[tauri::command]
pub fn mcp_pin_version(id: String, version: String) -> AppResult<InstalledServer> {
    installer::pin_version(&id, &version)
}

/// Update one server to whatever the marketplace currently advertises.
/// UI is expected to call `mcp_capability_diff` BEFORE this so the user
/// can review the diff (rug-pull defence per ETDI arXiv 2506.01333).
#[tauri::command]
pub fn mcp_update_server(id: String) -> AppResult<InstalledServer> {
    installer::update_to_latest(&id)
}

/// List installed servers.
#[tauri::command]
pub fn mcp_list_installed() -> AppResult<Vec<InstalledServer>> {
    installer::load()
}

/// Toggle a server enabled/disabled (no restart).
#[tauri::command]
pub fn mcp_set_enabled(id: String, enabled: bool) -> AppResult<InstalledServer> {
    installer::set_enabled(&id, enabled)
}

/// Override the per-tool allowlist.  `None` = all tools allowed.
#[tauri::command]
pub fn mcp_set_tool_allowlist(
    id: String,
    allowlist: Option<Vec<String>>,
) -> AppResult<InstalledServer> {
    installer::set_tool_allowlist(&id, allowlist)
}

// ─── Runtime ────────────────────────────────────────────────────────────────

/// Snapshot one server's runtime state.
#[tauri::command]
pub fn mcp_get_server_state(id: String) -> AppResult<Option<RuntimeSnapshot>> {
    runner::snapshot(&id)
}

/// Start one server.
#[tauri::command]
pub fn mcp_start_server(id: String) -> AppResult<RuntimeSnapshot> {
    let server = installer::find(&id)?
        .ok_or_else(|| AppError::NotFound(format!("server '{id}' is not installed")))?;
    runner::start(&server)
}

/// Stop one server.
#[tauri::command]
pub fn mcp_stop_server(id: String) -> AppResult<()> {
    runner::stop(&id)
}

/// Invoke one tool — used by the chat panel + tools dashboard.
#[tauri::command]
pub fn mcp_invoke_tool(call: ToolCall) -> AppResult<ToolCallResult> {
    let result = runner::invoke_tool(&call)?;
    // Append to the tamper-evident ledger BEFORE returning, so even if
    // the renderer crashes, the audit trail is intact.
    let _ = ledger::append(
        &call.server_id,
        &call.tool_name,
        &call.arguments,
        &result.result,
        result.success,
        result.latency_ms,
    );
    Ok(result)
}

/// Fetch the tool list for an installed server.
#[tauri::command]
pub fn mcp_get_tool_schema(id: String) -> AppResult<Vec<ToolSpec>> {
    runner::list_tools(&id)
}

// ─── Health ─────────────────────────────────────────────────────────────────

/// Quick ping for a single server.
#[tauri::command]
pub fn mcp_health_ping(id: String) -> AppResult<Option<HealthRow>> {
    health::ping(&id)
}

/// Whole-fleet HealthDashboard data.
#[tauri::command]
pub fn mcp_health_dashboard_data() -> AppResult<Vec<HealthRow>> {
    health::dashboard()
}

// ─── OAuth ──────────────────────────────────────────────────────────────────

/// Start a PKCE flow.
#[tauri::command]
pub fn mcp_oauth_start(req: StartFlowRequest) -> AppResult<PendingFlow> {
    oauth::start_flow(&req)
}

/// Complete an OAuth flow (called by the loopback callback handler).
#[tauri::command]
pub fn mcp_oauth_callback(
    server_id: String,
    code: String,
    state: String,
    client_secret: Option<String>,
) -> AppResult<CompleteFlowResult> {
    oauth::complete_flow(&server_id, &code, &state, client_secret.as_deref())
}

/// Revoke + delete stored tokens.
#[tauri::command]
pub fn mcp_oauth_revoke(server_id: String) -> AppResult<bool> {
    oauth::revoke(&server_id)
}

/// Lookup tokens for a server.
#[tauri::command]
pub fn mcp_oauth_get_tokens(server_id: String) -> AppResult<Option<StoredTokens>> {
    oauth::get_tokens(&server_id)
}

// ─── Capability diff ────────────────────────────────────────────────────────

/// Compare two capability snapshots.  UI calls before update.
#[tauri::command]
pub fn mcp_capability_diff(
    prev: ServerCapabilities,
    next: ServerCapabilities,
) -> AppResult<CapabilityDiff> {
    Ok(capability_diff::compare(&prev, &next))
}

// ─── Ledger ─────────────────────────────────────────────────────────────────

/// Query the tamper-evident invocation ledger.
#[tauri::command]
pub fn mcp_ledger_query(
    server_id: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> AppResult<Vec<LedgerEntry>> {
    ledger::query(server_id.as_deref(), limit.unwrap_or(100), offset.unwrap_or(0))
}

/// Verify the entire chain.  Returns Ok / Tampered / Empty variant.
#[tauri::command]
pub fn mcp_ledger_verify_chain() -> AppResult<LedgerVerification> {
    ledger::verify_chain()
}

/// Export the entire chain as JSONL.
#[tauri::command]
pub fn mcp_ledger_export() -> AppResult<String> {
    ledger::export_jsonl()
}

// ─── Sandbox ────────────────────────────────────────────────────────────────

/// Build the sandbox plan for one server (does not apply — UI surfaces
/// what would be enforced before the user opts in).
#[tauri::command]
pub fn mcp_sandbox_plan(id: String) -> AppResult<SandboxPlan> {
    let server = installer::find(&id)?
        .ok_or_else(|| AppError::NotFound(format!("server '{id}' is not installed")))?;
    Ok(sandbox::plan(&server))
}

/// Read-only status — drives the "sandbox enforced/unsupported" badge.
#[tauri::command]
pub fn mcp_sandbox_status() -> AppResult<SandboxStatus> {
    if cfg!(target_os = "linux") {
        Ok(SandboxStatus::Enforced)
    } else {
        Ok(SandboxStatus::Unsupported)
    }
}

// ─── Bundle ─────────────────────────────────────────────────────────────────

/// List all stock bundle templates.
#[tauri::command]
pub fn mcp_bundle_list() -> AppResult<Vec<BundleTemplate>> {
    Ok(bundle::stock_bundles())
}

/// Install one bundle.
#[tauri::command]
pub fn mcp_bundle_install(bundle_id: String) -> AppResult<Vec<BundleInstallResult>> {
    bundle::install_bundle(&bundle_id)
}
