// SPDX-License-Identifier: MIT
//! Behavioral tests for eu_ai_act_full Phase 1 type skeleton.

use impforge_app_lib::eu_ai_act_full::{AiActRiskTier, AiDecisionLog, ComplianceReport};

#[test]
fn ai_act_risk_tier_serializes_snake_case() {
    for (t, expected) in [
        (AiActRiskTier::Minimal, "minimal"),
        (AiActRiskTier::Limited, "limited"),
        (AiActRiskTier::HighRisk, "high_risk"),
        (AiActRiskTier::Unacceptable, "unacceptable"),
    ] {
        let j = serde_json::to_string(&t).expect("serialize AiActRiskTier");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn ai_decision_log_roundtrips() {
    let original = AiDecisionLog {
        id: "log-1".into(),
        timestamp: chrono::Utc::now(),
        tier: AiActRiskTier::Limited,
        model: "qwen3-imp:8b".into(),
        purpose: "Chat with user".into(),
        user_consented: true,
    };
    let j = serde_json::to_string(&original).expect("serialize AiDecisionLog");
    let back: AiDecisionLog = serde_json::from_str(&j).expect("deserialize AiDecisionLog");
    assert_eq!(original.id, back.id);
    assert_eq!(original.model, back.model);
    assert_eq!(original.tier, back.tier);
}

#[test]
fn compliance_report_roundtrips() {
    let now = chrono::Utc::now();
    let original = ComplianceReport {
        period_start: now - chrono::Duration::days(7),
        period_end: now,
        decisions: vec![],
    };
    let j = serde_json::to_string(&original).expect("serialize ComplianceReport");
    let back: ComplianceReport = serde_json::from_str(&j).expect("deserialize ComplianceReport");
    assert_eq!(original.decisions.len(), back.decisions.len());
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<AiActRiskTier>();
    assert_traits::<AiDecisionLog>();
    assert_traits::<ComplianceReport>();
}
