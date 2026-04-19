// SPDX-License-Identifier: MIT
//! Behavioral tests for memory_lite Phase 1 type skeleton.

use impforge_app_lib::memory_lite::{Bm25Result, MemoryEntry, MemoryKind};

#[test]
fn memory_kind_serializes_snake_case() {
    for (k, expected) in [
        (MemoryKind::Episodic, "episodic"),
        (MemoryKind::Procedural, "procedural"),
        (MemoryKind::Observational, "observational"),
        (MemoryKind::Zettelkasten, "zettelkasten"),
    ] {
        let j = serde_json::to_string(&k).expect("serialize MemoryKind");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn memory_entry_roundtrips_through_json() {
    let original = MemoryEntry {
        id: "m-1".into(),
        kind: MemoryKind::Episodic,
        content: "I learned TDD".into(),
        timestamp: chrono::Utc::now(),
    };
    let j = serde_json::to_string(&original).expect("serialize MemoryEntry");
    let back: MemoryEntry = serde_json::from_str(&j).expect("deserialize MemoryEntry");
    assert_eq!(original.id, back.id);
    assert_eq!(original.kind, back.kind);
    assert_eq!(original.content, back.content);
}

#[test]
fn bm25_result_roundtrips() {
    let original = Bm25Result {
        entry: MemoryEntry {
            id: "m-2".into(),
            kind: MemoryKind::Procedural,
            content: "Steps".into(),
            timestamp: chrono::Utc::now(),
        },
        score: 1.42,
    };
    let j = serde_json::to_string(&original).expect("serialize Bm25Result");
    let back: Bm25Result = serde_json::from_str(&j).expect("deserialize Bm25Result");
    assert_eq!(original.score, back.score);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<MemoryEntry>();
    assert_traits::<MemoryKind>();
    assert_traits::<Bm25Result>();
}
