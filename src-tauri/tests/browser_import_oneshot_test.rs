// SPDX-License-Identifier: MIT
//! Behavioural integration tests for browser_import_oneshot Phase 2
//! (Tier 2 of Feature 3, Global Digest).

use impforge_app_lib::browser_import_oneshot::{
    BrowserBookmark, BrowserFamily, BrowserHistoryItem, BrowserProfile,
};
use std::path::PathBuf;

#[test]
fn browser_family_serializes_lowercase() {
    for (k, expected) in [
        (BrowserFamily::Firefox, "firefox"),
        (BrowserFamily::Chrome, "chrome"),
        (BrowserFamily::Chromium, "chromium"),
        (BrowserFamily::Brave, "brave"),
        (BrowserFamily::Edge, "edge"),
        (BrowserFamily::Vivaldi, "vivaldi"),
        (BrowserFamily::Opera, "opera"),
    ] {
        let j = serde_json::to_string(&k).expect("serialize BrowserFamily");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn browser_profile_roundtrips() {
    let original = BrowserProfile {
        family: BrowserFamily::Firefox,
        path: PathBuf::from("/home/user/.mozilla"),
        name: "default".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize");
    let back: BrowserProfile = serde_json::from_str(&j).expect("deserialize");
    assert_eq!(original.family, back.family);
    assert_eq!(original.path, back.path);
    assert_eq!(original.name, back.name);
}

#[test]
fn bookmark_roundtrips_through_json() {
    let original = BrowserBookmark {
        url: "https://example.com".into(),
        title: "Example".into(),
        folder: Some("Bookmarks Bar".into()),
        date_added: None,
    };
    let j = serde_json::to_string(&original).expect("ser");
    let back: BrowserBookmark = serde_json::from_str(&j).expect("de");
    assert_eq!(original.url, back.url);
    assert_eq!(original.folder, back.folder);
}

#[test]
fn history_item_roundtrips_through_json() {
    let original = BrowserHistoryItem {
        url: "https://example.com".into(),
        title: Some("Example".into()),
        visit_count: 5,
        last_visit: None,
    };
    let j = serde_json::to_string(&original).expect("ser");
    let back: BrowserHistoryItem = serde_json::from_str(&j).expect("de");
    assert_eq!(original.url, back.url);
    assert_eq!(original.visit_count, 5);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<BrowserFamily>();
    assert_traits::<BrowserProfile>();
    assert_traits::<BrowserBookmark>();
    assert_traits::<BrowserHistoryItem>();
}
