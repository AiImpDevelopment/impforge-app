// SPDX-License-Identifier: MIT
//! Memory-only chat session storage. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §6.2.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real `VecDeque<Thread>` cap 20 backed by `Mutex<VecDeque<Thread>>`.

use crate::chat_lite::Message;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Newtype wrapping a UUID v4 string identifying a chat thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadId(pub String);

/// Lightweight snapshot of a chat thread (without full message body) for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSnapshot {
    pub id: ThreadId,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
}

/// Create a new thread with the given title and return its identifier.
#[tauri::command]
pub async fn session_create_thread(_title: String) -> AppResult<ThreadId> {
    Err(AppError::Internal(
        "chat_session_memory::session_create_thread not implemented in Phase 1".into(),
    ))
}

/// Append a message to the thread identified by `id`.
#[tauri::command]
pub async fn session_append_message(_id: ThreadId, _message: Message) -> AppResult<()> {
    Err(AppError::Internal(
        "chat_session_memory::session_append_message not implemented in Phase 1".into(),
    ))
}

/// List all threads currently held in memory (most-recent first).
#[tauri::command]
pub async fn session_list_threads() -> AppResult<Vec<ThreadSnapshot>> {
    Err(AppError::Internal(
        "chat_session_memory::session_list_threads not implemented in Phase 1".into(),
    ))
}

/// Return the full message history for the given thread.
#[tauri::command]
pub async fn session_get_messages(_id: ThreadId) -> AppResult<Vec<Message>> {
    Err(AppError::Internal(
        "chat_session_memory::session_get_messages not implemented in Phase 1".into(),
    ))
}
