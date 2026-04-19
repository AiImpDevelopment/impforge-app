// SPDX-License-Identifier: MIT
//! Cannibalism-gated upgrade-nudge engine. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §6
//! (Cannibalism Gating Model).
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real cadence-aware nudge evaluator + dismiss persistence.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Concrete categories of upgrade nudges that the engine can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NudgeKind {
    PersistenceLoss,
    SearchDepth,
    KnowledgeLink,
    EntityExtract,
    IntentDispatch,
    StyleLearn,
    AutonomyAudit,
    WidgetCustom,
    TransportRich,
}

/// One nudge-triggering event observed by an in-app module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NudgeEvent {
    pub kind: NudgeKind,
    pub trigger_module: String,
    pub trigger_at: chrono::DateTime<chrono::Utc>,
    pub session_id: String,
}

/// Decision returned by the nudge engine for one event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NudgeResult {
    pub should_show: bool,
    pub copy: String,
    pub utm_url: String,
}

/// Evaluate one nudge event and decide whether (and what) to show.
#[tauri::command]
pub async fn nudge_evaluate(_event: NudgeEvent) -> AppResult<NudgeResult> {
    Err(AppError::Internal(
        "upgrade_nudge::nudge_evaluate not implemented in Phase 1".into(),
    ))
}

/// Dismiss the named nudge category (optionally forever).
#[tauri::command]
pub async fn nudge_dismiss(_kind: NudgeKind, _forever: bool) -> AppResult<()> {
    Err(AppError::Internal(
        "upgrade_nudge::nudge_dismiss not implemented in Phase 1".into(),
    ))
}

/// Globally enable or disable all nudges.
#[tauri::command]
pub async fn nudge_global_disable(_disabled: bool) -> AppResult<()> {
    Err(AppError::Internal(
        "upgrade_nudge::nudge_global_disable not implemented in Phase 1".into(),
    ))
}
