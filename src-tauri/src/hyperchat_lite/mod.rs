// SPDX-License-Identifier: MIT
//! HyperChat Lite — chat / edit / agent mode machine, event stream, and session state.
//!
//! MIT-rewritten per `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md`
//! §4.2 SHIP-DEGRADED #2. Pro creator/monaco/xterm bridges removed.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real mode-machine + Tokio broadcast bus + session manager.

pub mod event_stream;
pub mod modes;
pub mod session;

use crate::error::{AppError, AppResult};

/// Attempt to transition the HyperChat mode from `from` to `to`.
#[tauri::command]
pub async fn hyperchat_mode_transition(
    _from: modes::HyperChatMode,
    _to: modes::HyperChatMode,
) -> AppResult<modes::ModeTransition> {
    Err(AppError::Internal(
        "hyperchat_lite::hyperchat_mode_transition not implemented in Phase 1".into(),
    ))
}

/// Return current event-bus statistics.
#[tauri::command]
pub async fn hyperchat_event_stats() -> AppResult<event_stream::EventStreamStats> {
    Err(AppError::Internal(
        "hyperchat_lite::hyperchat_event_stats not implemented in Phase 1".into(),
    ))
}

/// Create a new HyperChat session and return its initial state.
#[tauri::command]
pub async fn hyperchat_session_new() -> AppResult<session::HyperChatSession> {
    Err(AppError::Internal(
        "hyperchat_lite::hyperchat_session_new not implemented in Phase 1".into(),
    ))
}
