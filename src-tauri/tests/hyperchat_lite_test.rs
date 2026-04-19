// SPDX-License-Identifier: MIT
//! Behavioral tests for hyperchat_lite Phase 2 implementation.
//! Type-roundtrip tests stay; new tests exercise the real state machine,
//! event hub, and session ring buffer.

use impforge_app_lib::hyperchat_lite::event_stream::{hub, Event, EventHub, EventStreamStats};
use impforge_app_lib::hyperchat_lite::modes::{
    transition, HyperChatMode, ModeState, ModeTransition, TransitionError,
};
use impforge_app_lib::hyperchat_lite::session::{HyperChatSession, MAX_RESIDENT_EVENTS};

#[test]
fn hyperchat_mode_serializes_lowercase() {
    for (m, expected) in [
        (HyperChatMode::Chat, "chat"),
        (HyperChatMode::Edit, "edit"),
        (HyperChatMode::Agent, "agent"),
    ] {
        let j = serde_json::to_string(&m).expect("serialize HyperChatMode");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn mode_state_roundtrips_through_json() {
    let original = ModeState {
        current: HyperChatMode::Chat,
        history: vec![HyperChatMode::Edit, HyperChatMode::Agent],
    };
    let j = serde_json::to_string(&original).expect("serialize ModeState");
    let back: ModeState = serde_json::from_str(&j).expect("deserialize ModeState");
    assert_eq!(original.current, back.current);
    assert_eq!(original.history, back.history);
}

#[test]
fn mode_transition_roundtrips() {
    let original = ModeTransition {
        from: HyperChatMode::Chat,
        to: HyperChatMode::Agent,
    };
    let j = serde_json::to_string(&original).expect("serialize ModeTransition");
    let back: ModeTransition = serde_json::from_str(&j).expect("deserialize ModeTransition");
    assert_eq!(original.from, back.from);
    assert_eq!(original.to, back.to);
}

#[test]
fn transition_error_serializes_with_kind_tag() {
    let e = TransitionError::SameMode {
        mode: HyperChatMode::Edit,
    };
    let j = serde_json::to_string(&e).expect("serialize TransitionError");
    assert!(j.contains("\"kind\":\"same_mode\""), "got: {j}");
}

#[test]
fn event_chat_stream_started_serializes_with_kind_tag() {
    let e = Event::ChatStreamStarted {
        run_id: "abc".into(),
    };
    let j = serde_json::to_string(&e).expect("serialize Event");
    assert!(j.contains("\"kind\":\"chat_stream_started\""), "got: {j}");
    assert!(j.contains("\"run_id\":\"abc\""), "got: {j}");
}

#[test]
fn event_test_serializes_with_kind_tag() {
    let e = Event::Test {
        message: "hello".into(),
    };
    let j = serde_json::to_string(&e).expect("serialize Event::Test");
    assert!(j.contains("\"kind\":\"test\""), "got: {j}");
}

#[test]
fn event_stream_stats_roundtrips() {
    let original = EventStreamStats {
        subscribers: 3,
        events_published: 42,
    };
    let j = serde_json::to_string(&original).expect("serialize EventStreamStats");
    let back: EventStreamStats = serde_json::from_str(&j).expect("deserialize EventStreamStats");
    assert_eq!(original.subscribers, back.subscribers);
    assert_eq!(original.events_published, back.events_published);
}

#[test]
fn hyperchat_session_roundtrips() {
    let original = HyperChatSession {
        id: "session-1".into(),
        started_at: chrono::Utc::now(),
        messages: vec![],
        recent_events: std::collections::VecDeque::new(),
    };
    let j = serde_json::to_string(&original).expect("serialize HyperChatSession");
    let back: HyperChatSession = serde_json::from_str(&j).expect("deserialize HyperChatSession");
    assert_eq!(original.id, back.id);
    assert_eq!(original.messages.len(), back.messages.len());
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<HyperChatMode>();
    assert_traits::<ModeState>();
    assert_traits::<ModeTransition>();
    assert_traits::<TransitionError>();
    assert_traits::<Event>();
    assert_traits::<EventStreamStats>();
    assert_traits::<HyperChatSession>();
}

// --- Phase 2 behavioral tests ---

#[test]
fn modes_chat_to_edit_allowed() {
    let result = transition(HyperChatMode::Chat, HyperChatMode::Edit);
    assert!(result.is_ok());
}

#[test]
fn modes_chat_to_agent_allowed() {
    let result = transition(HyperChatMode::Chat, HyperChatMode::Agent);
    assert!(result.is_ok());
}

#[test]
fn modes_same_mode_is_noop() {
    let err = transition(HyperChatMode::Chat, HyperChatMode::Chat).expect_err("same mode errors");
    assert_eq!(
        err,
        TransitionError::SameMode {
            mode: HyperChatMode::Chat
        }
    );
}

#[test]
fn modes_all_pairs_in_either_direction() {
    for (a, b) in [
        (HyperChatMode::Chat, HyperChatMode::Edit),
        (HyperChatMode::Chat, HyperChatMode::Agent),
        (HyperChatMode::Edit, HyperChatMode::Chat),
        (HyperChatMode::Edit, HyperChatMode::Agent),
        (HyperChatMode::Agent, HyperChatMode::Chat),
        (HyperChatMode::Agent, HyperChatMode::Edit),
    ] {
        let t = transition(a, b).expect("ok");
        assert_eq!(t.from, a);
        assert_eq!(t.to, b);
    }
}

#[test]
fn event_hub_receives_published_events() {
    let h = EventHub::new();
    let mut sub = h.subscribe();
    h.publish(Event::Test {
        message: "hello".into(),
    });
    let recv = sub.try_recv();
    match recv {
        Some(Event::Test { message }) => assert_eq!(message, "hello"),
        other => panic!("expected Some(Test), got {other:?}"),
    }
}

#[test]
fn event_hub_publishes_to_multiple_subscribers() {
    let h = EventHub::new();
    let mut a = h.subscribe();
    let mut b = h.subscribe();
    h.publish(Event::ChatStreamEnded {
        run_id: "r1".into(),
    });
    assert!(matches!(a.try_recv(), Some(Event::ChatStreamEnded { .. })));
    assert!(matches!(b.try_recv(), Some(Event::ChatStreamEnded { .. })));
}

#[test]
fn global_hub_is_a_singleton() {
    // Calling `hub()` twice must return the same address.
    let a = hub() as *const EventHub;
    let b = hub() as *const EventHub;
    assert_eq!(a, b, "hub() must return the same singleton");
}

#[test]
fn session_event_buffer_is_capped_at_max_resident() {
    let mut s = HyperChatSession::new();
    for i in 0..(MAX_RESIDENT_EVENTS + 5) {
        s.append_event(Event::Test {
            message: format!("e{i}"),
        });
    }
    assert_eq!(s.recent_events.len(), MAX_RESIDENT_EVENTS);
}
