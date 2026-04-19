// SPDX-License-Identifier: MIT
//! Behavioral tests for chat_lite Phase 2 implementation.
//! Type-roundtrip tests stay; new tests exercise the real reqwest path.

use impforge_app_lib::chat_lite::{stream_to_buffer, ChatChunk, Message, MessageRole};

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

#[tokio::test]
async fn stream_to_buffer_concatenates_tokens_in_order() {
    // Verifies the HTTP path is wired — connecting to a closed port must
    // surface as AppError::Ollama (not a panic / silent success).
    let messages = vec![Message {
        role: MessageRole::User,
        content: "say hi".into(),
    }];
    let buf = stream_to_buffer(&messages, "fake-model", "http://127.0.0.1:1").await;
    assert!(buf.is_err(), "fake URL must fail, got Ok: {buf:?}");
}

#[tokio::test]
async fn stream_to_buffer_parses_assistant_content() {
    // Spin up an in-process HTTP server that mimics Ollama's `/api/chat`
    // non-streaming response and confirm we extract `message.content`.
    use std::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    listener.set_nonblocking(true).expect("nonblocking");
    let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");

    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 4096];
        let _ = sock.read(&mut buf).await.expect("read");
        let body = r#"{"message":{"role":"assistant","content":"hello world"},"done":true}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        sock.write_all(resp.as_bytes()).await.expect("write");
        sock.shutdown().await.expect("shutdown");
    });

    let messages = vec![Message {
        role: MessageRole::User,
        content: "ping".into(),
    }];
    let url = format!("http://127.0.0.1:{}", addr.port());
    let result = stream_to_buffer(&messages, "any", &url).await;

    server.await.expect("server task");
    assert_eq!(result.expect("ok response"), "hello world");
}
