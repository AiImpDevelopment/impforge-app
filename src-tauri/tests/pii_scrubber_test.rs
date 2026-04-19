// SPDX-License-Identifier: MIT
//! Behavioral tests for pii_scrubber Phase 1 type skeleton.

use impforge_app_lib::pii_scrubber::{PiiKind, PiiMatch, ScrubResult};

#[test]
fn pii_kind_serializes_snake_case() {
    for (k, expected) in [
        (PiiKind::Email, "email"),
        (PiiKind::Phone, "phone"),
        (PiiKind::Iban, "iban"),
        (PiiKind::CreditCard, "credit_card"),
        (PiiKind::Ssn, "ssn"),
        (PiiKind::IpAddress, "ip_address"),
        (PiiKind::Address, "address"),
        (PiiKind::FullName, "full_name"),
    ] {
        let j = serde_json::to_string(&k).expect("serialize PiiKind");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn pii_match_roundtrips() {
    let original = PiiMatch {
        kind: PiiKind::Email,
        start: 5,
        end: 25,
        redacted: "[EMAIL]".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize PiiMatch");
    let back: PiiMatch = serde_json::from_str(&j).expect("deserialize PiiMatch");
    assert_eq!(original.kind, back.kind);
    assert_eq!(original.start, back.start);
    assert_eq!(original.end, back.end);
}

#[test]
fn scrub_result_roundtrips() {
    let original = ScrubResult {
        scrubbed: "Contact: [EMAIL]".into(),
        matches: vec![],
    };
    let j = serde_json::to_string(&original).expect("serialize ScrubResult");
    let back: ScrubResult = serde_json::from_str(&j).expect("deserialize ScrubResult");
    assert_eq!(original.scrubbed, back.scrubbed);
    assert_eq!(original.matches.len(), back.matches.len());
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<PiiKind>();
    assert_traits::<PiiMatch>();
    assert_traits::<ScrubResult>();
}
