// SPDX-License-Identifier: MIT
//! AES-256-GCM encryption helpers. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real AES-256-GCM via `aes-gcm` + Argon2 KDF (RustCrypto only).

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// One AES-256-GCM ciphertext blob with the nonce + KDF salt needed to decrypt it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBlob {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub kdf_salt: [u8; 16],
}

/// Opaque handle identifying an in-memory key (UUID-based, not the key itself).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyHandle(pub String);

/// Encrypt the given plaintext with the key referenced by `key`.
#[tauri::command]
pub async fn crypto_encrypt(_plaintext: Vec<u8>, _key: KeyHandle) -> AppResult<EncryptedBlob> {
    Err(AppError::Internal(
        "crypto_lite::crypto_encrypt not implemented in Phase 1".into(),
    ))
}

/// Decrypt the given blob with the key referenced by `key`.
#[tauri::command]
pub async fn crypto_decrypt(_blob: EncryptedBlob, _key: KeyHandle) -> AppResult<Vec<u8>> {
    Err(AppError::Internal(
        "crypto_lite::crypto_decrypt not implemented in Phase 1".into(),
    ))
}

/// Derive a fresh key from a passphrase via Argon2 and return its handle.
#[tauri::command]
pub async fn crypto_derive_key(_passphrase: String) -> AppResult<KeyHandle> {
    Err(AppError::Internal(
        "crypto_lite::crypto_derive_key not implemented in Phase 1".into(),
    ))
}
