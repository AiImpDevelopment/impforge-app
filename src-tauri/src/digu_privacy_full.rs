// SPDX-License-Identifier: MIT
//! Brand-defining 5-tier privacy system. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.1 SHIP-AS-IS-EQUIVALENT.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real per-tier policy enforcement with persistent settings.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Privacy tier ordered from most-restrictive (`Fortress`) to most-permissive (`Open`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyTier {
    Fortress,
    Vault,
    Shield,
    Bridge,
    Open,
}

/// The concrete policy associated with a privacy tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub tier: PrivacyTier,
    pub allow_cloud: bool,
    pub allow_telemetry: bool,
    pub allow_external_share: bool,
    pub require_pii_scrub: bool,
}

/// Categories of operations the privacy gate can rule on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyOperation {
    Inference,
    Storage,
    Network,
    Telemetry,
    Sharing,
}

/// One privacy-gate decision with a human-readable explanation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyDecision {
    pub allowed: bool,
    pub reason: String,
}

/// Return the currently-active privacy tier.
#[tauri::command]
pub async fn privacy_get_tier() -> AppResult<PrivacyTier> {
    Err(AppError::Internal(
        "digu_privacy_full::privacy_get_tier not implemented in Phase 1".into(),
    ))
}

/// Set the active privacy tier (persisted across restarts).
#[tauri::command]
pub async fn privacy_set_tier(_tier: PrivacyTier) -> AppResult<()> {
    Err(AppError::Internal(
        "digu_privacy_full::privacy_set_tier not implemented in Phase 1".into(),
    ))
}

/// Return the policy associated with the current tier.
#[tauri::command]
pub async fn privacy_get_policy() -> AppResult<PrivacyPolicy> {
    Err(AppError::Internal(
        "digu_privacy_full::privacy_get_policy not implemented in Phase 1".into(),
    ))
}

/// Decide whether the requested operation is permitted under the current policy.
#[tauri::command]
pub async fn privacy_check_operation(_op: PrivacyOperation) -> AppResult<PrivacyDecision> {
    Err(AppError::Internal(
        "digu_privacy_full::privacy_check_operation not implemented in Phase 1".into(),
    ))
}
