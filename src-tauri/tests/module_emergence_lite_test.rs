// SPDX-License-Identifier: MIT
//! Behavioral tests for module_emergence_lite Phase 1 type skeleton.

use impforge_app_lib::module_emergence_lite::{
    Capability, EmergenceContext, IpcRequest, IpcResponse, ModuleId,
};

#[test]
fn module_id_roundtrips_through_json() {
    let original = ModuleId("chat_lite".into());
    let j = serde_json::to_string(&original).expect("serialize ModuleId");
    let back: ModuleId = serde_json::from_str(&j).expect("deserialize ModuleId");
    assert_eq!(original.0, back.0);
}

#[test]
fn capability_roundtrips_through_json() {
    let original = Capability {
        name: "chat.send".into(),
        schema: serde_json::json!({"type": "object"}),
    };
    let j = serde_json::to_string(&original).expect("serialize Capability");
    let back: Capability = serde_json::from_str(&j).expect("deserialize Capability");
    assert_eq!(original.name, back.name);
}

#[test]
fn emergence_context_roundtrips() {
    let original = EmergenceContext {
        module: ModuleId("chat_lite".into()),
        capabilities: vec![Capability {
            name: "chat.send".into(),
            schema: serde_json::json!({}),
        }],
    };
    let j = serde_json::to_string(&original).expect("serialize EmergenceContext");
    let back: EmergenceContext = serde_json::from_str(&j).expect("deserialize EmergenceContext");
    assert_eq!(original.module.0, back.module.0);
    assert_eq!(original.capabilities.len(), back.capabilities.len());
}

#[test]
fn ipc_request_roundtrips() {
    let original = IpcRequest {
        from: ModuleId("chat_lite".into()),
        to: ModuleId("knowledge_lite".into()),
        capability: "knowledge.search".into(),
        payload: serde_json::json!({"q": "rust"}),
    };
    let j = serde_json::to_string(&original).expect("serialize IpcRequest");
    let back: IpcRequest = serde_json::from_str(&j).expect("deserialize IpcRequest");
    assert_eq!(original.capability, back.capability);
}

#[test]
fn ipc_response_roundtrips() {
    let original = IpcResponse {
        success: true,
        payload: serde_json::json!({"ok": true}),
    };
    let j = serde_json::to_string(&original).expect("serialize IpcResponse");
    let back: IpcResponse = serde_json::from_str(&j).expect("deserialize IpcResponse");
    assert_eq!(original.success, back.success);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<ModuleId>();
    assert_traits::<Capability>();
    assert_traits::<EmergenceContext>();
    assert_traits::<IpcRequest>();
    assert_traits::<IpcResponse>();
}
