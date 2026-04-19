// SPDX-License-Identifier: MIT
//! Behavioral tests for auto_digest_lite Phase 1 type skeleton.

use impforge_app_lib::auto_digest_lite::{DigestEntry, DigestSource};

#[test]
fn digest_source_rss_serializes_with_kind_tag() {
    let v = DigestSource::Rss {
        url: "https://example.com/feed".into(),
    };
    let j = serde_json::to_string(&v).expect("serialize DigestSource::Rss");
    assert!(j.contains("\"kind\":\"rss\""), "got: {j}");
    assert!(
        j.contains("\"url\":\"https://example.com/feed\""),
        "got: {j}"
    );
}

#[test]
fn digest_source_local_file_serializes_with_kind_tag() {
    let v = DigestSource::LocalFile {
        path: "/tmp/data.txt".into(),
    };
    let j = serde_json::to_string(&v).expect("serialize DigestSource::LocalFile");
    assert!(j.contains("\"kind\":\"local_file\""), "got: {j}");
}

#[test]
fn digest_source_roundtrips() {
    let original = DigestSource::Rss {
        url: "https://x.test".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize");
    let back: DigestSource = serde_json::from_str(&j).expect("deserialize");
    match back {
        DigestSource::Rss { url } => assert_eq!(url, "https://x.test"),
        DigestSource::LocalFile { .. } => panic!("expected Rss variant"),
    }
}

#[test]
fn digest_entry_roundtrips_through_json() {
    let original = DigestEntry {
        source: DigestSource::LocalFile {
            path: "/tmp/x.md".into(),
        },
        title: "Note".into(),
        body: "Content".into(),
        fetched_at: chrono::Utc::now(),
    };
    let j = serde_json::to_string(&original).expect("serialize DigestEntry");
    let back: DigestEntry = serde_json::from_str(&j).expect("deserialize DigestEntry");
    assert_eq!(original.title, back.title);
    assert_eq!(original.body, back.body);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<DigestSource>();
    assert_traits::<DigestEntry>();
}
