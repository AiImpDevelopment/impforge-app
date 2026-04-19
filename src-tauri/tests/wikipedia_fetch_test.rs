// SPDX-License-Identifier: MIT
//! Behavioral tests for wikipedia_fetch Phase 1 type skeleton.

use impforge_app_lib::wikipedia_fetch::{WikipediaArticle, WikipediaSearchHit};

#[test]
fn wikipedia_article_roundtrips_through_json() {
    let original = WikipediaArticle {
        title: "Rust (programming language)".into(),
        summary: "Rust is a multi-paradigm language".into(),
        body: "Long body".into(),
        url: "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
        fetched_at: chrono::Utc::now(),
    };
    let j = serde_json::to_string(&original).expect("serialize WikipediaArticle");
    let back: WikipediaArticle = serde_json::from_str(&j).expect("deserialize WikipediaArticle");
    assert_eq!(original.title, back.title);
    assert_eq!(original.url, back.url);
}

#[test]
fn wikipedia_search_hit_roundtrips() {
    let original = WikipediaSearchHit {
        title: "Rust".into(),
        snippet: "Rust is a language...".into(),
        url: "https://en.wikipedia.org/wiki/Rust".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize WikipediaSearchHit");
    let back: WikipediaSearchHit =
        serde_json::from_str(&j).expect("deserialize WikipediaSearchHit");
    assert_eq!(original.title, back.title);
    assert_eq!(original.snippet, back.snippet);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<WikipediaArticle>();
    assert_traits::<WikipediaSearchHit>();
}
