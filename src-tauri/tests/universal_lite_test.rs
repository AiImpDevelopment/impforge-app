// SPDX-License-Identifier: MIT
//! Behavioral tests for universal_lite Phase 1 type skeleton.

use impforge_app_lib::universal_lite::{
    ConsumerProtocol, ToolInvocation, ToolOutput, ToolSchema,
};

#[test]
fn consumer_protocol_serializes_snake_case() {
    for (p, expected) in [
        (ConsumerProtocol::Mcp, "mcp"),
        (ConsumerProtocol::Openai, "openai"),
        (ConsumerProtocol::ReAct, "re_act"),
        (ConsumerProtocol::Gbnf, "gbnf"),
    ] {
        let j = serde_json::to_string(&p).expect("serialize ConsumerProtocol");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn tool_schema_roundtrips_through_json() {
    let original = ToolSchema {
        name: "search".into(),
        description: "Run a query".into(),
        parameters: serde_json::json!({"type": "object"}),
    };
    let j = serde_json::to_string(&original).expect("serialize ToolSchema");
    let back: ToolSchema = serde_json::from_str(&j).expect("deserialize ToolSchema");
    assert_eq!(original.name, back.name);
    assert_eq!(original.description, back.description);
}

#[test]
fn tool_invocation_roundtrips() {
    let original = ToolInvocation {
        protocol: ConsumerProtocol::Mcp,
        tool: "search".into(),
        arguments: serde_json::json!({"q": "rust"}),
    };
    let j = serde_json::to_string(&original).expect("serialize ToolInvocation");
    let back: ToolInvocation = serde_json::from_str(&j).expect("deserialize ToolInvocation");
    assert_eq!(original.tool, back.tool);
    assert_eq!(original.protocol, back.protocol);
}

#[test]
fn tool_output_roundtrips() {
    let original = ToolOutput {
        success: true,
        result: serde_json::json!({"hits": []}),
    };
    let j = serde_json::to_string(&original).expect("serialize ToolOutput");
    let back: ToolOutput = serde_json::from_str(&j).expect("deserialize ToolOutput");
    assert_eq!(original.success, back.success);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<ConsumerProtocol>();
    assert_traits::<ToolSchema>();
    assert_traits::<ToolInvocation>();
    assert_traits::<ToolOutput>();
}
