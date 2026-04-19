// SPDX-License-Identifier: MIT
//! Lightweight widget framework: Clock / Notes / Monitor (3 built-ins).
//! MIT-rewritten per `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real widget process supervisor + power-mode scheduler.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Built-in widget categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    Clock,
    Notes,
    Monitor,
}

/// Power-management state for a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetPowerMode {
    DeepSleep,
    Idle,
    Active,
}

/// Snapshot of one widget's runtime state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetState {
    pub id: String,
    pub kind: WidgetKind,
    pub power_mode: WidgetPowerMode,
    pub bytes_used: u64,
}

/// Create and register a new widget of the given kind.
#[tauri::command]
pub async fn widget_create(_kind: WidgetKind) -> AppResult<WidgetState> {
    Err(AppError::Internal(
        "widgets_lite::widget_create not implemented in Phase 1".into(),
    ))
}

/// List all registered widgets.
#[tauri::command]
pub async fn widget_list() -> AppResult<Vec<WidgetState>> {
    Err(AppError::Internal(
        "widgets_lite::widget_list not implemented in Phase 1".into(),
    ))
}

/// Suspend the widget identified by `id` into low-power mode.
#[tauri::command]
pub async fn widget_suspend(_id: String) -> AppResult<()> {
    Err(AppError::Internal(
        "widgets_lite::widget_suspend not implemented in Phase 1".into(),
    ))
}

/// Resume the widget identified by `id` from low-power mode.
#[tauri::command]
pub async fn widget_resume(_id: String) -> AppResult<()> {
    Err(AppError::Internal(
        "widgets_lite::widget_resume not implemented in Phase 1".into(),
    ))
}

/// Close and unregister the widget identified by `id`.
#[tauri::command]
pub async fn widget_close(_id: String) -> AppResult<()> {
    Err(AppError::Internal(
        "widgets_lite::widget_close not implemented in Phase 1".into(),
    ))
}
