// SPDX-License-Identifier: MIT
//! Event types and bus statistics for the HyperChat Lite event stream.

use crate::hyperchat_lite::modes::HyperChatMode;
use serde::{Deserialize, Serialize};

/// Events broadcast over the in-process event stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    ChatStreamStarted {
        run_id: String,
    },
    ChatStreamChunk {
        run_id: String,
        content: String,
    },
    ChatStreamEnded {
        run_id: String,
    },
    ModeChanged {
        run_id: String,
        from: HyperChatMode,
        to: HyperChatMode,
    },
    /// Diagnostic test event (stripped in release builds at the call site).
    Test {
        message: String,
    },
}

/// Snapshot of bus health for inspection by the host or tooling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStreamStats {
    pub subscribers: usize,
    pub events_published: u64,
}
