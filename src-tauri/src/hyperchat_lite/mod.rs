// SPDX-License-Identifier: MIT
//! HyperChat Lite — chat / edit / agent mode machine, event stream, and session state.
//!
//! MIT-rewritten per `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md`
//! §4.2 SHIP-DEGRADED #2. Pro creator/monaco/xterm bridges removed.
//!
//! Phase 2: real mode-machine + MPSC event bus + memory-only session manager.

pub mod event_stream;
pub mod modes;
pub mod session;

pub use event_stream::{hub, Event, EventHub, EventStreamStats, EventSubscription};
pub use modes::{transition, HyperChatMode, ModeState, ModeTransition, TransitionError};
pub use session::{HyperChatSession, MAX_RESIDENT_EVENTS};

use crate::error::{AppError, AppResult};

/// Attempt to transition the HyperChat mode from `from` to `to`.
///
/// Same-mode transition surfaces as `AppError::InvalidArgument` carrying the
/// `TransitionError`'s display string.
#[tauri::command]
pub async fn hyperchat_mode_transition(
    from: modes::HyperChatMode,
    to: modes::HyperChatMode,
) -> AppResult<modes::ModeTransition> {
    transition(from, to).map_err(|e| AppError::InvalidArgument(e.to_string()))
}

/// Return current event-bus statistics (subscribers + events_published).
#[tauri::command]
pub async fn hyperchat_event_stats() -> AppResult<event_stream::EventStreamStats> {
    Ok(hub().stats())
}

/// Create a new HyperChat session and return its initial state.
#[tauri::command]
pub async fn hyperchat_session_new() -> AppResult<session::HyperChatSession> {
    Ok(session::HyperChatSession::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cmd_session_new_returns_fresh_session() {
        let s = hyperchat_session_new().await.expect("ok");
        assert!(!s.id.is_empty());
        assert!(s.messages.is_empty());
    }

    #[tokio::test]
    async fn cmd_mode_transition_chat_to_edit() {
        let t = hyperchat_mode_transition(HyperChatMode::Chat, HyperChatMode::Edit)
            .await
            .expect("ok");
        assert_eq!(t.from, HyperChatMode::Chat);
        assert_eq!(t.to, HyperChatMode::Edit);
    }

    #[tokio::test]
    async fn cmd_mode_transition_same_mode_errors() {
        let err = hyperchat_mode_transition(HyperChatMode::Chat, HyperChatMode::Chat)
            .await
            .expect_err("same mode");
        match err {
            AppError::InvalidArgument(_) => {}
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }
}
