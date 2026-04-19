// SPDX-License-Identifier: MIT
//! Shared types for the MCP plugin marketplace.
//!
//! Mirrors `impforge-cli`'s marketplace types (App and CLI evolve
//! independently, no shared crate yet — the App layers on top with
//! richer capability + invocation types).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Top-level marketplace category (also drives the left-rail filter chips).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Files,
    Web,
    Comms,
    Code,
    Data,
    Ai,
    Custom,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Files => "files",
            Category::Web => "web",
            Category::Comms => "comms",
            Category::Code => "code",
            Category::Data => "data",
            Category::Ai => "ai",
            Category::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "files" => Some(Self::Files),
            "web" => Some(Self::Web),
            "comms" => Some(Self::Comms),
            "code" => Some(Self::Code),
            "data" => Some(Self::Data),
            "ai" => Some(Self::Ai),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    /// Every category — used by the left-rail filter chips.
    pub const ALL: [Category; 7] = [
        Category::Files,
        Category::Web,
        Category::Comms,
        Category::Code,
        Category::Data,
        Category::Ai,
        Category::Custom,
    ];
}

/// Transport carriage for an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Stdio,
    StreamableHttp,
    Sse,
}

impl Transport {
    pub fn as_str(&self) -> &'static str {
        match self {
            Transport::Stdio => "stdio",
            Transport::StreamableHttp => "streamable_http",
            Transport::Sse => "sse",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "streamable_http" | "streamable-http" | "http" => Self::StreamableHttp,
            "sse" => Self::Sse,
            _ => Self::Stdio,
        }
    }
}

/// One entry in the marketplace catalog (mirrored from the public CDN).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceEntry {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub description: String,
    pub category: Category,
    pub tags: Vec<String>,
    pub transport: Transport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env_schema: Vec<EnvVarSpec>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub install_count: u64,
    pub stars: u32,
    pub verified: bool,
    pub signed: bool,
    pub oauth: bool,
    pub last_updated: DateTime<Utc>,
    pub capability_hint: CapabilityHint,
}

/// Schema for one env var the user needs to fill in before install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarSpec {
    pub name: String,
    pub description: String,
    /// `true` for secrets — UI will show a masked input + reveal toggle.
    pub secret: bool,
    /// `true` for required — UI blocks install button until set.
    pub required: bool,
}

/// Quick summary of advertised capabilities (for marketplace cards before install).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityHint {
    pub tools: u32,
    pub resources: u32,
    pub prompts: u32,
}

/// Full capability schema fetched after install (includes per-tool detail).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub server_id: String,
    pub server_version: String,
    pub tools: Vec<ToolSpec>,
    pub resources: Vec<ResourceSpec>,
    pub prompts: Vec<PromptSpec>,
    /// Hash of the canonical capability list — drives the capability-diff
    /// rug-pull defense (per ETDI arXiv 2506.01333).
    pub capability_hash: String,
    /// When this snapshot was taken.
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    /// MUST be the full description from the server — no UI truncation
    /// per Invariant Labs tool-poisoning notification.
    pub description: String,
    /// JSON schema for arguments (rendered by `ToolSchemaRenderer.svelte`).
    pub input_schema: serde_json::Value,
    /// Egress hosts the tool will hit — empty if tool is purely local.
    pub egress_hosts: Vec<String>,
    /// Filesystem paths the tool will touch — empty if tool is purely network.
    pub fs_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSpec {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSpec {
    pub name: String,
    pub description: String,
    pub arguments: Vec<serde_json::Value>,
}

/// One installed server on the user's machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledServer {
    pub id: String,
    pub version: String,
    pub installed_at: DateTime<Utc>,
    pub config_path: PathBuf,
    pub transport: Transport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// Tool allowlist — `None` means "all tools allowed".  `Some(Vec)`
    /// means "only these tool names may be invoked" (per-tool capability
    /// allowlists, per Multikernel pattern).
    pub tool_allowlist: Option<Vec<String>>,
    /// Filesystem paths granted to the server (per Landlock sandbox).
    pub fs_allowlist: Vec<PathBuf>,
    /// Network hosts granted to the server.
    pub net_allowlist: Vec<String>,
    /// Whether the server is currently enabled (toggled in `InstalledView`).
    pub enabled: bool,
    /// True if this entry was installed as part of a bundle — surfaces
    /// in UI with a "from <bundle name>" chip.
    pub bundle_source: Option<String>,
}

