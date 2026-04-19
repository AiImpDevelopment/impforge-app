// SPDX-License-Identifier: MIT
//! Local-Ollama-only tool-calling cortex. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real tool-call dispatch + event publication backed by tokio channels.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// One outgoing tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Result of one tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call: ToolCall,
    pub output: serde_json::Value,
    pub success: bool,
}

/// Events published to the cortex event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CortexEvent {
    ChatStarted { thread_id: String },
    ChatEnded { thread_id: String },
    ToolInvoked { call: ToolCall },
    ToolCompleted { result: ToolResult },
}

/// Invoke the named tool with the given arguments.
#[tauri::command]
pub async fn cortex_invoke_tool(_call: ToolCall) -> AppResult<ToolResult> {
    Err(AppError::Internal(
        "cortex_lite::cortex_invoke_tool not implemented in Phase 1".into(),
    ))
}

/// Publish a cortex event to all subscribers.
#[tauri::command]
pub async fn cortex_publish_event(_event: CortexEvent) -> AppResult<()> {
    Err(AppError::Internal(
        "cortex_lite::cortex_publish_event not implemented in Phase 1".into(),
    ))
}
