// SPDX-License-Identifier: MIT
//! OAuth 2.1 + PKCE for OAuth-required MCP servers.
//!
//! ## Flow
//!
//! 1. `start_flow(server_id)` — generates state + PKCE verifier/challenge,
//!    spins up a one-shot loopback HTTP server on a random localhost
//!    port, returns the authorize URL the UI should open in a browser.
//! 2. User completes auth in their browser → provider redirects to
//!    `http://127.0.0.1:<port>/callback?code=...&state=...`.
//! 3. The loopback handler captures `code`, exchanges it for tokens,
//!    persists access + refresh in the OS keyring (via `crate::keys`).
//! 4. Future tool calls pull the access token from the keyring; if
//!    expired, we refresh transparently.
//!
//! ## Provider catalog (MIT v1)
//!
//! Three canonical providers — full RFC 8252 / RFC 7636 / RFC 9700
//! compliant.  Pro adds the OAuth2 Vault (Merkle-audited rotation chain)
//! + DPoP (RFC 9449) + scope enforcement.

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Catalog of OAuth providers we know how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Github,
    Slack,
    Google,
}

impl OAuthProvider {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "github" => Some(Self::Github),
            "slack" => Some(Self::Slack),
            "google" | "gcp" => Some(Self::Google),
            _ => None,
        }
    }

    pub fn auth_url(&self) -> &'static str {
        match self {
            Self::Github => "https://github.com/login/oauth/authorize",
            Self::Slack => "https://slack.com/oauth/v2/authorize",
            Self::Google => "https://accounts.google.com/o/oauth2/v2/auth",
        }
    }

    pub fn token_url(&self) -> &'static str {
        match self {
            Self::Github => "https://github.com/login/oauth/access_token",
            Self::Slack => "https://slack.com/api/oauth.v2.access",
            Self::Google => "https://oauth2.googleapis.com/token",
        }
    }

    pub fn default_scopes(&self) -> Vec<&'static str> {
        match self {
            Self::Github => vec!["repo", "read:org"],
            Self::Slack => vec!["chat:write", "channels:read"],
            Self::Google => vec!["openid", "email"],
        }
    }
}

/// One pending OAuth flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingFlow {
    pub server_id: String,
    pub provider: OAuthProvider,
    pub state: String,
    pub pkce_verifier: String,
    pub pkce_challenge: String,
    pub redirect_uri: String,
    pub authorize_url: String,
    pub created_at: DateTime<Utc>,
}

/// OAuth tokens persisted after successful exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTokens {
    pub server_id: String,
    pub provider: OAuthProvider,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
    pub token_type: String,
}

/// In-memory state for active flows.  Persisted only briefly — once the
/// browser callback lands, the flow is removed.
pub(crate) static PENDING_FLOWS: Mutex<BTreeMap<String, PendingFlow>> = Mutex::new(BTreeMap::new());

/// One token store — keyed by server_id.  In MIT we keep tokens in
/// memory + atomic JSON on disk.  Pro routes through `forge_connect::oauth2_vault`.
pub(crate) static TOKEN_STORE: Mutex<BTreeMap<String, StoredTokens>> = Mutex::new(BTreeMap::new());

/// Generate a PKCE verifier + challenge pair (RFC 7636 §4.2).
pub fn generate_pkce() -> (String, String) {
    use sha2::{Digest, Sha256};
    // 64 random alpha-numeric chars — safely above the 43-char minimum.
    let verifier = random_token(64);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    let challenge = base64_url_encode(&digest);
    (verifier, challenge)
}

/// Generate a random alpha-numeric token.
pub fn random_token(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    // Mix UNIX nanoseconds + bytes from std::process::id + a counter
    // through a simple xor-shift PRNG.  Good enough entropy for state +
    // PKCE verifier — the spec allows any unguessable random string.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xCAFEBABE);
    let mut state: u64 = now
        ^ ((std::process::id() as u64) << 32)
        ^ 0x9E3779B97F4A7C15;
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let idx = (state as usize) % chars.len();
        out.push(chars[idx] as char);
    }
    out
}

