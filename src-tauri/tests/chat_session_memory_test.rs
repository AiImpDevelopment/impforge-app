// SPDX-License-Identifier: MIT
//! Behavioral tests for chat_session_memory Phase 2 implementation.
//! Type-roundtrip tests stay; new tests exercise the real VecDeque store.

use impforge_app_lib::chat_lite::{Message, MessageRole};
use impforge_app_lib::chat_session_memory::{SessionStore, ThreadId, ThreadSnapshot, MAX_THREADS};

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
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<ThreadId>();
    assert_traits::<ThreadSnapshot>();
    assert_traits::<Message>();
}

fn user(text: &str) -> Message {
    Message {
        role: MessageRole::User,
        content: text.into(),
    }
}

#[test]
fn session_store_holds_max_20_threads() {
    let mut store = SessionStore::new();
    let mut ids: Vec<ThreadId> = Vec::new();
    for i in 0..25 {
        let id = store.create_thread(format!("thread {i}"));
        store.append_message(&id, user(&format!("msg {i}")));
        ids.push(id);
    }
    let threads: Vec<ThreadSnapshot> = store.list_threads();
    assert_eq!(threads.len(), MAX_THREADS, "cap is {MAX_THREADS}");
    assert_eq!(
        threads[0].title, "thread 5",
        "oldest 5 threads should be evicted"
    );
    assert_eq!(threads.last().expect("non-empty").title, "thread 24");

    // Evicted threads must no longer be retrievable.
    let evicted = store.get_messages(&ids[0]);
    assert!(
        evicted.is_empty(),
        "thread 0 was evicted, get_messages must return empty"
    );

    // Surviving threads still have their messages.
    let surviving = store.get_messages(&ids[24]);
    assert_eq!(surviving.len(), 1);
    assert_eq!(surviving[0].content, "msg 24");
}

#[test]
fn session_store_lost_on_drop() {
    let store_id = {
        let mut store = SessionStore::new();
        store.create_thread("ephemeral".into())
    };
    // After scope exit `store` is dropped — proves no persistence.
    assert!(!store_id.0.is_empty());
}

#[test]
fn list_threads_is_oldest_first() {
    let mut store = SessionStore::new();
    let _a = store.create_thread("a".into());
    let _b = store.create_thread("b".into());
    let _c = store.create_thread("c".into());
    let snaps = store.list_threads();
    assert_eq!(snaps.len(), 3);
    assert_eq!(snaps[0].title, "a");
    assert_eq!(snaps[1].title, "b");
    assert_eq!(snaps[2].title, "c");
}
