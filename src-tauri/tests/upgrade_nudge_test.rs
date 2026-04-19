// SPDX-License-Identifier: MIT
//! Behavioral tests for upgrade_nudge Phase 1 type skeleton.

use impforge_app_lib::upgrade_nudge::{NudgeEvent, NudgeKind, NudgeResult};

#[test]
fn nudge_kind_serializes_snake_case() {
    for (k, expected) in [
        (NudgeKind::PersistenceLoss, "persistence_loss"),
        (NudgeKind::SearchDepth, "search_depth"),
        (NudgeKind::KnowledgeLink, "knowledge_link"),
        (NudgeKind::EntityExtract, "entity_extract"),
        (NudgeKind::IntentDispatch, "intent_dispatch"),
        (NudgeKind::StyleLearn, "style_learn"),
        (NudgeKind::AutonomyAudit, "autonomy_audit"),
        (NudgeKind::WidgetCustom, "widget_custom"),
        (NudgeKind::TransportRich, "transport_rich"),
    ] {
        let j = serde_json::to_string(&k).expect("serialize NudgeKind");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn nudge_event_roundtrips() {
    let original = NudgeEvent {
        kind: NudgeKind::SearchDepth,
        trigger_module: "knowledge_lite".into(),
        trigger_at: chrono::Utc::now(),
        session_id: "sess-1".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize NudgeEvent");
    let back: NudgeEvent = serde_json::from_str(&j).expect("deserialize NudgeEvent");
    assert_eq!(original.kind, back.kind);
    assert_eq!(original.trigger_module, back.trigger_module);
}

#[test]
fn nudge_result_roundtrips() {
    let original = NudgeResult {
        should_show: true,
        copy: "Upgrade for deeper search".into(),
        utm_url: "https://impforge.com/?utm_source=app".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize NudgeResult");
    let back: NudgeResult = serde_json::from_str(&j).expect("deserialize NudgeResult");
    assert_eq!(original.should_show, back.should_show);
    assert_eq!(original.copy, back.copy);
    assert_eq!(original.utm_url, back.utm_url);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<NudgeKind>();
    assert_traits::<NudgeEvent>();
    assert_traits::<NudgeResult>();
}
