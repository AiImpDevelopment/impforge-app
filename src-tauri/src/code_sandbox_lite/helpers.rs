// SPDX-License-Identifier: MIT
//! Shared helpers for the code_sandbox_lite module.
//!
//! Pure functions only — no global state, no I/O at the top level.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::{Limits, MAX_DISK_MIB, MAX_MEMORY_MIB, MAX_STDOUT_BYTES, MAX_TIME_SECS, MIN_MEMORY_MIB, MIN_TIME_SECS};

/// Returns the directory where ImpForge App caches sandbox blobs and
/// session state.  Creates it on first call.
pub fn sandbox_cache_dir() -> std::io::Result<PathBuf> {
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".impforge")
        .join("sandbox");
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

/// Path of the cached Pyodide WASM tarball.
pub fn pyodide_blob_path(version: &str) -> std::io::Result<PathBuf> {
    Ok(sandbox_cache_dir()?.join(format!("pyodide-{version}.tar.bz2")))
}

/// Path where we pin the SHA-256 we observed at first download.
pub fn pyodide_pin_path(version: &str) -> std::io::Result<PathBuf> {
    Ok(sandbox_cache_dir()?.join(format!("pyodide-{version}.sha256")))
}

/// SHA-256 of an arbitrary byte buffer, returned as lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

/// Lowercase-hex encoder — no `hex` dep needed.
pub fn hex_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0xF) as usize] as char);
    }
    out
}

/// UNIX timestamp in seconds (saturating cast).
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Validate user-supplied limits against the Tier-2 floor + ceiling.
pub fn validate_limits(l: &Limits) -> Result<(), String> {
    if !(MIN_MEMORY_MIB..=MAX_MEMORY_MIB).contains(&l.memory_mib) {
        return Err(format!(
            "memory {} MiB out of range; must be between {} and {} MiB",
            l.memory_mib, MIN_MEMORY_MIB, MAX_MEMORY_MIB
        ));
    }
    if !(MIN_TIME_SECS..=MAX_TIME_SECS).contains(&l.time_secs) {
        return Err(format!(
            "time {} s out of range; must be between {} and {} s",
            l.time_secs, MIN_TIME_SECS, MAX_TIME_SECS
        ));
    }
    if l.stdout_cap > MAX_STDOUT_BYTES {
        return Err(format!(
            "stdout_cap {} > MAX_STDOUT_BYTES {}",
            l.stdout_cap, MAX_STDOUT_BYTES
        ));
    }
    if l.disk_mib > MAX_DISK_MIB {
        return Err(format!(
            "disk_mib {} > MAX_DISK_MIB {}",
            l.disk_mib, MAX_DISK_MIB
        ));
    }
    Ok(())
}

/// Generate a UUID-v4-style id (random hex).  Avoids the full `uuid`
/// crate when we just need an opaque identifier.
pub fn new_id(prefix: &str) -> String {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(nanos.to_le_bytes());
    let hash = hasher.finalize();
    format!("{prefix}-{}", hex_encode(&hash[..8]))
}

/// Base64-encode using the `base64` crate (already a workspace dep).
/// Used for Output::Image payloads.
pub fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Decode base64 — symmetric with `base64_encode`.
pub fn base64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_sandbox_lite::types::Limits;

    #[test]
    fn sha256_hex_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hex_encode_handles_edges() {
        assert_eq!(hex_encode(&[]), "");
        assert_eq!(hex_encode(&[0]), "00");
        assert_eq!(hex_encode(&[0xFF]), "ff");
        assert_eq!(hex_encode(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
    }

    #[test]
    fn validate_limits_accepts_default() {
        validate_limits(&Limits::default()).expect("default limits valid");
    }

    #[test]
    fn validate_limits_rejects_oversize_memory() {
        let bad = Limits {
            memory_mib: 1_000_000,
            ..Limits::default()
        };
        assert!(validate_limits(&bad).is_err());
    }

    #[test]
    fn validate_limits_rejects_zero_time() {
        let bad = Limits {
            time_secs: 0,
            ..Limits::default()
        };
        assert!(validate_limits(&bad).is_err());
    }

    #[test]
    fn new_id_is_unique_and_has_prefix() {
        let a = new_id("cell");
        let b = new_id("cell");
        assert!(a.starts_with("cell-"));
        assert_ne!(a, b, "two ids should differ");
    }

    #[test]
    fn now_unix_is_positive() {
        assert!(now_unix() > 1_700_000_000); // past 2023-11
    }

    #[test]
    fn base64_round_trip() {
        let raw = b"hello world";
        let b64 = base64_encode(raw);
        let back = base64_decode(&b64).expect("decode");
        assert_eq!(back, raw);
    }

    #[test]
    fn pyodide_paths_have_version_and_extension() {
        let blob = pyodide_blob_path("0.28.3").expect("blob");
        let pin = pyodide_pin_path("0.28.3").expect("pin");
        assert!(blob.to_string_lossy().contains("0.28.3"));
        assert!(blob.to_string_lossy().ends_with(".tar.bz2"));
        assert!(pin.to_string_lossy().ends_with(".sha256"));
    }
}
