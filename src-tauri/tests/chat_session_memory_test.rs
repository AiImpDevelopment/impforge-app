// SPDX-License-Identifier: MIT
//! Behavioral tests for chat_session_memory Phase 1 type skeleton.

use impforge_app_lib::chat_session_memory::{ThreadId, ThreadSnapshot};
use impforge_app_lib::chat_lite::Message;

#[test]
fn thread_id_roundtrips_through_json() {
    let id = ThreadId("550e8400-e29b-41d4-a716-446655440000".into());
    let j = serde_json::to_string(&id).expect("serialize ThreadId");
    let back: ThreadId = serde_json::from_str(&j).expect("deserialize ThreadId");
    assert_eq!(id.0, back.0);
}

#[test]
fn thread_snapshot_roundtrips_through_json() {
    let original = ThreadSnapshot {
        id: ThreadId("test-thread".into()),
        title: "Test Thread".into(),
        created_at: chrono::Utc::now(),
        message_count: 5,
    };
    let j = serde_json::to_string(&original).expect("serialize ThreadSnapshot");
    let back: ThreadSnapshot = serde_json::from_str(&j).expect("deserialize ThreadSnapshot");
    assert_eq!(original.id.0, back.id.0);
    assert_eq!(original.title, back.title);
    assert_eq!(original.message_count, back.message_count);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<ThreadId>();
    assert_traits::<ThreadSnapshot>();
    assert_traits::<Message>();
}

// TODO Phase 2: integration tests for the four Tauri commands once
// the in-memory `VecDeque<Thread>` (cap 20) is wired in.
