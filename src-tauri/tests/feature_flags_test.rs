// SPDX-License-Identifier: MIT
//! Behavioral tests for feature_flags Phase 1 type skeleton.

use impforge_app_lib::feature_flags::{FeatureCategory, FeatureFlag, FeatureFlagStats};

#[test]
fn feature_category_serializes_snake_case() {
    for (c, expected) in [
        (FeatureCategory::AiCore, "ai_core"),
        (FeatureCategory::Knowledge, "knowledge"),
        (FeatureCategory::Privacy, "privacy"),
        (FeatureCategory::Digiimp, "digiimp"),
        (FeatureCategory::Compliance, "compliance"),
    ] {
        let j = serde_json::to_string(&c).expect("serialize FeatureCategory");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn feature_flag_roundtrips_through_json() {
    let original = FeatureFlag {
        id: "chat.streaming".into(),
        name: "Streaming chat".into(),
        description: "Enable streaming chat responses".into(),
        category: FeatureCategory::AiCore,
        enabled: true,
        default_enabled: true,
    };
    let j = serde_json::to_string(&original).expect("serialize FeatureFlag");
    let back: FeatureFlag = serde_json::from_str(&j).expect("deserialize FeatureFlag");
    assert_eq!(original.id, back.id);
    assert_eq!(original.category, back.category);
    assert_eq!(original.enabled, back.enabled);
}

#[test]
fn feature_flag_stats_roundtrips() {
    let original = FeatureFlagStats {
        total: 47,
        enabled: 30,
        disabled: 17,
    };
    let j = serde_json::to_string(&original).expect("serialize FeatureFlagStats");
    let back: FeatureFlagStats =
        serde_json::from_str(&j).expect("deserialize FeatureFlagStats");
    assert_eq!(original.total, back.total);
    assert_eq!(original.enabled, back.enabled);
    assert_eq!(original.disabled, back.disabled);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<FeatureCategory>();
    assert_traits::<FeatureFlag>();
    assert_traits::<FeatureFlagStats>();
}
