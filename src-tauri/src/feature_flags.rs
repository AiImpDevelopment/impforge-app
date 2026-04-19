// SPDX-License-Identifier: MIT
//! Feature-flag registry. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.1.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real persistence at `~/.impforge-app/features.json` with diff-aware writes.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// High-level feature category buckets used by the UI grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureCategory {
    AiCore,
    Knowledge,
    Privacy,
    Digiimp,
    Compliance,
}

/// One toggleable feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: FeatureCategory,
    pub enabled: bool,
    pub default_enabled: bool,
}

/// Aggregate counters for the feature-flag registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlagStats {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
}

/// List all registered feature flags.
#[tauri::command]
pub async fn feature_flag_list() -> AppResult<Vec<FeatureFlag>> {
    Err(AppError::Internal(
        "feature_flags::feature_flag_list not implemented in Phase 1".into(),
    ))
}

/// Set the enabled state of the flag identified by `id`.
#[tauri::command]
pub async fn feature_flag_set(_id: String, _enabled: bool) -> AppResult<()> {
    Err(AppError::Internal(
        "feature_flags::feature_flag_set not implemented in Phase 1".into(),
    ))
}

/// Return aggregate counters across the entire flag registry.
#[tauri::command]
pub async fn feature_flag_stats() -> AppResult<FeatureFlagStats> {
    Err(AppError::Internal(
        "feature_flags::feature_flag_stats not implemented in Phase 1".into(),
    ))
}

/// Reset all flags to their default-enabled state.
#[tauri::command]
pub async fn feature_flag_reset_defaults() -> AppResult<()> {
    Err(AppError::Internal(
        "feature_flags::feature_flag_reset_defaults not implemented in Phase 1".into(),
    ))
}
