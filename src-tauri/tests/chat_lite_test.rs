// SPDX-License-Identifier: MIT
//! Behavioral tests for chat_lite Phase 1 type skeleton.
//! Tests serde roundtrip, enum-format, and Phase 1 NotImplemented contract.

use impforge_app_lib::chat_lite::{ChatChunk, Message, MessageRole};

#[test]
fn message_role_serializes_lowercase() {
    let m = Message {
        role: MessageRole::User,
        content: "hello".into(),
    };
    let j = serde_json::to_string(&m).expect("serialize Message");
    assert!(j.contains("\"role\":\"user\""), "role lowercase, got: {j}");
}

#[test]
fn message_roundtrips_through_json() {
    let original = Message {
        role: MessageRole::Assistant,
        content: "I am ready.".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize Message");
    let back: Message = serde_json::from_str(&j).expect("deserialize Message");
    assert_eq!(original.role, back.role);
    assert_eq!(original.content, back.content);
}

#[test]
fn message_role_all_variants_serialize() {
    for (role, expected) in [
        (MessageRole::User, "user"),
        (MessageRole::Assistant, "assistant"),
        (MessageRole::System, "system"),
    ] {
        let j = serde_json::to_string(&role).expect("serialize MessageRole");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn chat_chunk_default_done_false() {
    let c = ChatChunk {
        content: "x".into(),
        done: false,
    };
    let j = serde_json::to_string(&c).expect("serialize ChatChunk");
    assert!(j.contains("\"done\":false"));
}

#[test]
fn message_implements_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<Message>();
    assert_traits::<MessageRole>();
    assert_traits::<ChatChunk>();
}