/// Trust score breakdown — separated from the score itself so the UI
/// can render a tooltip explaining why the score is what it is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScore {
    pub score: u8,
    pub verified_pts: u8,
    pub signed_pts: u8,
    pub installs_pts: u8,
    pub stars_pts: u8,
    pub recency_pts: u8,
    pub sandbox_pts: u8,
}

impl TrustScore {
    /// Compute the trust score from a marketplace entry.  Same formula
    /// as `impforge-cli` — keeps the score consistent across tiers.
    pub fn compute(entry: &MarketplaceEntry) -> Self {
        let verified_pts = if entry.verified { 30 } else { 0 };
        let signed_pts = if entry.signed { 20 } else { 0 };
        let installs_log = ((entry.install_count.max(1)) as f64).log10().min(6.0);
        let installs_pts = (installs_log * 2.5).round().clamp(0.0, 15.0) as u8;
        let stars_pts = (entry.stars.min(5) as u8).saturating_mul(3);
        let recency_pts = recency_score_u8(entry.last_updated);
        let sandbox_pts = if entry.capability_hint.tools > 0 { 10 } else { 0 };
        let total = (verified_pts as u16
            + signed_pts as u16
            + installs_pts as u16
            + stars_pts as u16
            + recency_pts as u16
            + sandbox_pts as u16)
            .min(100);
        Self {
            score: total as u8,
            verified_pts,
            signed_pts,
            installs_pts,
            stars_pts,
            recency_pts,
            sandbox_pts,
        }
    }
}

fn recency_score_u8(dt: DateTime<Utc>) -> u8 {
    let age_days = (Utc::now() - dt).num_days();
    if age_days < 0 {
        return 10;
    }
    if age_days <= 7 {
        return 10;
    }
    if age_days >= 365 {
        return 0;
    }
    let f = 10.0 * (1.0 - (age_days as f64 - 7.0) / (365.0 - 7.0));
    f.round().clamp(0.0, 10.0) as u8
}

/// Health snapshot for one running server (driven by `health.rs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerHealth {
    pub server_id: String,
    pub running: bool,
    /// Last 100 calls — p95 latency in milliseconds.
    pub p95_latency_ms: Option<u64>,
    /// Success rate over last 100 calls (0.0 – 1.0).
    pub success_rate: f32,
    pub calls_last_hour: u64,
    pub restart_count: u32,
    pub last_error: Option<String>,
    pub uptime_seconds: u64,
}

/// One row in the tamper-evident invocation ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub seq: u64,
    pub timestamp: DateTime<Utc>,
    pub server_id: String,
    pub tool_name: String,
    /// SHA-256 of the canonicalised arguments JSON.
    pub args_hash: String,
    /// SHA-256 of the canonicalised result JSON.
    pub result_hash: String,
    /// SHA-256 of the previous row — chains the ledger.
    pub prev_hash: String,
    /// SHA-256 of `(timestamp, server_id, tool_name, args_hash, result_hash, prev_hash)`.
    pub row_hash: String,
    pub success: bool,
    pub latency_ms: u64,
}

/// Ledger verification result — the App badge ("tamper-evident") goes
/// neon-green when this is `Ok` and orange when `Tampered`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerVerification {
    Ok { rows: u64 },
    Tampered { broken_seq: u64, reason: String },
    Empty,
}

/// One outgoing tool call — drives `runner::invoke`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub server_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

/// Result of a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub success: bool,
    pub result: serde_json::Value,
    pub latency_ms: u64,
    pub error: Option<String>,
}

