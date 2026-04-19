// SPDX-License-Identifier: MIT
//! Memory-only chat session storage. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §6.2.
//!
//! Phase 2: real `VecDeque<Thread>` capped at MAX_THREADS, guarded by a global
//! `Mutex<Option<SessionStore>>`. No persistence — Pro adds SQLite. Lost on
//! app restart by design.

use crate::chat_lite::Message;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use uuid::Uuid;

/// Maximum number of threads the in-memory store will hold. Oldest evicted.
pub const MAX_THREADS: usize = 20;

/// Newtype wrapping a UUID v4 string identifying a chat thread.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadId(pub String);

impl ThreadId {
    /// Generate a fresh random thread id.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Borrow the inner UUID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight snapshot of a chat thread (without full message body) for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSnapshot {
    pub id: ThreadId,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
}

#[derive(Debug, Clone)]
struct Thread {
    id: ThreadId,
    title: String,
    created_at: chrono::DateTime<chrono::Utc>,
    messages: Vec<Message>,
}

/// Memory-only thread store, capped at MAX_THREADS. FIFO eviction.
pub struct SessionStore {
    threads: VecDeque<Thread>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            threads: VecDeque::with_capacity(MAX_THREADS),
        }
    }

    /// Create a new thread and return its identifier. Evicts the oldest thread
    /// if the store is at capacity.
    pub fn create_thread(&mut self, title: String) -> ThreadId {
        let id = ThreadId::new();
        let thread = Thread {
            id: id.clone(),
            title,
            created_at: chrono::Utc::now(),
            messages: Vec::new(),
        };
        if self.threads.len() == MAX_THREADS {
            self.threads.pop_front();
        }
        self.threads.push_back(thread);
        id
    }

    /// Append a message to the thread identified by `id`. No-op if not found.
    pub fn append_message(&mut self, id: &ThreadId, msg: Message) {
        if let Some(t) = self.threads.iter_mut().find(|t| &t.id == id) {
            t.messages.push(msg);
        }
    }

    /// List all threads in oldest-first order (the order they were created in).
    pub fn list_threads(&self) -> Vec<ThreadSnapshot> {
        self.threads
            .iter()
            .map(|t| ThreadSnapshot {
                id: t.id.clone(),
                title: t.title.clone(),
                created_at: t.created_at,
                message_count: t.messages.len(),
            })
            .collect()
    }

    /// Return the full message history for the given thread, or empty if missing.
    pub fn get_messages(&self, id: &ThreadId) -> Vec<Message> {
        self.threads
            .iter()
            .find(|t| &t.id == id)
            .map(|t| t.messages.clone())
            .unwrap_or_default()
    }

    /// Number of threads currently held in memory.
    pub fn len(&self) -> usize {
        self.threads.len()
    }

    /// Whether the store has zero threads.
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

// Global Tauri-state-managed store. Lazy-initialized on first call.
static STORE: Mutex<Option<SessionStore>> = Mutex::new(None);

fn with_store<R>(f: impl FnOnce(&mut SessionStore) -> R) -> AppResult<R> {
    let mut guard = STORE
        .lock()
        .map_err(|e| AppError::Internal(format!("session store mutex poisoned: {e}")))?;
    Ok(f(guard.get_or_insert_with(SessionStore::new)))
}

/// Tauri: create a new thread with the given title and return its identifier.
#[tauri::command]
pub async fn session_create_thread(title: String) -> AppResult<ThreadId> {
    with_store(|s| s.create_thread(title))
}

/// Tauri: append a message to the thread identified by `id`.
#[tauri::command]
pub async fn session_append_message(id: ThreadId, message: Message) -> AppResult<()> {
    with_store(|s| s.append_message(&id, message))
}

/// Tauri: list all threads currently held in memory (oldest first).
#[tauri::command]
pub async fn session_list_threads() -> AppResult<Vec<ThreadSnapshot>> {
    with_store(|s| s.list_threads())
}

/// Tauri: return the full message history for the given thread.
#[tauri::command]
pub async fn session_get_messages(id: ThreadId) -> AppResult<Vec<Message>> {
    with_store(|s| s.get_messages(&id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(text: &str) -> Message {
        Message {
            role: crate::chat_lite::MessageRole::User,
            content: text.into(),
        }
    }

    #[test]
    fn empty_store_lists_zero_threads() {
        let s = SessionStore::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.list_threads().is_empty());
    }

    #[test]
    fn create_then_append_then_get() {
        let mut s = SessionStore::new();
        let id = s.create_thread("t1".into());
        s.append_message(&id, user("hi"));
        s.append_message(&id, user("there"));
        let msgs = s.get_messages(&id);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "hi");
        assert_eq!(msgs[1].content, "there");
    }

    #[test]
    fn append_to_unknown_thread_is_noop() {
        let mut s = SessionStore::new();
        s.append_message(&ThreadId("does-not-exist".into()), user("x"));
        assert!(s.is_empty());
    }

    #[test]
    fn get_unknown_thread_returns_empty() {
        let s = SessionStore::new();
        let msgs = s.get_messages(&ThreadId("missing".into()));
        assert!(msgs.is_empty());
    }
}