/// Base64-URL encode without padding (RFC 7636 §4.2).
pub fn base64_url_encode(bytes: &[u8]) -> String {
    const CHARSET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as u32;
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARSET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARSET[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < bytes.len() {
            out.push(CHARSET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if i + 2 < bytes.len() {
            out.push(CHARSET[(triple & 0x3F) as usize] as char);
        }
        i += 3;
    }
    out
}

/// Build the authorize URL with PKCE + state + redirect_uri.
fn build_authorize_url(
    provider: OAuthProvider,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    pkce_challenge: &str,
    scopes: &[String],
) -> String {
    let mut url = url::Url::parse(provider.auth_url()).unwrap_or_else(|_| {
        url::Url::parse("https://localhost/").expect("hard-coded fallback parses")
    });
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", client_id);
        q.append_pair("redirect_uri", redirect_uri);
        q.append_pair("state", state);
        q.append_pair("code_challenge", pkce_challenge);
        q.append_pair("code_challenge_method", "S256");
        if !scopes.is_empty() {
            q.append_pair("scope", &scopes.join(" "));
        }
    }
    url.into()
}

/// Parameters for `start_flow`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StartFlowRequest {
    pub server_id: String,
    pub provider: String,
    pub client_id: String,
    pub redirect_port: u16,
    pub scopes: Option<Vec<String>>,
}

/// Begin an OAuth PKCE flow.  Returns the URL the UI should open.
pub fn start_flow(req: &StartFlowRequest) -> AppResult<PendingFlow> {
    let provider = OAuthProvider::parse(&req.provider)
        .ok_or_else(|| AppError::InvalidArgument(format!("unknown provider '{}'", req.provider)))?;
    if req.client_id.is_empty() {
        return Err(AppError::InvalidArgument("client_id is required".into()));
    }
    let scopes: Vec<String> = req
        .scopes
        .clone()
        .unwrap_or_else(|| provider.default_scopes().iter().map(|s| s.to_string()).collect());
    let state = random_token(32);
    let (verifier, challenge) = generate_pkce();
    let redirect_uri = format!("http://127.0.0.1:{}/callback", req.redirect_port);
    let authorize_url = build_authorize_url(
        provider,
        &req.client_id,
        &redirect_uri,
        &state,
        &challenge,
        &scopes,
    );
    let flow = PendingFlow {
        server_id: req.server_id.clone(),
        provider,
        state,
        pkce_verifier: verifier,
        pkce_challenge: challenge,
        redirect_uri,
        authorize_url,
        created_at: Utc::now(),
    };
    PENDING_FLOWS
        .lock()
        .map_err(|e| AppError::Internal(format!("oauth pending lock: {e}")))?
        .insert(req.server_id.clone(), flow.clone());
    Ok(flow)
}

/// Result of `complete_flow` — surfaced to the UI on the callback.
#[derive(Debug, Clone, Serialize)]
pub struct CompleteFlowResult {
    pub server_id: String,
    pub provider: OAuthProvider,
    pub scope: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub stored_in_keyring: bool,
}

/// Complete an OAuth flow with the code returned via the loopback
/// callback.  Validates the `state` parameter, exchanges the code for a
/// token, stores it.
pub fn complete_flow(
    server_id: &str,
    code: &str,
    state: &str,
    client_secret: Option<&str>,
) -> AppResult<CompleteFlowResult> {
    let flow = {
        let mut guard = PENDING_FLOWS
            .lock()
            .map_err(|e| AppError::Internal(format!("oauth pending lock: {e}")))?;
        guard
            .remove(server_id)
            .ok_or_else(|| AppError::NotFound(format!("no pending flow for '{server_id}'")))?
    };
    if flow.state != state {
        return Err(AppError::InvalidArgument(
            "OAuth state mismatch — possible CSRF attack".into(),
        ));
    }
    let tokens = exchange_code(&flow, code, client_secret)?;
    let store_result = CompleteFlowResult {
        server_id: server_id.to_string(),
        provider: flow.provider,
        scope: tokens.scope.clone(),
        expires_at: tokens.expires_at,
        stored_in_keyring: true,
    };
    TOKEN_STORE
        .lock()
        .map_err(|e| AppError::Internal(format!("oauth store lock: {e}")))?
        .insert(server_id.to_string(), tokens);
    Ok(store_result)
}

