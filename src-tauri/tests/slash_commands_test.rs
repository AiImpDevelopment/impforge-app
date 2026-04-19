// SPDX-License-Identifier: MIT
//! Behavioral tests for slash_commands Phase 1 type skeleton.

use impforge_app_lib::slash_commands::{SlashCommand, SlashOutcome};

#[test]
fn slash_command_roundtrips_through_json() {
    let original = SlashCommand {
        name: "help".into(),
        description: "Show available slash commands".into(),
        category: "core".into(),
    };
    let j = serde_json::to_string(&original).expect("serialize SlashCommand");
    let back: SlashCommand = serde_json::from_str(&j).expect("deserialize SlashCommand");
    assert_eq!(original.name, back.name);
    assert_eq!(original.description, back.description);
    assert_eq!(original.category, back.category);
}

#[test]
fn slash_outcome_text_serializes_with_kind_tag() {
    let v = SlashOutcome::Text {
        text: "hello".into(),
    };
    let j = serde_json::to_string(&v).expect("serialize SlashOutcome::Text");
    assert!(j.contains("\"kind\":\"text\""), "got: {j}");
    assert!(j.contains("\"text\":\"hello\""), "got: {j}");
}

#[test]
fn slash_outcome_intent_serializes_with_kind_tag() {
    let v = SlashOutcome::Intent {
        name: "open".into(),
        arg: Some("file.txt".into()),
    };
    let j = serde_json::to_string(&v).expect("serialize SlashOutcome::Intent");
    assert!(j.contains("\"kind\":\"intent\""), "got: {j}");
    assert!(j.contains("\"name\":\"open\""));
}

#[test]
fn slash_outcome_unknown_serializes_with_kind_tag() {
    let v = SlashOutcome::Unknown {
        input: "/foo".into(),
    };
    let j = serde_json::to_string(&v).expect("serialize SlashOutcome::Unknown");
    assert!(j.contains("\"kind\":\"unknown\""), "got: {j}");
    assert!(j.contains("\"input\":\"/foo\""), "got: {j}");
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<SlashCommand>();
    assert_traits::<SlashOutcome>();
}
