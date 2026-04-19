// SPDX-License-Identifier: MIT
//! Keyring-backed BYOK key storage (Feature 1, Tier 2/3).
//!
//! Wraps `keyring-rs` so the rest of the app never touches the keychain
//! API directly. We never log a key, never serialize one, never expose
//! it across the Tauri IPC boundary unless the caller explicitly asks
//! (and even then, only via a Secret-newtype helper).
//!
//! Layout:
//!   - `(KEYRING_SERVICE, provider_id)` — one entry per provider
//!   - Linux: GNOME Keyring / KWallet via Secret Service
//!   - macOS: Keychain Services (`apple-native` feature)
//!   - Windows: Credential Manager (`windows-native` feature)
//!
//! See research report `2026-04-19-multi-provider-byok.md` §3 (security).

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Service slot in the OS keychain. Stable across versions — renaming
/// invalidates every stored key.
pub const KEYRING_SERVICE: &str = "impforge-app";

/// In-memory wrapper that prevents accidental Debug logging of a key.
/// Conceptually equivalent to `secrecy::Secret<String>` without pulling
/// in another crate — see research §3 ("In-memory hygiene").
pub struct SecretKey(String);

impl SecretKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretKey(<redacted>)")
    }
}

/// Status hint surfaced to the UI without ever leaking the secret value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStatus {
    Set,
    Missing,
    Local,
}

/// Save (or replace) a key in the OS keychain under `provider_id`.
pub fn save_key(provider_id: &str, key: &str) -> AppResult<()> {
    if key.trim().is_empty() {
        return Err(AppError::InvalidArgument(
            "refusing to store an empty key".into(),
        ));
    }
    if provider_id.trim().is_empty() {
        return Err(AppError::InvalidArgument(
            "provider_id must be non-empty".into(),
        ));
    }
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider_id)
        .map_err(|e| AppError::Internal(format!("keyring open: {e}")))?;
    entry
        .set_password(key)
        .map_err(|e| AppError::Internal(format!("keyring write: {e}")))?;
    Ok(())
}

/// Retrieve a key from the keychain. Returns `Ok(None)` for `NoEntry` so the
/// caller can distinguish "missing" from "transport failure".
pub fn get_key(provider_id: &str) -> AppResult<Option<SecretKey>> {
    if provider_id.trim().is_empty() {
        return Err(AppError::InvalidArgument(
            "provider_id must be non-empty".into(),
        ));
    }
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider_id)
        .map_err(|e| AppError::Internal(format!("keyring open: {e}")))?;
    match entry.get_password() {
        Ok(k) => Ok(Some(SecretKey::new(k))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Internal(format!("keyring read: {e}"))),
    }
}

/// Delete a key from the keychain. Returns `Ok(())` even if no entry exists,
/// so the operation is idempotent.
pub fn delete_key(provider_id: &str) -> AppResult<()> {
    if provider_id.trim().is_empty() {
        return Err(AppError::InvalidArgument(
            "provider_id must be non-empty".into(),
        ));
    }
    let entry = keyring::Entry::new(KEYRING_SERVICE, provider_id)
        .map_err(|e| AppError::Internal(format!("keyring open: {e}")))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Internal(format!("keyring delete: {e}"))),
    }
}

/// Has-a-key probe that never exposes the value.
pub fn has_key(provider_id: &str) -> AppResult<bool> {
    Ok(get_key(provider_id)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_key_debug_redacts() {
        let s = SecretKey::new("sk-very-secret");
        let dbg = format!("{:?}", s);
        assert!(!dbg.contains("sk-very-secret"));
        assert!(dbg.contains("redacted"));
    }

    #[test]
    fn secret_key_expose_returns_inner() {
        let s = SecretKey::new("hello");
        assert_eq!(s.expose(), "hello");
    }

    #[test]
    fn secret_key_into_inner_consumes() {
        let s = SecretKey::new("bye");
        assert_eq!(s.into_inner(), "bye");
    }

    #[test]
    fn save_empty_key_rejected() {
        let r = save_key("test-empty", "");
        assert!(matches!(r, Err(AppError::InvalidArgument(_))));
        let r = save_key("test-empty", "   ");
        assert!(matches!(r, Err(AppError::InvalidArgument(_))));
    }

    #[test]
    fn empty_provider_id_rejected_on_get() {
        let r = get_key("");
        assert!(matches!(r, Err(AppError::InvalidArgument(_))));
    }

    #[test]
    fn empty_provider_id_rejected_on_delete() {
        let r = delete_key("");
        assert!(matches!(r, Err(AppError::InvalidArgument(_))));
    }

    #[test]
    fn key_status_serialization() {
        assert_eq!(
            serde_json::to_string(&KeyStatus::Set).expect("serialize Set"),
            "\"set\""
        );
        assert_eq!(
            serde_json::to_string(&KeyStatus::Missing).expect("serialize Missing"),
            "\"missing\""
        );
        assert_eq!(
            serde_json::to_string(&KeyStatus::Local).expect("serialize Local"),
            "\"local\""
        );
    }

    #[test]
    fn keyring_service_is_app_specific() {
        // App's service slot MUST differ from CLI's so installs don't share keys
        // unintentionally (different security boundaries / different UIs).
        assert_eq!(KEYRING_SERVICE, "impforge-app");
        assert_ne!(KEYRING_SERVICE, "impforge-cli");
    }
}
