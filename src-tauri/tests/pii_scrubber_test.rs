// SPDX-License-Identifier: MIT
//! Behavioural integration tests for pii_scrubber Phase 2 — actual
//! detection + redaction.  Updated as part of Feature 3 (Global
//! Digest, Tier 2) which depends on a working scrubber for Recall
//! lesson #4.

use impforge_app_lib::pii_scrubber::{
    detect, scrub, PiiKind, PiiMatch, ScrubResult,
};

#[test]
fn pii_kind_serializes_snake_case() {
    for (k, expected) in [
        (PiiKind::Email, "email"),
        (PiiKind::Phone, "phone"),
        (PiiKind::Iban, "iban"),
        (PiiKind::CreditCard, "credit_card"),
        (PiiKind::Ssn, "ssn"),
        (PiiKind::IpAddress, "ip_address"),
        (PiiKind::ApiKey, "api_key"),
        (PiiKind::JwtToken, "jwt_token"),
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
    let j = serde_json::to_string(&original).expect("ser");
    let back: PiiMatch = serde_json::from_str(&j).expect("de");
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
    let j = serde_json::to_string(&original).expect("ser");
    let back: ScrubResult = serde_json::from_str(&j).expect("de");
    assert_eq!(original.scrubbed, back.scrubbed);
}

#[test]
fn detect_finds_email_and_returns_match_offsets() {
    let m = detect("write to test@example.com please");
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].kind, PiiKind::Email);
    assert!(m[0].end > m[0].start);
}

#[test]
fn scrub_replaces_with_redaction_marker() {
    let r = scrub("Card 4111-1111-1111-1111 expires").expect("scrub");
    assert!(r.scrubbed.contains("[REDACTED:CCN]"));
    assert!(r.matches.iter().any(|m| m.kind == PiiKind::CreditCard));
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
