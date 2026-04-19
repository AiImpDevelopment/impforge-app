// SPDX-License-Identifier: MIT
//! Per-session conversational state for HyperChat Lite.

use crate::chat_lite::Message;
use crate::hyperchat_lite::event_stream::Event;
use serde::{Deserialize, Serialize};

/// One in-flight HyperChat session bound to a single conversation thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperChatSession {
    pub id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub messages: Vec<Message>,
    pub recent_events: std::collections::VecDeque<Event>,
}
