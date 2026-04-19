// SPDX-License-Identifier: MIT
//! Chat backend (Ollama HTTP streaming). MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2.
//!
//! Phase 2: real Ollama HTTP streaming via reqwest + Tauri Channel.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Role of a chat message. Lowercase serde representation matches Ollama API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// One message in a chat thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// One streaming chunk from Ollama. `done=true` signals end-of-response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    pub content: String,
    pub done: bool,
}

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "qwen3:8b";
const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Build the JSON body sent to `/api/chat`.
fn build_chat_body(messages: &[Message], model: &str, stream: bool) -> serde_json::Value {
    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            let role = match m.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
            };
            serde_json::json!({
                "role": role,
                "content": m.content,
            })
        })
        .collect();

    serde_json::json!({
        "model": model,
        "messages": msgs,
        "stream": stream,
    })
}

/// Buffer-mode chat — collects the full response text before returning.
///
/// Used by tests and any non-streaming caller. POSTs to `{base_url}/api/chat`
/// with `stream=false` and returns the assistant's `message.content`.
pub async fn stream_to_buffer(
    messages: &[Message],
    model: &str,
    base_url: &str,
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| AppError::Ollama(format!("client build: {e}")))?;

    let body = build_chat_body(messages, model, false);

    let resp = client
        .post(format!("{base_url}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Ollama(format!("post: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Ollama(format!("status {}", resp.status())));
    }

    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Ollama(format!("decode body: {e}")))?;

    Ok(v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string())
}

/// Tauri command: stream chat tokens to frontend via Channel.
///
/// POSTs `{DEFAULT_OLLAMA_URL}/api/chat` with `stream=true`, parses NDJSON
/// chunks line-by-line, and forwards each chunk to the frontend over the
/// supplied `tauri::ipc::Channel<ChatChunk>`. Stops on the first chunk with
/// `done=true`.
#[tauri::command]
pub async fn chat_stream(
    messages: Vec<Message>,
    model: Option<String>,
    on_chunk: tauri::ipc::Channel<ChatChunk>,
) -> AppResult<()> {
    let model = model.unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| AppError::Ollama(format!("client build: {e}")))?;

    let body = build_chat_body(&messages, &model, true);

    let resp = client
        .post(format!("{DEFAULT_OLLAMA_URL}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Ollama(format!("post: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Ollama(format!("status {}", resp.status())));
    }

    use futures::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| AppError::Ollama(format!("stream: {e}")))?;
        buffer.extend_from_slice(&bytes);

        // Ollama emits one JSON object per line. Drain complete lines from
        // the buffer; leave any partial trailing line for the next chunk.
        while let Some(nl) = buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=nl).collect();
            let trimmed = std::str::from_utf8(&line)
                .map_err(|e| AppError::Ollama(format!("utf8: {e}")))?
                .trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let content = v
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let done = v.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
                on_chunk
                    .send(ChatChunk { content, done })
                    .map_err(|e| AppError::Internal(format!("channel send: {e}")))?;
                if done {
                    return Ok(());
                }
            }
        }
    }

    // Stream ended without a `done=true` chunk — flush any trailing buffered
    // line, then signal completion to the frontend.
    if !buffer.is_empty() {
        let trimmed = std::str::from_utf8(&buffer)
            .map_err(|e| AppError::Ollama(format!("utf8: {e}")))?
            .trim();
        if !trimmed.is_empty() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let content = v
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                on_chunk
                    .send(ChatChunk {
                        content,
                        done: true,
                    })
                    .map_err(|e| AppError::Internal(format!("channel send: {e}")))?;
                return Ok(());
            }
        }
    }

    on_chunk
        .send(ChatChunk {
            content: String::new(),
            done: true,
        })
        .map_err(|e| AppError::Internal(format!("channel send: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_chat_body_includes_stream_flag() {
        let msgs = vec![Message {
            role: MessageRole::User,
            content: "hi".into(),
        }];
        let body = build_chat_body(&msgs, "qwen3:8b", true);
        assert_eq!(body["stream"], serde_json::Value::Bool(true));
        assert_eq!(body["model"], "qwen3:8b");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hi");
    }

    #[test]
    fn build_chat_body_maps_assistant_role() {
        let msgs = vec![Message {
            role: MessageRole::Assistant,
            content: "ok".into(),
        }];
        let body = build_chat_body(&msgs, "m", false);
        assert_eq!(body["messages"][0]["role"], "assistant");
    }
}
