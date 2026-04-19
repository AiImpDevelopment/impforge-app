// SPDX-License-Identifier: MIT
//! Behavioural integration tests for auto_digest_lite Phase 2 (Tier 2
//! of Feature 3, Global Digest).  Internal unit tests live alongside
//! the module; this file exercises the public-API surface only.

use impforge_app_lib::auto_digest_lite::{
    DigestEntry, DigestSource, DigestState, RunOnceSummary,
};

#[test]
fn digest_source_feed_serializes_with_kind_tag() {
    let v = DigestSource::Feed {
        id: "feed-1".into(),
        url: "https://example.com/feed".into(),
        interval_secs: 300,
        last_modified: None,
        etag: None,
        last_polled_unix: 0,
    };
    let j = serde_json::to_string(&v).expect("serialize DigestSource::Feed");
    assert!(j.contains("\"kind\":\"feed\""), "got: {j}");
    assert!(
        j.contains("\"url\":\"https://example.com/feed\""),
        "got: {j}"
    );
}

#[test]
fn digest_source_folder_serializes_with_kind_tag() {
    let v = DigestSource::Folder {
        id: "folder-1".into(),
        path: std::path::PathBuf::from("/tmp/data"),
        recursive: true,
        allow_ext: vec!["md".into()],
    };
    let j = serde_json::to_string(&v).expect("serialize");
    assert!(j.contains("\"kind\":\"folder\""), "got: {j}");
}

#[test]
fn digest_source_clipboard_screenshots_browser_round_trip() {
    let sources = vec![
        DigestSource::Clipboard { id: "cb-1".into() },
        DigestSource::Screenshots {
            id: "ss-1".into(),
            path: None,
        },
        DigestSource::Browser {
            id: "br-1".into(),
            family: "firefox".into(),
        },
    ];
    for src in sources {
        let j = serde_json::to_string(&src).expect("serialize");
        let back: DigestSource = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(src.id(), back.id());
    }
}

#[test]
fn digest_state_default_is_safe() {
    let state = DigestState::default();
    assert!(!state.paused, "default state must NOT be paused");
    assert!(state.sources.is_empty(), "default state has no sources");
    assert!(
        state.quiet_hours_enabled,
        "default state has quiet hours enabled (Recall lesson)"
    );
}

#[test]
fn digest_entry_carries_pii_redaction_count() {
    let entry = DigestEntry {
        source_id: "src-1".into(),
        kind: "feed".into(),
        title: "test".into(),
        url_or_path: "https://example.com/x".into(),
        fetched_at: chrono::Utc::now(),
        pii_redactions: 3,
        bytes: 100,
    };
    let j = serde_json::to_string(&entry).expect("ser");
    let back: DigestEntry = serde_json::from_str(&j).expect("de");
    assert_eq!(back.pii_redactions, 3);
}

#[test]
fn run_once_summary_has_required_counters() {
    let s = RunOnceSummary::default();
    let j = serde_json::to_string(&s).expect("ser");
    // The summary's shape is part of the public API consumed by the
    // Svelte UI — guard against accidental field renames.
    for field in &[
        "feeds_pulled",
        "feeds_unchanged",
        "entries_persisted",
        "folders_swept",
        "files_indexed",
        "chunks_indexed",
        "pii_redactions",
        "errors",
    ] {
        assert!(
            j.contains(&format!("\"{field}\"")),
            "RunOnceSummary missing field {field}: {j}"
        );
    }
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<DigestSource>();
    assert_traits::<DigestEntry>();
    assert_traits::<DigestState>();
    assert_traits::<RunOnceSummary>();
}
