// SPDX-License-Identifier: MIT
//! Behavioral tests for injection_firewall Phase 1 type skeleton.

use impforge_app_lib::injection_firewall::{InjectionScan, InjectionVerdict};

#[test]
fn injection_verdict_serializes_snake_case() {
    for (v, expected) in [
        (InjectionVerdict::Clean, "clean"),
        (InjectionVerdict::Suspicious, "suspicious"),
        (InjectionVerdict::Blocked, "blocked"),
    ] {
        let j = serde_json::to_string(&v).expect("serialize InjectionVerdict");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn injection_scan_roundtrips() {
    let original = InjectionScan {
        verdict: InjectionVerdict::Suspicious,
        patterns_matched: vec!["ignore previous instructions".into()],
        sanitized: "user said something".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize InjectionScan");
    let back: InjectionScan = serde_json::from_str(&j).expect("deserialize InjectionScan");
    assert_eq!(original.verdict, back.verdict);
    assert_eq!(original.patterns_matched, back.patterns_matched);
    assert_eq!(original.sanitized, back.sanitized);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<InjectionVerdict>();
    assert_traits::<InjectionScan>();
}
