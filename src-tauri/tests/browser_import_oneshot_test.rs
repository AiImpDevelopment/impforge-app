// SPDX-License-Identifier: MIT
//! Behavioral tests for browser_import_oneshot Phase 1 type skeleton.

use impforge_app_lib::browser_import_oneshot::{
    Bookmark, BrowserKind, BrowserProfile, HistoryEntry,
};

#[test]
fn browser_kind_serializes_lowercase() {
    for (k, expected) in [
        (BrowserKind::Firefox, "firefox"),
        (BrowserKind::Chromium, "chromium"),
        (BrowserKind::Brave, "brave"),
        (BrowserKind::Edge, "edge"),
        (BrowserKind::Vivaldi, "vivaldi"),
        (BrowserKind::Opera, "opera"),
    ] {
        let j = serde_json::to_string(&k).expect("serialize BrowserKind");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn browser_profile_roundtrips() {
    let original = BrowserProfile {
        kind: BrowserKind::Firefox,
        path: "/home/user/.mozilla".into(),
        name: "default".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize BrowserProfile");
    let back: BrowserProfile = serde_json::from_str(&j).expect("deserialize BrowserProfile");
    assert_eq!(original.kind, back.kind);
    assert_eq!(original.path, back.path);
    assert_eq!(original.name, back.name);
}

#[test]
fn bookmark_roundtrips_through_json() {
    let original = Bookmark {
        url: "https://example.com".into(),
        title: "Example".into(),
        folder: Some("Bookmarks Bar".into()),
    };
    let j = serde_json::to_string(&original).expect("serialize Bookmark");
    let back: Bookmark = serde_json::from_str(&j).expect("deserialize Bookmark");
    assert_eq!(original.url, back.url);
    assert_eq!(original.folder, back.folder);
}

#[test]
fn history_entry_roundtrips_through_json() {
    let original = HistoryEntry {
        url: "https://example.com".into(),
        title: "Example".into(),
        visited_at: chrono::Utc::now(),
    };
    let j = serde_json::to_string(&original).expect("serialize HistoryEntry");
    let back: HistoryEntry = serde_json::from_str(&j).expect("deserialize HistoryEntry");
    assert_eq!(original.url, back.url);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<BrowserKind>();
    assert_traits::<BrowserProfile>();
    assert_traits::<Bookmark>();
    assert_traits::<HistoryEntry>();
}
