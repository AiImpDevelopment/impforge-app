// SPDX-License-Identifier: MIT
//! PII detection + scrubbing. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.1
//! SHIP-AS-IS-EQUIVALENT (subset of the Pro `privacy_shield` engine).
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real regex-based detection + redaction with locale awareness.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Categories of personally-identifiable information the scrubber recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PiiKind {
    Email,
    Phone,
    Iban,
    CreditCard,
    Ssn,
    IpAddress,
    Address,
    FullName,
}

/// One PII match within an input string. Byte offsets into the original text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiMatch {
    pub kind: PiiKind,
    pub start: usize,
    pub end: usize,
    pub redacted: String,
}

/// Result of running the scrubber: cleaned text plus the list of matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrubResult {
    pub scrubbed: String,
    pub matches: Vec<PiiMatch>,
}

/// Detect (but do not redact) all PII matches in the input text.
#[tauri::command]
pub async fn pii_detect(_text: String) -> AppResult<Vec<PiiMatch>> {
    Err(AppError::Internal(
        "pii_scrubber::pii_detect not implemented in Phase 1".into(),
    ))
}

/// Scrub the input text, replacing all detected PII with redacted markers.
#[tauri::command]
pub async fn pii_scrub(_text: String) -> AppResult<ScrubResult> {
    Err(AppError::Internal(
        "pii_scrubber::pii_scrub not implemented in Phase 1".into(),
    ))
}
