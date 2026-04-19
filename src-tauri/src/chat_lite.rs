// SPDX-License-Identifier: MIT
//! Chat backend (Ollama HTTP streaming). MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real Ollama HTTP streaming via reqwest + Tauri Channel.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Role of a chat message. Lowercase serde representation matches Ollama API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// One message in a chat thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// One streaming chunk from Ollama. `done=true` signals end-of-response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    pub content: String,
    pub done: bool,
}

/// Stream chat tokens to frontend via Tauri Channel.
///
/// Phase 1: returns `Err(AppError::Internal(...))`.
/// Phase 2: wires `reqwest` to `localhost:11434/api/chat` and forwards chunks.
#[tauri::command]
pub async fn chat_stream(
    _messages: Vec<Message>,
    _model: Option<String>,
    _on_chunk: tauri::ipc::Channel<ChatChunk>,
) -> AppResult<()> {
    Err(AppError::Internal(
        "chat_lite::chat_stream not implemented in Phase 1".into(),
    ))
}
