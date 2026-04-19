// SPDX-License-Identifier: MIT
//! Bridge between the host Tauri app and the DigiImp animated companion.
//! MIT-rewritten per `docs/superpowers/specs/2026-04-19-digiimp-design.md` §5.3.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real wgpu-driven runtime state mutator + IPC channel.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Top-level animated state of the DigiImp companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigiImpState {
    Idle,
    Listening,
    Thinking,
    Responding,
    Error,
    Celebrating,
    Sleeping,
    Alerting,
    Acknowledging,
    Adventuring,
}

/// Pre-defined colour presets (rendered as kebab-case for design-system parity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigiImpColorPreset {
    NeonGreen,
    Cyan,
    Magenta,
    Amber,
}

/// Window placement / display modes for the DigiImp companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigiImpDisplayMode {
    Floating,
    Docked,
    Ambient,
    Hidden,
}

/// Aggregate runtime state describing how the companion currently looks + behaves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigiImpRuntimeState {
    pub state: DigiImpState,
    pub state_progress: f32,
    pub energy: f32,
    pub glow_intensity: f32,
    pub color_preset: DigiImpColorPreset,
    pub display_mode: DigiImpDisplayMode,
}

/// Duration choices for putting the companion to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestDuration {
    OneHour,
    FourHours,
    Today,
    Forever,
}

/// Set the high-level animated state and its progress in [0.0, 1.0].
#[tauri::command]
pub async fn digiimp_set_state(_state: DigiImpState, _progress: f32) -> AppResult<()> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_set_state not implemented in Phase 1".into(),
    ))
}

/// Set the energy level (0.0 = depleted, 1.0 = full).
#[tauri::command]
pub async fn digiimp_set_energy(_energy: f32) -> AppResult<()> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_set_energy not implemented in Phase 1".into(),
    ))
}

/// Set the glow intensity (0.0 = off, 1.0 = max).
#[tauri::command]
pub async fn digiimp_set_glow(_intensity: f32) -> AppResult<()> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_set_glow not implemented in Phase 1".into(),
    ))
}

/// Switch the active colour preset.
#[tauri::command]
pub async fn digiimp_set_color_preset(_preset: DigiImpColorPreset) -> AppResult<()> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_set_color_preset not implemented in Phase 1".into(),
    ))
}

/// Switch the active display mode.
#[tauri::command]
pub async fn digiimp_set_display_mode(_mode: DigiImpDisplayMode) -> AppResult<()> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_set_display_mode not implemented in Phase 1".into(),
    ))
}

/// Read the current aggregate runtime state.
#[tauri::command]
pub async fn digiimp_get_state() -> AppResult<DigiImpRuntimeState> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_get_state not implemented in Phase 1".into(),
    ))
}

/// Put the companion to rest for the given duration.
#[tauri::command]
pub async fn digiimp_rest_mode(_duration: RestDuration) -> AppResult<()> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_rest_mode not implemented in Phase 1".into(),
    ))
}

/// Wake the companion from rest mode immediately.
#[tauri::command]
pub async fn digiimp_wake() -> AppResult<()> {
    Err(AppError::Internal(
        "digiimp_bridge::digiimp_wake not implemented in Phase 1".into(),
    ))
}
