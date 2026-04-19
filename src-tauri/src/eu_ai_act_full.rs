// SPDX-License-Identifier: MIT
//! EU AI Act risk classification + decision logging. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.1
//! SHIP-AS-IS-EQUIVALENT (mandatory legal compliance).
//!
//! Phase 1: type skeleton + Tauri command signatures returning NotImplemented.
//! Phase 2: real risk classifier + persistent decision log + report exporter.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// EU AI Act risk-tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiActRiskTier {
    Minimal,
    Limited,
    HighRisk,
    Unacceptable,
}

/// Persistent record of one AI decision for audit purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDecisionLog {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tier: AiActRiskTier,
    pub model: String,
    pub purpose: String,
    pub user_consented: bool,
}

/// Compliance report covering a date range and aggregated decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
    pub decisions: Vec<AiDecisionLog>,
}

/// Classify the risk tier for the given (model, purpose) tuple.
#[tauri::command]
pub async fn eu_ai_classify_risk(
    _model: String,
    _purpose: String,
) -> AppResult<AiActRiskTier> {
    Err(AppError::Internal(
        "eu_ai_act_full::eu_ai_classify_risk not implemented in Phase 1".into(),
    ))
}

/// Append a decision to the persistent compliance log.
#[tauri::command]
pub async fn eu_ai_log_decision(_log: AiDecisionLog) -> AppResult<()> {
    Err(AppError::Internal(
        "eu_ai_act_full::eu_ai_log_decision not implemented in Phase 1".into(),
    ))
}

/// Export an aggregate compliance report covering the last `days` days.
#[tauri::command]
pub async fn eu_ai_export_report(_days: u32) -> AppResult<ComplianceReport> {
    Err(AppError::Internal(
        "eu_ai_act_full::eu_ai_export_report not implemented in Phase 1".into(),
    ))
}
