// SPDX-License-Identifier: MIT
//! Behavioral tests for digu_privacy_full Phase 1 type skeleton.

use impforge_app_lib::digu_privacy_full::{
    PrivacyDecision, PrivacyOperation, PrivacyPolicy, PrivacyTier,
};

#[test]
fn privacy_tier_serializes_lowercase() {
    for (t, expected) in [
        (PrivacyTier::Fortress, "fortress"),
        (PrivacyTier::Vault, "vault"),
        (PrivacyTier::Shield, "shield"),
        (PrivacyTier::Bridge, "bridge"),
        (PrivacyTier::Open, "open"),
    ] {
        let j = serde_json::to_string(&t).expect("serialize PrivacyTier");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn privacy_operation_serializes_snake_case() {
    for (o, expected) in [
        (PrivacyOperation::Inference, "inference"),
        (PrivacyOperation::Storage, "storage"),
        (PrivacyOperation::Network, "network"),
        (PrivacyOperation::Telemetry, "telemetry"),
        (PrivacyOperation::Sharing, "sharing"),
    ] {
        let j = serde_json::to_string(&o).expect("serialize PrivacyOperation");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn privacy_policy_roundtrips() {
    let original = PrivacyPolicy {
        tier: PrivacyTier::Vault,
        allow_cloud: false,
        allow_telemetry: false,
        allow_external_share: false,
        require_pii_scrub: true,
    };
    let j = serde_json::to_string(&original).expect("serialize PrivacyPolicy");
    let back: PrivacyPolicy = serde_json::from_str(&j).expect("deserialize PrivacyPolicy");
    assert_eq!(original.tier, back.tier);
    assert_eq!(original.allow_cloud, back.allow_cloud);
    assert_eq!(original.require_pii_scrub, back.require_pii_scrub);
}

#[test]
fn privacy_decision_roundtrips() {
    let original = PrivacyDecision {
        allowed: false,
        reason: "Cloud not allowed in Fortress tier".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize PrivacyDecision");
    let back: PrivacyDecision = serde_json::from_str(&j).expect("deserialize PrivacyDecision");
    assert_eq!(original.allowed, back.allowed);
    assert_eq!(original.reason, back.reason);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<PrivacyTier>();
    assert_traits::<PrivacyPolicy>();
    assert_traits::<PrivacyOperation>();
    assert_traits::<PrivacyDecision>();
}