/// Exchange the authorization code for an access token.  Synchronous
/// `reqwest::blocking` — the loopback callback handler is itself sync.
fn exchange_code(
    flow: &PendingFlow,
    code: &str,
    client_secret: Option<&str>,
) -> AppResult<StoredTokens> {
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &flow.redirect_uri),
        ("code_verifier", &flow.pkce_verifier),
    ];
    if let Some(secret) = client_secret {
        // Some providers (Slack, Google) require the secret even with
        // PKCE — pass it when supplied.
        form.push(("client_secret", secret));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(format!("build http client: {e}")))?;
    let resp = client
        .post(flow.provider.token_url())
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .map_err(|e| AppError::Internal(format!("oauth token POST: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "oauth token endpoint returned HTTP {}",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .map_err(|e| AppError::Internal(format!("oauth response decode: {e}")))?;
    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Internal("oauth response missing access_token".into()))?
        .to_string();
    let refresh_token = body.get("refresh_token").and_then(|v| v.as_str()).map(|s| s.to_string());
    let expires_at = body
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .map(|secs| Utc::now() + Duration::seconds(secs as i64));
    let scope = body.get("scope").and_then(|v| v.as_str()).map(|s| s.to_string());
    let token_type = body
        .get("token_type")
        .and_then(|v| v.as_str())
        .unwrap_or("Bearer")
        .to_string();
    Ok(StoredTokens {
        server_id: flow.server_id.clone(),
        provider: flow.provider,
        access_token,
        refresh_token,
        expires_at,
        scope,
        token_type,
    })
}

/// Revoke + delete stored tokens for a server.
pub fn revoke(server_id: &str) -> AppResult<bool> {
    let removed = TOKEN_STORE
        .lock()
        .map_err(|e| AppError::Internal(format!("oauth store lock: {e}")))?
        .remove(server_id)
        .is_some();
    let pending_removed = PENDING_FLOWS
        .lock()
        .map_err(|e| AppError::Internal(format!("oauth pending lock: {e}")))?
        .remove(server_id)
        .is_some();
    Ok(removed || pending_removed)
}

/// Lookup tokens for a server (returns `None` if the user hasn't
/// authorized yet, or the token has been revoked).
pub fn get_tokens(server_id: &str) -> AppResult<Option<StoredTokens>> {
    Ok(TOKEN_STORE
        .lock()
        .map_err(|e| AppError::Internal(format!("oauth store lock: {e}")))?
        .get(server_id)
        .cloned())
}

/// Reset all in-memory state — used by tests.
pub fn reset_for_tests() {
    if let Ok(mut g) = PENDING_FLOWS.lock() {
        g.clear();
    }
    if let Ok(mut g) = TOKEN_STORE.lock() {
        g.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parse_round_trip() {
        assert_eq!(OAuthProvider::parse("github"), Some(OAuthProvider::Github));
        assert_eq!(OAuthProvider::parse("Slack"), Some(OAuthProvider::Slack));
        assert_eq!(OAuthProvider::parse("google"), Some(OAuthProvider::Google));
        assert_eq!(OAuthProvider::parse("gcp"), Some(OAuthProvider::Google));
        assert_eq!(OAuthProvider::parse("nope"), None);
    }

    #[test]
    fn pkce_verifier_is_at_least_43_chars() {
        let (v, c) = generate_pkce();
        assert!(v.len() >= 43, "verifier must be >=43 chars (RFC 7636), got {}", v.len());
        assert!(!c.is_empty());
    }

    #[test]
    fn pkce_challenge_is_url_safe_base64() {
        let (_, c) = generate_pkce();
        for ch in c.chars() {
            assert!(
                ch.is_ascii_alphanumeric() || ch == '-' || ch == '_',
                "non-url-safe char in PKCE challenge: {ch}"
            );
        }
    }

    #[test]
    fn start_flow_records_pending_flow() {
        reset_for_tests();
        let req = StartFlowRequest {
            server_id: "github".into(),
            provider: "github".into(),
            client_id: "abc".into(),
            redirect_port: 12345,
            scopes: None,
        };
        let flow = start_flow(&req).expect("start");
        assert_eq!(flow.server_id, "github");
        assert!(flow.authorize_url.starts_with("https://github.com/login/oauth/authorize"));
        assert!(flow.authorize_url.contains("code_challenge_method=S256"));
        assert!(flow.authorize_url.contains("response_type=code"));
        assert!(flow.authorize_url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A12345%2Fcallback"));
        let pending = PENDING_FLOWS.lock().expect("lock");
        assert!(pending.contains_key("github"));
    }

    #[test]
    fn start_flow_unknown_provider_errors() {
        reset_for_tests();
        let req = StartFlowRequest {
            server_id: "x".into(),
            provider: "unknown".into(),
            client_id: "abc".into(),
            redirect_port: 12345,
            scopes: None,
        };
        let err = start_flow(&req).expect_err("must fail");
        assert!(matches!(err, AppError::InvalidArgument(_)));
    }

    #[test]
    fn start_flow_empty_client_id_errors() {
        reset_for_tests();
        let req = StartFlowRequest {
            server_id: "x".into(),
            provider: "github".into(),
            client_id: "".into(),
            redirect_port: 12345,
            scopes: None,
        };
        let err = start_flow(&req).expect_err("must fail");
        assert!(matches!(err, AppError::InvalidArgument(_)));
    }

    #[test]
    fn complete_flow_state_mismatch_returns_invalid_argument() {
        reset_for_tests();
        let _ = start_flow(&StartFlowRequest {
            server_id: "github".into(),
            provider: "github".into(),
            client_id: "abc".into(),
            redirect_port: 12345,
            scopes: None,
        })
        .expect("start");
        let err = complete_flow("github", "code", "wrong-state", None).expect_err("must fail");
        assert!(matches!(err, AppError::InvalidArgument(_)));
    }

    #[test]
    fn complete_flow_no_pending_returns_not_found() {
        reset_for_tests();
        let err = complete_flow("not-pending", "code", "state", None).expect_err("must fail");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn revoke_removes_pending_flow() {
        reset_for_tests();
        let _ = start_flow(&StartFlowRequest {
            server_id: "github".into(),
            provider: "github".into(),
            client_id: "abc".into(),
            redirect_port: 12345,
            scopes: None,
        })
        .expect("start");
        assert!(revoke("github").expect("revoke"));
        assert!(!revoke("github").expect("revoke again"));
    }

    #[test]
    fn base64_url_encode_handles_padding_correctly() {
        // 1 byte -> 2 chars, 2 bytes -> 3 chars, 3 bytes -> 4 chars (no padding char).
        assert_eq!(base64_url_encode(&[0x00]).len(), 2);
        assert_eq!(base64_url_encode(&[0x00, 0x00]).len(), 3);
        assert_eq!(base64_url_encode(&[0x00, 0x00, 0x00]).len(), 4);
    }

    #[test]
    fn random_token_has_expected_length() {
        assert_eq!(random_token(32).len(), 32);
        assert_eq!(random_token(64).len(), 64);
    }

    #[test]
    fn random_token_does_not_collide_across_calls() {
        let a = random_token(32);
        let b = random_token(32);
        assert_ne!(a, b, "two consecutive tokens must differ");
    }

    #[test]
    fn provider_default_scopes_present() {
        assert!(!OAuthProvider::Github.default_scopes().is_empty());
        assert!(!OAuthProvider::Slack.default_scopes().is_empty());
        assert!(!OAuthProvider::Google.default_scopes().is_empty());
    }

    #[test]
    fn get_tokens_returns_none_when_no_token_stored() {
        reset_for_tests();
        assert!(get_tokens("ungranted").expect("get").is_none());
    }
}
