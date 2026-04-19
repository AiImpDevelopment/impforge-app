// SPDX-License-Identifier: MIT
//! Behavioral tests for slash_commands Phase 2 implementation.
//! Type-roundtrip tests stay; new tests exercise catalog + dispatch.

use impforge_app_lib::slash_commands::{catalog, dispatch, SlashCommand, SlashOutcome};

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
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<SlashCommand>();
    assert_traits::<SlashOutcome>();
}

#[test]
fn catalog_has_at_least_50_commands() {
    let cmds = catalog();
    assert!(cmds.len() >= 50, "want >=50, got {}", cmds.len());
}

#[test]
fn catalog_covers_required_categories() {
    let cmds = catalog();
    for required in [
        "chat",
        "knowledge",
        "system",
        "privacy",
        "files",
        "pro",
        "digiimp",
        "auto-digest",
        "format",
        "browser",
        "mcp",
        "widgets",
        "quality",
        "settings",
        "fun",
        "ai",
    ] {
        assert!(
            cmds.iter().any(|c| c.category == required),
            "missing category: {required}"
        );
    }
}

#[test]
fn catalog_command_names_are_unique() {
    let cmds = catalog();
    let mut names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
    let total = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), total, "duplicate command name detected");
}

#[test]
fn slash_help_returns_help() {
    let result = dispatch("/help".into(), None);
    match result {
        SlashOutcome::Text { text } => {
            assert!(text.contains("Available commands"));
            assert!(text.contains("/help"));
        }
        other => panic!("expected Text outcome, got {other:?}"),
    }
}

#[test]
fn unknown_slash_returns_unknown() {
    let result = dispatch("/notarealcommand".into(), None);
    assert!(matches!(result, SlashOutcome::Unknown { .. }));
}

#[test]
fn slash_search_with_arg_returns_search_intent() {
    let result = dispatch("/search rust".into(), None);
    match result {
        SlashOutcome::Intent { name, arg } => {
            assert_eq!(name, "/search");
            assert_eq!(arg.as_deref(), Some("rust"));
        }
        other => panic!("expected Intent outcome, got {other:?}"),
    }
}
