// SPDX-License-Identifier: MIT
//! Bounded MPSC event bus for HyperChat Lite. Port of the Pro
//! `hyperchat_v2/event_stream.rs` design simplified for MIT.
//!
//! Phase 2: real `EventHub` backed by a `Mutex<Vec<Sender<Event>>>` plus
//! a process-wide `OnceLock<EventHub>` accessor (`hub()`). Disconnected
//! senders are pruned on every publish.

use crate::hyperchat_lite::modes::HyperChatMode;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

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

/// In-process pub/sub bus. Each `subscribe()` returns its own receiver.
pub struct EventHub {
    senders: Mutex<Vec<Sender<Event>>>,
    subscriber_count: AtomicUsize,
    events_published: AtomicU64,
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            senders: Mutex::new(Vec::new()),
            subscriber_count: AtomicUsize::new(0),
            events_published: AtomicU64::new(0),
        }
    }

    /// Subscribe to the event stream. Returns a fresh `EventSubscription`
    /// that owns the receiver half of an `mpsc::channel`.
    pub fn subscribe(&self) -> EventSubscription {
        let (tx, rx) = channel();
        if let Ok(mut g) = self.senders.lock() {
            g.push(tx);
            self.subscriber_count.store(g.len(), Ordering::Relaxed);
        }
        EventSubscription { rx }
    }

    /// Publish an event to every active subscriber. Disconnected senders
    /// are pruned.
    pub fn publish(&self, event: Event) {
        if let Ok(mut g) = self.senders.lock() {
            g.retain(|s| s.send(event.clone()).is_ok());
            self.subscriber_count.store(g.len(), Ordering::Relaxed);
        }
        self.events_published.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot health stats.
    pub fn stats(&self) -> EventStreamStats {
        EventStreamStats {
            subscribers: self.subscriber_count.load(Ordering::Relaxed),
            events_published: self.events_published.load(Ordering::Relaxed),
        }
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Receiver-side handle returned by `EventHub::subscribe`.
pub struct EventSubscription {
    rx: Receiver<Event>,
}

impl EventSubscription {
    /// Non-blocking poll for the next event. Returns `None` if empty.
    pub fn try_recv(&mut self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

/// Process-wide event hub accessor. Lazy-initialized.
static HUB: OnceLock<EventHub> = OnceLock::new();

pub fn hub() -> &'static EventHub {
    HUB.get_or_init(EventHub::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_hub_has_zero_subscribers_zero_events() {
        let h = EventHub::new();
        let s = h.stats();
        assert_eq!(s.subscribers, 0);
        assert_eq!(s.events_published, 0);
    }

    #[test]
    fn publish_increments_event_count() {
        let h = EventHub::new();
        h.publish(Event::Test {
            message: "x".into(),
        });
        h.publish(Event::Test {
            message: "y".into(),
        });
        assert_eq!(h.stats().events_published, 2);
    }

    #[test]
    fn dropped_subscriber_is_pruned_on_next_publish() {
        let h = EventHub::new();
        {
            let _sub = h.subscribe();
            assert_eq!(h.stats().subscribers, 1);
        } // sub drops here
        h.publish(Event::Test {
            message: "trigger prune".into(),
        });
        assert_eq!(h.stats().subscribers, 0);
    }
}
