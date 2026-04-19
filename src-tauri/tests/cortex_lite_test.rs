// SPDX-License-Identifier: MIT
//! Behavioral tests for cortex_lite Phase 1 type skeleton.

use impforge_app_lib::cortex_lite::{CortexEvent, ToolCall, ToolResult};

#[test]
fn tool_call_roundtrips_through_json() {
    let original = ToolCall {
        name: "knowledge_search".into(),
        arguments: serde_json::json!({"query": "rust"}),
    };
    let j = serde_json::to_string(&original).expect("serialize ToolCall");
    let back: ToolCall = serde_json::from_str(&j).expect("deserialize ToolCall");
    assert_eq!(original.name, back.name);
}

#[test]
fn tool_result_roundtrips_through_json() {
    let original = ToolResult {
        call: ToolCall {
            name: "test".into(),
            arguments: serde_json::json!({}),
        },
        output: serde_json::json!({"ok": true}),
        success: true,
    };
    let j = serde_json::to_string(&original).expect("serialize ToolResult");
    let back: ToolResult = serde_json::from_str(&j).expect("deserialize ToolResult");
    assert_eq!(original.success, back.success);
    assert_eq!(original.call.name, back.call.name);
}

#[test]
fn cortex_event_chat_started_serializes_with_kind_tag() {
    let e = CortexEvent::ChatStarted {
        thread_id: "t-1".into(),
    };
    let j = serde_json::to_string(&e).expect("serialize CortexEvent");
    assert!(j.contains("\"kind\":\"chat_started\""), "got: {j}");
    assert!(j.contains("\"thread_id\":\"t-1\""), "got: {j}");
}

#[test]
fn cortex_event_tool_invoked_serializes_with_kind_tag() {
    let e = CortexEvent::ToolInvoked {
        call: ToolCall {
            name: "search".into(),
            arguments: serde_json::json!({}),
        },
    };
    let j = serde_json::to_string(&e).expect("serialize CortexEvent");
    assert!(j.contains("\"kind\":\"tool_invoked\""), "got: {j}");
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<ToolCall>();
    assert_traits::<ToolResult>();
    assert_traits::<CortexEvent>();
}
