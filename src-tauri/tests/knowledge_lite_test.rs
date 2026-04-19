// SPDX-License-Identifier: MIT
//! Behavioral tests for knowledge_lite Phase 1 type skeleton.

use impforge_app_lib::knowledge_lite::{KnowledgeEntry, SearchResult};

fn sample_entry() -> KnowledgeEntry {
    KnowledgeEntry {
        id: "id-1".into(),
        source: "wiki".into(),
        title: "Sample".into(),
        body: "Hello world".into(),
        ingested_at: chrono::Utc::now(),
    }
}

#[test]
fn knowledge_entry_roundtrips_through_json() {
    let original = sample_entry();
    let j = serde_json::to_string(&original).expect("serialize KnowledgeEntry");
    let back: KnowledgeEntry = serde_json::from_str(&j).expect("deserialize KnowledgeEntry");
    assert_eq!(original.id, back.id);
    assert_eq!(original.title, back.title);
    assert_eq!(original.body, back.body);
}

#[test]
fn search_result_roundtrips_through_json() {
    let original = SearchResult {
        entry: sample_entry(),
        rank: 0.87,
        snippet: "Hello".into(),
        sub_scores: serde_json::json!({"porter_rank": 1, "trigram_rank": null}),
        line_start: 1,
        line_end: 12,
    };
    let j = serde_json::to_string(&original).expect("serialize SearchResult");
    let back: SearchResult = serde_json::from_str(&j).expect("deserialize SearchResult");
    assert_eq!(original.rank, back.rank);
    assert_eq!(original.snippet, back.snippet);
    assert_eq!(original.line_start, back.line_start);
    assert_eq!(original.line_end, back.line_end);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<KnowledgeEntry>();
    assert_traits::<SearchResult>();
}
