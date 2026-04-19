// SPDX-License-Identifier: MIT
//! Mode-state machine for HyperChat Lite (Chat / Edit / Agent).
//!
//! Phase 2: real `transition()` implementation. Same-mode transition is an
//! explicit error (no-op semantics carried by the error variant — the host
//! decides whether to ignore or surface it).

use serde::{Deserialize, Serialize};

/// Three operational modes for the HyperChat experience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HyperChatMode {
    Chat,
    Edit,
    Agent,
}

/// Current mode + transition history (most-recent first when populated).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeState {
    pub current: HyperChatMode,
    pub history: Vec<HyperChatMode>,
}

impl ModeState {
    /// Fresh state starting in Chat mode with a one-element history.
    pub fn fresh() -> Self {
        Self {
            current: HyperChatMode::Chat,
            history: vec![HyperChatMode::Chat],
        }
    }
}

impl Default for ModeState {
    fn default() -> Self {
        Self::fresh()
    }
}

/// One transition from a previous mode to a new one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeTransition {
    pub from: HyperChatMode,
    pub to: HyperChatMode,
}

/// Errors that can occur when attempting to switch modes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionError {
    /// Caller attempted to transition into the mode that is already active.
    SameMode { mode: HyperChatMode },
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SameMode { mode } => write!(f, "already in {mode:?}"),
        }
    }
}

impl std::error::Error for TransitionError {}

/// Transition from `from` to `to`. Same-mode transition returns `SameMode`.
pub fn transition(
    from: HyperChatMode,
    to: HyperChatMode,
) -> Result<ModeTransition, TransitionError> {
    if from == to {
        return Err(TransitionError::SameMode { mode: from });
    }
    Ok(ModeTransition { from, to })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_state_is_chat_with_one_element_history() {
        let s = ModeState::fresh();
        assert_eq!(s.current, HyperChatMode::Chat);
        assert_eq!(s.history, vec![HyperChatMode::Chat]);
    }

    #[test]
    fn transition_chat_to_edit_returns_pair() {
        let t = transition(HyperChatMode::Chat, HyperChatMode::Edit).expect("ok");
        assert_eq!(t.from, HyperChatMode::Chat);
        assert_eq!(t.to, HyperChatMode::Edit);
    }

    #[test]
    fn transition_same_mode_errors() {
        let err = transition(HyperChatMode::Chat, HyperChatMode::Chat).expect_err("same mode");
        assert_eq!(
            err,
            TransitionError::SameMode {
                mode: HyperChatMode::Chat
            }
        );
    }
}
