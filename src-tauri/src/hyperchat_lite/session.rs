// SPDX-License-Identifier: MIT
//! Per-session conversational state for HyperChat Lite.
//!
//! Phase 2: real `HyperChatSession` with bounded recent-events ring buffer.
//! NO JSONL, NO crash recovery — that's Pro.

use crate::chat_lite::Message;
use crate::hyperchat_lite::event_stream::Event;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Maximum number of recent events kept resident in a session.
pub const MAX_RESIDENT_EVENTS: usize = 256;

/// One in-flight HyperChat session bound to a single conversation thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperChatSession {
    pub id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub messages: Vec<Message>,
    pub recent_events: VecDeque<Event>,
}

impl HyperChatSession {
    /// Create a new session with a fresh UUID and empty buffers.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            started_at: chrono::Utc::now(),
            messages: Vec::new(),
            recent_events: VecDeque::new(),
        }
    }

    /// Append a chat message. Unbounded — message-history is small relative
    /// to event volume.
    pub fn append_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    /// Append an event. Drops the oldest event once `MAX_RESIDENT_EVENTS`
    /// is exceeded.
    pub fn append_event(&mut self, event: Event) {
        self.recent_events.push_back(event);
        while self.recent_events.len() > MAX_RESIDENT_EVENTS {
            self.recent_events.pop_front();
        }
    }
}

impl Default for HyperChatSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_lite::MessageRole;

    #[test]
    fn new_session_has_empty_buffers_and_uuid_id() {
        let s = HyperChatSession::new();
        assert!(!s.id.is_empty());
        assert!(s.messages.is_empty());
        assert!(s.recent_events.is_empty());
    }

    #[test]
    fn append_event_caps_at_max_resident() {
        let mut s = HyperChatSession::new();
        for i in 0..(MAX_RESIDENT_EVENTS + 10) {
            s.append_event(Event::Test {
                message: format!("e{i}"),
            });
        }
        assert_eq!(s.recent_events.len(), MAX_RESIDENT_EVENTS);
        // Oldest 10 evicted; first remaining must be e10.
        match s.recent_events.front().expect("non-empty") {
            Event::Test { message } => assert_eq!(message, "e10"),
            other => panic!("expected Test event, got {other:?}"),
        }
    }

    #[test]
    fn append_message_grows_unbounded() {
        let mut s = HyperChatSession::new();
        for i in 0..1000 {
            s.append_message(Message {
                role: MessageRole::User,
                content: format!("m{i}"),
            });
        }
        assert_eq!(s.messages.len(), 1000);
    }
}
