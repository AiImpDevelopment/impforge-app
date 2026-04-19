// SPDX-License-Identifier: MIT
//! Multi-provider BYOK chat backend (Feature 1, Tier 2/3).
//!
//! Five v1 providers:
//!   - **Ollama**     (local, no key)
//!   - **OpenAI**     (BYOK)
//!   - **Anthropic**  (BYOK)
//!   - **Gemini**     (BYOK)
//!   - **OpenRouter** (BYOK, OpenAI-compatible via custom base URL)
//!
//! Frontend interaction:
//!   - `provider_list()` -> `Vec<ProviderInfo>` for the Settings UI.
//!   - `provider_add(name, key)` / `provider_remove(name)` -> keychain CRUD.
//!   - `provider_chat_stream(provider, model, messages, on_chunk)` -> drives
//!     a `tauri::ipc::Channel<ChatChunk>` exactly like `chat_lite::chat_stream`
//!     so the Svelte side keeps a single streaming codepath.
//!
//! See research report `2026-04-19-multi-provider-byok.md` §A + §B Tier-2.

use crate::chat_lite::{ChatChunk, Message, MessageRole};
use crate::error::{AppError, AppResult};
use crate::keys::{self, KeyStatus};
use crate::spend;
use futures::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatRequest, ChatStreamEvent, StreamChunk};
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// The five canonical providers. Mirrors `impforge-cli`'s `ProviderId` but
/// does NOT depend on it — App and CLI evolve independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Ollama,
    OpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
}

impl Provider {
    /// Stable lower-case slug — used in keychain entries and Tauri payloads.
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::OpenAi => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Gemini => "gemini",
            Provider::OpenRouter => "openrouter",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ollama" => Some(Provider::Ollama),
            "openai" => Some(Provider::OpenAi),
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "gemini" | "google" => Some(Provider::Gemini),
            "openrouter" | "router" => Some(Provider::OpenRouter),
            _ => None,
        }
    }

    pub fn all() -> &'static [Provider] {
        &[
            Provider::Ollama,
            Provider::OpenAi,
            Provider::Anthropic,
            Provider::Gemini,
            Provider::OpenRouter,
        ]
    }

    pub fn requires_key(&self) -> bool {
        !matches!(self, Provider::Ollama)
    }

    /// Cheap+capable default per research §5/§6.
    pub fn default_model(&self) -> &'static str {
        match self {
            Provider::Ollama => "qwen3:8b",
            Provider::OpenAi => "gpt-4o-mini",
            Provider::Anthropic => "claude-3-haiku-20240307",
            Provider::Gemini => "gemini-2.0-flash",
            Provider::OpenRouter => "openai/gpt-4o-mini",
        }
    }

    /// Display name shown in the Settings UI header for each card.
    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Ollama => "Ollama (local)",
            Provider::OpenAi => "OpenAI",
            Provider::Anthropic => "Anthropic",
            Provider::Gemini => "Google Gemini",
            Provider::OpenRouter => "OpenRouter",
        }
    }
}

/// Safe-for-frontend snapshot of one provider — never carries a key value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: Provider,
    pub display_name: String,
    pub default_model: String,
    pub requires_key: bool,
    pub key_status: KeyStatus,
}

fn parse_or_err(name: &str) -> AppResult<Provider> {
    Provider::parse(name).ok_or_else(|| {
        AppError::InvalidArgument(format!(
            "unknown provider '{name}' (try ollama / openai / anthropic / gemini / openrouter)",
        ))
    })
}

fn key_status_for(p: Provider) -> AppResult<KeyStatus> {
    if !p.requires_key() {
        return Ok(KeyStatus::Local);
    }
    Ok(if keys::has_key(p.as_str())? {
        KeyStatus::Set
    } else {
        KeyStatus::Missing
    })
}

/// List every provider with display metadata + key status — never the value.
#[tauri::command]
pub fn provider_list() -> AppResult<Vec<ProviderInfo>> {
    let mut out = Vec::with_capacity(Provider::all().len());
    for &p in Provider::all() {
        out.push(ProviderInfo {
            id: p,
            display_name: p.display_name().to_string(),
            default_model: p.default_model().to_string(),
            requires_key: p.requires_key(),
            key_status: key_status_for(p)?,
        });
    }
    Ok(out)
}

