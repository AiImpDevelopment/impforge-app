// SPDX-License-Identifier: MIT
//! Behavioral tests for digiimp_bridge Phase 1 type skeleton.

use impforge_app_lib::digiimp_bridge::{
    DigiImpColorPreset, DigiImpDisplayMode, DigiImpRuntimeState, DigiImpState, RestDuration,
};

#[test]
fn digiimp_state_serializes_snake_case() {
    for (s, expected) in [
        (DigiImpState::Idle, "idle"),
        (DigiImpState::Listening, "listening"),
        (DigiImpState::Thinking, "thinking"),
        (DigiImpState::Responding, "responding"),
        (DigiImpState::Error, "error"),
        (DigiImpState::Celebrating, "celebrating"),
        (DigiImpState::Sleeping, "sleeping"),
        (DigiImpState::Alerting, "alerting"),
        (DigiImpState::Acknowledging, "acknowledging"),
        (DigiImpState::Adventuring, "adventuring"),
    ] {
        let j = serde_json::to_string(&s).expect("serialize DigiImpState");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn digiimp_color_preset_serializes_kebab_case() {
    for (c, expected) in [
        (DigiImpColorPreset::NeonGreen, "neon-green"),
        (DigiImpColorPreset::Cyan, "cyan"),
        (DigiImpColorPreset::Magenta, "magenta"),
        (DigiImpColorPreset::Amber, "amber"),
    ] {
        let j = serde_json::to_string(&c).expect("serialize DigiImpColorPreset");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn digiimp_display_mode_serializes_snake_case() {
    for (m, expected) in [
        (DigiImpDisplayMode::Floating, "floating"),
        (DigiImpDisplayMode::Docked, "docked"),
        (DigiImpDisplayMode::Ambient, "ambient"),
        (DigiImpDisplayMode::Hidden, "hidden"),
    ] {
        let j = serde_json::to_string(&m).expect("serialize DigiImpDisplayMode");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn rest_duration_serializes_snake_case() {
    for (r, expected) in [
        (RestDuration::OneHour, "one_hour"),
        (RestDuration::FourHours, "four_hours"),
        (RestDuration::Today, "today"),
        (RestDuration::Forever, "forever"),
    ] {
        let j = serde_json::to_string(&r).expect("serialize RestDuration");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn digiimp_runtime_state_roundtrips() {
    let original = DigiImpRuntimeState {
        state: DigiImpState::Thinking,
        state_progress: 0.42,
        energy: 0.78,
        glow_intensity: 0.6,
        color_preset: DigiImpColorPreset::NeonGreen,
        display_mode: DigiImpDisplayMode::Floating,
    };
    let j = serde_json::to_string(&original).expect("serialize DigiImpRuntimeState");
    let back: DigiImpRuntimeState =
        serde_json::from_str(&j).expect("deserialize DigiImpRuntimeState");
    assert_eq!(original.state, back.state);
    assert_eq!(original.state_progress, back.state_progress);
    assert_eq!(original.color_preset, back.color_preset);
    assert_eq!(original.display_mode, back.display_mode);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    >() {
    }
    assert_traits::<DigiImpState>();
    assert_traits::<DigiImpColorPreset>();
    assert_traits::<DigiImpDisplayMode>();
    assert_traits::<DigiImpRuntimeState>();
    assert_traits::<RestDuration>();
}
