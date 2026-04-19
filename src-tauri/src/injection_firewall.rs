// SPDX-License-Identifier: MIT
//! Prompt-injection scanner + sanitiser. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.1
//! SHIP-AS-IS-EQUIVALENT (OWASP LLM01).
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real heuristic + signature scanner with allow-list.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Verdict returned by the injection scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionVerdict {
    Clean,
    Suspicious,
    Blocked,
}

/// Result of one injection scan, including matched patterns and a sanitised payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionScan {
    pub verdict: InjectionVerdict,
    pub patterns_matched: Vec<String>,
    pub sanitized: String,
}

/// Scan the input text for prompt-injection patterns.
#[tauri::command]
pub async fn injection_scan(_text: String) -> AppResult<InjectionScan> {
    Err(AppError::Internal(
        "injection_firewall::injection_scan not implemented in Phase 1".into(),
    ))
}

/// Sanitise the input text by stripping or escaping known injection patterns.
#[tauri::command]
pub async fn injection_sanitize(_text: String) -> AppResult<String> {
    Err(AppError::Internal(
        "injection_firewall::injection_sanitize not implemented in Phase 1".into(),
    ))
}