/// Save a key. Refuses if `provider` is local-only or `key` is empty.
#[tauri::command]
pub fn provider_add(provider: String, key: String) -> AppResult<()> {
    let p = parse_or_err(&provider)?;
    if !p.requires_key() {
        return Err(AppError::InvalidArgument(format!(
            "{} is local-only and needs no key",
            p.as_str()
        )));
    }
    keys::save_key(p.as_str(), &key)
}

/// Delete a stored key. Idempotent.
#[tauri::command]
pub fn provider_remove(provider: String) -> AppResult<()> {
    let p = parse_or_err(&provider)?;
    if !p.requires_key() {
        return Ok(());
    }
    keys::delete_key(p.as_str())
}

/// Build a `genai::Client` configured for the chosen provider — same
/// pattern as `impforge-cli` (Anthropic/Gemini/OpenAI use AuthResolver,
/// OpenRouter uses ServiceTargetResolver to swap base URL).
fn build_client(provider: Provider, key: Option<String>) -> Client {
    match provider {
        Provider::Ollama => Client::default(),
        Provider::OpenAi | Provider::Anthropic | Provider::Gemini => {
            let key = key.unwrap_or_default();
            let auth = AuthResolver::from_resolver_fn(
                move |_: ModelIden| -> Result<Option<AuthData>, genai::resolver::Error> {
                    Ok(Some(AuthData::from_single(key.clone())))
                },
            );
            Client::builder().with_auth_resolver(auth).build()
        }
        Provider::OpenRouter => {
            let key = key.unwrap_or_default();
            let resolver = ServiceTargetResolver::from_resolver_fn(
                move |st: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
                    let endpoint = Endpoint::from_static("https://openrouter.ai/api/v1/");
                    let auth = AuthData::from_single(key.clone());
                    let model = ModelIden::new(AdapterKind::OpenAI, st.model.model_name);
                    Ok(ServiceTarget {
                        endpoint,
                        auth,
                        model,
                    })
                },
            );
            Client::builder()
                .with_service_target_resolver(resolver)
                .build()
        }
    }
}

fn convert_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|m| match m.role {
            MessageRole::User => ChatMessage::user(m.content.clone()),
            MessageRole::Assistant => ChatMessage::assistant(m.content.clone()),
            MessageRole::System => ChatMessage::system(m.content.clone()),
        })
        .collect()
}

