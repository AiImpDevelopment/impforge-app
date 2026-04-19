// SPDX-License-Identifier: MIT
//! Slash-command catalog + dispatcher. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED #3.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: command registry backed by static catalog + dispatcher routing.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// One registered slash command (e.g. `/help`, `/clear`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub category: String,
}

/// Result of dispatching a slash command.
///
/// Internally-tagged enum (`tag = "kind"`); newtype variants must be expressed as
/// struct variants because serde-json cannot embed a primitive into the tagged
/// object form.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlashOutcome {
    /// Command produced direct text output (e.g. help).
    Text { text: String },
    /// Command resolved to an intent for the host to handle.
    Intent { name: String, arg: Option<String> },
    /// Input did not match a known slash command.
    Unknown { input: String },
}

/// Return the catalog of all registered slash commands.
#[tauri::command]
pub async fn slash_catalog() -> AppResult<Vec<SlashCommand>> {
    Err(AppError::Internal(
        "slash_commands::slash_catalog not implemented in Phase 1".into(),
    ))
}

/// Dispatch a slash-command input string in the context of an optional thread.
#[tauri::command]
pub async fn slash_dispatch(
    _input: String,
    _thread_id: Option<String>,
) -> AppResult<SlashOutcome> {
    Err(AppError::Internal(
        "slash_commands::slash_dispatch not implemented in Phase 1".into(),
    ))
}
