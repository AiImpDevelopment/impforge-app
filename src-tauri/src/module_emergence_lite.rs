// SPDX-License-Identifier: MIT
//! Lightweight module-emergence kernel (`emerge!()` macro + IPC core only).
//! MIT-rewritten per `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real capability registry + request/response router.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Newtype identifier for an ImpForge module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleId(pub String);

/// One capability advertised by a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub schema: serde_json::Value,
}

/// Registration payload describing one module + its capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergenceContext {
    pub module: ModuleId,
    pub capabilities: Vec<Capability>,
}

/// One IPC request from a `from` module to a `to` module for a named capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub from: ModuleId,
    pub to: ModuleId,
    pub capability: String,
    pub payload: serde_json::Value,
}

/// One IPC response with success flag + free-form payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub payload: serde_json::Value,
}

/// Register a module's capabilities with the emergence kernel.
#[tauri::command]
pub async fn emergence_register(_ctx: EmergenceContext) -> AppResult<()> {
    Err(AppError::Internal(
        "module_emergence_lite::emergence_register not implemented in Phase 1".into(),
    ))
}

/// Send an IPC request from one module to another and await its response.
#[tauri::command]
pub async fn emergence_ask(_req: IpcRequest) -> AppResult<IpcResponse> {
    Err(AppError::Internal(
        "module_emergence_lite::emergence_ask not implemented in Phase 1".into(),
    ))
}

/// Check whether the given module advertises the named capability.
#[tauri::command]
pub async fn emergence_has_capability(
    _module: ModuleId,
    _capability: String,
) -> AppResult<bool> {
    Err(AppError::Internal(
        "module_emergence_lite::emergence_has_capability not implemented in Phase 1".into(),
    ))
}
