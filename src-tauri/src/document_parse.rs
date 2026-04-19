// SPDX-License-Identifier: MIT
//! Universal document parser. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.1 SHIP-AS-IS-EQUIVALENT.
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real format detection + text extraction across PDF/DOCX/XLSX/PPTX/ODF/MD/HTML.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Recognised document formats. `Unknown` covers files the parser cannot identify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Pdf,
    Docx,
    Xlsx,
    Pptx,
    Odt,
    Ods,
    Odp,
    Markdown,
    Html,
    Plaintext,
    Unknown,
}

/// Result of parsing a document — extracted plain text plus arbitrary metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    pub format: DocumentFormat,
    pub text: String,
    pub metadata: serde_json::Value,
}

/// Detect the document format of the file at `path` (cheap header sniff).
#[tauri::command]
pub async fn document_detect_format(_path: String) -> AppResult<DocumentFormat> {
    Err(AppError::Internal(
        "document_parse::document_detect_format not implemented in Phase 1".into(),
    ))
}

/// Extract the plain-text content of the document at `path`.
#[tauri::command]
pub async fn document_extract_text(_path: String) -> AppResult<ParseResult> {
    Err(AppError::Internal(
        "document_parse::document_extract_text not implemented in Phase 1".into(),
    ))
}
