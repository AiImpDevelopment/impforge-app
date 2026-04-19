// SPDX-License-Identifier: MIT
//! Integration tests for the multi-provider BYOK backend (Feature 1, Tier 2).
//!
//! These run from the workspace test target and exercise the public lib
//! surface only. Behavioural keychain tests are guarded with an env-var
//! escape hatch so CI on macOS/Windows runners does not fail when the
//! Secret Service is unavailable.

use impforge_app_lib::keys::{self, KeyStatus, SecretKey, KEYRING_SERVICE};
use impforge_app_lib::providers::{Provider, ProviderInfo};
use impforge_app_lib::spend::{self, ProviderSpend};

/// Per-process test serializer — the spend registry is a static singleton,
/// so spend tests in this binary must run sequentially with each other.
fn spend_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn provider_enum_serializes_lowercase() {
    let j = serde_json::to_string(&Provider::OpenRouter).expect("serialize OpenRouter");
    assert_eq!(j, "\"openrouter\"");
    let j = serde_json::to_string(&Provider::Anthropic).expect("serialize Anthropic");
    assert_eq!(j, "\"anthropic\"");
}

#[test]
fn provider_info_carries_no_secret() {
    let info = ProviderInfo {
        id: Provider::OpenAi,
        display_name: "OpenAI".into(),
        default_model: "gpt-4o-mini".into(),
        requires_key: true,
        key_status: KeyStatus::Set,
    };
    let j = serde_json::to_string(&info).expect("serialize ProviderInfo");
    // The serialized payload MUST NOT contain anything that looks like a key.
    assert!(!j.contains("sk-"));
    assert!(!j.contains("api"));
    assert!(j.contains("\"openai\""));
    assert!(j.contains("\"set\""));
}

#[test]
fn provider_round_trip_through_parse() {
    for &p in Provider::all() {
        assert_eq!(Provider::parse(p.as_str()), Some(p));
    }
}

#[test]
fn keyring_service_constant_is_app_specific() {
    assert_eq!(KEYRING_SERVICE, "impforge-app");
}

#[test]
fn secret_key_debug_never_leaks() {
    let s = SecretKey::new("sk-abcdef123456");
    let s_dbg = format!("{:?}", s);
    assert!(!s_dbg.contains("sk-"));
    assert!(!s_dbg.contains("abcdef"));
    assert!(s_dbg.contains("redacted"));
}

#[test]
fn spend_record_then_snapshot() {
    let _g = spend_test_lock();
    spend::spend_reset().expect("reset");
    spend::record_call("openai", "gpt-4o-mini", 512, 100);
    spend::record_call("openai", "gpt-4o-mini", 256, 50);
    spend::record_call("anthropic", "claude-3-haiku", 1024, 200);

    let all = spend::spend_get_all().expect("get_all");
    assert_eq!(all.len(), 2);
    let openai: &ProviderSpend = all
        .iter()
        .find(|s| s.provider == "openai")
        .expect("openai entry");
    assert_eq!(openai.call_count, 2);
    assert_eq!(openai.total_bytes, 768);
}

#[test]
fn spend_get_provider_returns_zero_default_when_unknown() {
    let _g = spend_test_lock();
    spend::spend_reset().expect("reset");
    let s = spend::spend_get_provider("never-called".into()).expect("snapshot");
    assert_eq!(s.call_count, 0);
    assert_eq!(s.total_bytes, 0);
}

/// Behavioural round-trip — only runs when `IMPFORGE_KEYRING_TEST=1`.
/// CI runners without an OS keychain (containerless macOS/Windows) skip this.
#[test]
fn keyring_round_trip_save_get_delete() {
    if std::env::var("IMPFORGE_KEYRING_TEST").ok().as_deref() != Some("1") {
        eprintln!("skipping keychain round-trip (set IMPFORGE_KEYRING_TEST=1 to run)");
        return;
    }
    let provider_id = "test-provider-roundtrip";
    keys::save_key(provider_id, "sk-test-roundtrip-value").expect("save");
    let got = keys::get_key(provider_id).expect("get");
    let got = got.expect("key was just saved");
    assert_eq!(got.expose(), "sk-test-roundtrip-value");
    assert!(keys::has_key(provider_id).expect("has_key"));
    keys::delete_key(provider_id).expect("delete");
    assert!(!keys::has_key(provider_id).expect("has_key after delete"));
}