/// One bundle template — installed via `bundle::install`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub server_ids: Vec<String>,
    /// Pre-filled config keys — `(server_id, env_var, value)`.
    pub prefilled_env: Vec<(String, String, String)>,
}

/// Diff produced by `capability_diff::compare` — displayed before
/// `apply_update` is called, per ETDI rug-pull defense.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDiff {
    pub server_id: String,
    pub from_version: String,
    pub to_version: String,
    pub added_tools: Vec<ToolSpec>,
    pub removed_tools: Vec<ToolSpec>,
    pub changed_tools: Vec<ToolDiff>,
    pub added_egress_hosts: Vec<String>,
    pub removed_egress_hosts: Vec<String>,
    pub added_fs_paths: Vec<String>,
    pub removed_fs_paths: Vec<String>,
    /// `true` if the diff broadens the server's capabilities (added
    /// tools, added egress, removed allowlist constraints).  UI shows
    /// this in orange and requires explicit re-consent.
    pub privilege_escalation: bool,
}

/// One tool that exists on both sides but changed in some way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDiff {
    pub name: String,
    pub from: ToolSpec,
    pub to: ToolSpec,
    /// Free-form list of human-readable changes.
    pub changes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_template() -> MarketplaceEntry {
        MarketplaceEntry {
            id: "t".into(),
            name: "T".into(),
            publisher: "p".into(),
            version: "0.1".into(),
            description: "x".into(),
            category: Category::Code,
            tags: vec![],
            transport: Transport::Stdio,
            command: None,
            args: vec![],
            env_schema: vec![],
            homepage: None,
            license: None,
            install_count: 100_000,
            stars: 5,
            verified: true,
            signed: true,
            oauth: false,
            last_updated: Utc::now(),
            capability_hint: CapabilityHint {
                tools: 5,
                resources: 1,
                prompts: 0,
            },
        }
    }

    #[test]
    fn category_round_trip() {
        for c in Category::ALL.iter().copied() {
            assert_eq!(Category::parse(c.as_str()), Some(c));
        }
        assert_eq!(Category::parse("UNKNOWN"), None);
    }

    #[test]
    fn transport_parse_falls_back_to_stdio() {
        assert_eq!(Transport::parse("stdio"), Transport::Stdio);
        assert_eq!(Transport::parse("streamable-http"), Transport::StreamableHttp);
        assert_eq!(Transport::parse("HTTP"), Transport::StreamableHttp);
        assert_eq!(Transport::parse("sse"), Transport::Sse);
        assert_eq!(Transport::parse("garbage"), Transport::Stdio);
    }

    #[test]
    fn trust_score_breakdown_caps_at_100() {
        let score = TrustScore::compute(&entry_template());
        assert!(score.score <= 100, "score must cap at 100, got {}", score.score);
        assert_eq!(score.verified_pts, 30);
        assert_eq!(score.signed_pts, 20);
        assert!(score.installs_pts >= 12);
        assert_eq!(score.stars_pts, 15);
        assert!(score.recency_pts >= 9);
        assert_eq!(score.sandbox_pts, 10);
    }

    #[test]
    fn trust_score_anonymous_low() {
        let mut e = entry_template();
        e.verified = false;
        e.signed = false;
        e.install_count = 1;
        e.stars = 0;
        e.last_updated = "2020-01-01T00:00:00Z".parse().expect("parse iso");
        e.capability_hint = CapabilityHint::default();
        let score = TrustScore::compute(&e);
        assert!(score.score <= 5, "anon should score <=5, got {}", score.score);
    }

    #[test]
    fn recency_modern_is_10() {
        assert_eq!(recency_score_u8(Utc::now()), 10);
    }

    #[test]
    fn recency_one_year_old_is_zero() {
        let dt: DateTime<Utc> = "2020-01-01T00:00:00Z".parse().expect("parse");
        assert_eq!(recency_score_u8(dt), 0);
    }
}
