// SPDX-License-Identifier: MIT
//! Universal tool consumer surface (4 protocols, no security gateway).
//! MIT-rewritten per `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real protocol adapters (MCP/OpenAI/ReAct/GBNF) + tool registry.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Supported tool-consumer protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerProtocol {
    Mcp,
    Openai,
    ReAct,
    Gbnf,
}

/// Schema describing one available tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// One outgoing tool invocation against the universal surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub protocol: ConsumerProtocol,
    pub tool: String,
    pub arguments: serde_json::Value,
}

/// Result returned by an invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub result: serde_json::Value,
}

/// Register a tool schema with the universal surface.
#[tauri::command]
pub async fn universal_register_tool(_schema: ToolSchema) -> AppResult<()> {
    Err(AppError::Internal(
        "universal_lite::universal_register_tool not implemented in Phase 1".into(),
    ))
}

/// List all currently-registered tools.
#[tauri::command]
pub async fn universal_list_tools() -> AppResult<Vec<ToolSchema>> {
    Err(AppError::Internal(
        "universal_lite::universal_list_tools not implemented in Phase 1".into(),
    ))
}

/// Invoke a registered tool through the chosen consumer protocol.
#[tauri::command]
pub async fn universal_invoke(_call: ToolInvocation) -> AppResult<ToolOutput> {
    Err(AppError::Internal(
        "universal_lite::universal_invoke not implemented in Phase 1".into(),
    ))
}
