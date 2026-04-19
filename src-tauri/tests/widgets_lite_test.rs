// SPDX-License-Identifier: MIT
//! Behavioral tests for widgets_lite Phase 1 type skeleton.

use impforge_app_lib::widgets_lite::{WidgetKind, WidgetPowerMode, WidgetState};

#[test]
fn widget_kind_serializes_snake_case() {
    for (k, expected) in [
        (WidgetKind::Clock, "clock"),
        (WidgetKind::Notes, "notes"),
        (WidgetKind::Monitor, "monitor"),
    ] {
        let j = serde_json::to_string(&k).expect("serialize WidgetKind");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn widget_power_mode_serializes_snake_case() {
    for (m, expected) in [
        (WidgetPowerMode::DeepSleep, "deep_sleep"),
        (WidgetPowerMode::Idle, "idle"),
        (WidgetPowerMode::Active, "active"),
    ] {
        let j = serde_json::to_string(&m).expect("serialize WidgetPowerMode");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn widget_state_roundtrips() {
    let original = WidgetState {
        id: "w-1".into(),
        kind: WidgetKind::Clock,
        power_mode: WidgetPowerMode::Active,
        bytes_used: 4_096,
    };
    let j = serde_json::to_string(&original).expect("serialize WidgetState");
    let back: WidgetState = serde_json::from_str(&j).expect("deserialize WidgetState");
    assert_eq!(original.id, back.id);
    assert_eq!(original.kind, back.kind);
    assert_eq!(original.power_mode, back.power_mode);
    assert_eq!(original.bytes_used, back.bytes_used);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<WidgetKind>();
    assert_traits::<WidgetPowerMode>();
    assert_traits::<WidgetState>();
}