/// Stream a chat response from the chosen provider, mirroring the Channel
/// contract used by `chat_lite::chat_stream`. The frontend can swap the
/// provider without changing its rendering code.
#[tauri::command]
pub async fn provider_chat_stream(
    provider: String,
    model: Option<String>,
    messages: Vec<Message>,
    on_chunk: tauri::ipc::Channel<ChatChunk>,
) -> AppResult<()> {
    let p = parse_or_err(&provider)?;
    let model = model.unwrap_or_else(|| p.default_model().to_string());

    let key = if p.requires_key() {
        match keys::get_key(p.as_str())? {
            Some(k) => Some(k.into_inner()),
            None => {
                return Err(AppError::InvalidArgument(format!(
                    "no key stored for {} — open Settings → Providers to add one",
                    p.as_str()
                )));
            }
        }
    } else {
        None
    };

    let client = build_client(p, key);
    let req = ChatRequest::new(convert_messages(&messages));

    let started = Instant::now();
    let stream_resp = client
        .exec_chat_stream(&model, req, None)
        .await
        .map_err(|e| AppError::Ollama(format!("genai stream: {e}")))?;

    let mut stream = stream_resp.stream;
    let mut byte_total: u64 = 0;
    while let Some(event) = stream.next().await {
        let event = event.map_err(|e| AppError::Ollama(format!("genai chunk: {e}")))?;
        match event {
            ChatStreamEvent::Chunk(StreamChunk { content }) => {
                byte_total = byte_total.saturating_add(content.len() as u64);
                on_chunk
                    .send(ChatChunk {
                        content,
                        done: false,
                    })
                    .map_err(|e| AppError::Internal(format!("channel send: {e}")))?;
            }
            ChatStreamEvent::ReasoningChunk(_)
            | ChatStreamEvent::ThoughtSignatureChunk(_)
            | ChatStreamEvent::ToolCallChunk(_)
            | ChatStreamEvent::Start
            | ChatStreamEvent::End(_) => {
                // Reasoning + tool-call streaming is out of scope for v1.
            }
        }
    }

    on_chunk
        .send(ChatChunk {
            content: String::new(),
            done: true,
        })
        .map_err(|e| AppError::Internal(format!("channel send: {e}")))?;

    spend::record_call(
        p.as_str(),
        &model,
        byte_total,
        started.elapsed().as_millis() as u64,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parse_canonical_round_trip() {
        for &p in Provider::all() {
            assert_eq!(Provider::parse(p.as_str()), Some(p));
        }
    }

    #[test]
    fn provider_parse_aliases_and_case() {
        assert_eq!(Provider::parse("Claude"), Some(Provider::Anthropic));
        assert_eq!(Provider::parse("GOOGLE"), Some(Provider::Gemini));
        assert_eq!(Provider::parse("router"), Some(Provider::OpenRouter));
        assert_eq!(Provider::parse(" ollama "), Some(Provider::Ollama));
    }

    #[test]
    fn provider_parse_rejects_unknown() {
        assert!(Provider::parse("groq").is_none());
        assert!(Provider::parse("").is_none());
    }

    #[test]
    fn requires_key_only_for_remote() {
        assert!(!Provider::Ollama.requires_key());
        assert!(Provider::OpenAi.requires_key());
        assert!(Provider::Anthropic.requires_key());
        assert!(Provider::Gemini.requires_key());
        assert!(Provider::OpenRouter.requires_key());
    }

    #[test]
    fn default_model_is_set_for_every_provider() {
        for &p in Provider::all() {
            assert!(!p.default_model().is_empty());
            assert!(!p.display_name().is_empty());
        }
    }

    #[test]
    fn convert_messages_maps_roles() {
        let msgs = vec![
            Message {
                role: MessageRole::System,
                content: "sys".into(),
            },
            Message {
                role: MessageRole::User,
                content: "u".into(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "a".into(),
            },
        ];
        let converted = convert_messages(&msgs);
        assert_eq!(converted.len(), 3);
    }

    #[test]
    fn provider_serializes_lowercase() {
        let j = serde_json::to_string(&Provider::OpenAi).expect("serialize OpenAi");
        assert_eq!(j, "\"openai\"");
        let j = serde_json::to_string(&Provider::OpenRouter).expect("serialize OpenRouter");
        assert_eq!(j, "\"openrouter\"");
    }

    #[test]
    fn provider_info_round_trip() {
        let info = ProviderInfo {
            id: Provider::Anthropic,
            display_name: Provider::Anthropic.display_name().into(),
            default_model: Provider::Anthropic.default_model().into(),
            requires_key: true,
            key_status: KeyStatus::Missing,
        };
        let j = serde_json::to_string(&info).expect("serialize ProviderInfo");
        let back: ProviderInfo = serde_json::from_str(&j).expect("deserialize ProviderInfo");
        assert_eq!(back.id, info.id);
        assert_eq!(back.requires_key, info.requires_key);
        assert_eq!(back.key_status, info.key_status);
    }

    #[test]
    fn parse_or_err_returns_invalid_arg_on_unknown() {
        let r = parse_or_err("definitely-not-a-provider");
        assert!(matches!(r, Err(AppError::InvalidArgument(_))));
    }

    #[test]
    fn provider_add_rejects_local_provider() {
        let r = provider_add("ollama".into(), "anything".into());
        assert!(matches!(r, Err(AppError::InvalidArgument(_))));
    }

    #[test]
    fn provider_remove_local_is_noop() {
        // Ollama has no key — removing it should not error.
        let r = provider_remove("ollama".into());
        assert!(r.is_ok());
    }
}
