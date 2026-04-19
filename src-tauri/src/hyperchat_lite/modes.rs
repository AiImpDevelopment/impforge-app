// SPDX-License-Identifier: MIT
//! Mode-state machine for HyperChat Lite (Chat / Edit / Agent).

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

/// One transition from a previous mode to a new one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeTransition {
    pub from: HyperChatMode,
    pub to: HyperChatMode,
}

/// Errors that can occur when attempting to switch modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionError {
    /// Caller attempted to transition into the mode that is already active.
    SameMode { mode: HyperChatMode },
}
