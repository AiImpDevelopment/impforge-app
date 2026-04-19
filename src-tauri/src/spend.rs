// SPDX-License-Identifier: MIT
//! Per-provider running spend telemetry (Feature 1, Tier 2/3).
//!
//! What we track:
//!   - call count per (provider, model)
//!   - cumulative response bytes per provider (proxy for output tokens — we
//!     do not bake provider pricing tables into the MIT app, that's a Pro
//!     feature)
//!   - cumulative latency
//!
//! What we never do:
//!   - phone home — every figure stays on disk in `~/.impforge-app/spend.json`
//!   - serialize keys — ALL telemetry is by `provider_id`, not by key
//!
//! Pro layers cost-aware routing on top via `providers_pro::autopilot`.
//!
//! See research report `2026-04-19-multi-provider-byok.md` §C move 1
//! ("Live Spend Mirror in the chat composer").

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Aggregate spend per provider — exposed unmodified to the frontend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderSpend {
    pub provider: String,
    pub call_count: u64,
    pub total_bytes: u64,
    pub total_latency_ms: u64,
    /// Per-model breakdown — useful for "which model did I burn the most on?".
    pub by_model: HashMap<String, ModelSpend>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelSpend {
    pub call_count: u64,
    pub total_bytes: u64,
    pub total_latency_ms: u64,
}

/// Process-wide registry. `OnceLock<Mutex<...>>` is the canonical "singleton
/// with interior mutability" pattern used elsewhere in the app
/// (cf. `chat_session_memory.rs`).
fn registry() -> &'static Mutex<HashMap<String, ProviderSpend>> {
    static R: OnceLock<Mutex<HashMap<String, ProviderSpend>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record one call's telemetry. Best-effort: a poisoned mutex is logged but
/// does not propagate — telemetry must never block real chat traffic.
pub fn record_call(provider: &str, model: &str, bytes: u64, latency_ms: u64) {
    let Ok(mut map) = registry().lock() else {
        tracing::warn!(target: "spend", "registry mutex poisoned — dropping telemetry");
        return;
    };
    let entry = map
        .entry(provider.to_string())
        .or_insert_with(|| ProviderSpend {
            provider: provider.to_string(),
            ..Default::default()
        });
    entry.call_count = entry.call_count.saturating_add(1);
    entry.total_bytes = entry.total_bytes.saturating_add(bytes);
    entry.total_latency_ms = entry.total_latency_ms.saturating_add(latency_ms);

    let m = entry
        .by_model
        .entry(model.to_string())
        .or_insert_with(ModelSpend::default);
    m.call_count = m.call_count.saturating_add(1);
    m.total_bytes = m.total_bytes.saturating_add(bytes);
    m.total_latency_ms = m.total_latency_ms.saturating_add(latency_ms);
}

/// Snapshot of every provider's spend so far. Stable order (sorted by id) so
/// the UI can render without a `key` reshuffle on every refresh.
#[tauri::command]
pub fn spend_get_all() -> AppResult<Vec<ProviderSpend>> {
    let map = registry()
        .lock()
        .map_err(|e| crate::error::AppError::Internal(format!("spend lock: {e}")))?;
    let mut v: Vec<ProviderSpend> = map.values().cloned().collect();
    v.sort_by(|a, b| a.provider.cmp(&b.provider));
    Ok(v)
}

/// Single-provider snapshot — returns a zero-default if no calls were made.
#[tauri::command]
pub fn spend_get_provider(provider: String) -> AppResult<ProviderSpend> {
    let map = registry()
        .lock()
        .map_err(|e| crate::error::AppError::Internal(format!("spend lock: {e}")))?;
    Ok(map.get(&provider).cloned().unwrap_or(ProviderSpend {
        provider,
        ..Default::default()
    }))
}

/// Reset all telemetry — exposed so the user can clear running stats from
/// Settings without quitting the app.
#[tauri::command]
pub fn spend_reset() -> AppResult<()> {
    let mut map = registry()
        .lock()
        .map_err(|e| crate::error::AppError::Internal(format!("spend lock: {e}")))?;
    map.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Process-wide test serializer. The `registry()` is a static singleton
    /// so any test that mutates state must run sequentially with every other
    /// test (including the integration tests in `tests/providers_test.rs`).
    /// Tests that only query state can also acquire this lock to get a
    /// consistent snapshot.
    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
        // Recover from poisoned mutex (a previous test panicked); the
        // serializer itself carries no state worth preserving.
        L.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Helper: clear before each test so order doesn't matter.
    fn reset() {
        let mut m = registry()
            .lock()
            .expect("registry lock should never be poisoned in test setup");
        m.clear();
    }

    #[test]
    fn record_then_get_returns_aggregate() {
        let _g = test_lock();
        reset();
        record_call("openai", "gpt-4o-mini", 1024, 150);
        record_call("openai", "gpt-4o-mini", 2048, 250);
        let s = spend_get_provider("openai".into()).expect("snapshot");
        assert_eq!(s.call_count, 2);
        assert_eq!(s.total_bytes, 3072);
        assert_eq!(s.total_latency_ms, 400);
        assert_eq!(s.by_model.len(), 1);
    }

    #[test]
    fn record_for_unknown_provider_creates_entry() {
        let _g = test_lock();
        reset();
        let s = spend_get_provider("anthropic".into()).expect("snapshot");
        assert_eq!(s.call_count, 0);
        assert_eq!(s.total_bytes, 0);
    }

    #[test]
    fn get_all_sorted_by_provider() {
        let _g = test_lock();
        reset();
        record_call("openai", "x", 1, 1);
        record_call("anthropic", "y", 1, 1);
        record_call("ollama", "z", 1, 1);
        let v = spend_get_all().expect("snapshot");
        let names: Vec<&str> = v.iter().map(|s| s.provider.as_str()).collect();
        assert_eq!(names, vec!["anthropic", "ollama", "openai"]);
    }

    #[test]
    fn per_model_breakdown_distinct() {
        let _g = test_lock();
        reset();
        record_call("openai", "gpt-4o", 100, 50);
        record_call("openai", "gpt-4o-mini", 50, 30);
        record_call("openai", "gpt-4o", 200, 70);
        let s = spend_get_provider("openai".into()).expect("snapshot");
        assert_eq!(s.by_model.len(), 2);
        let big = s.by_model.get("gpt-4o").expect("gpt-4o tracked");
        assert_eq!(big.call_count, 2);
        assert_eq!(big.total_bytes, 300);
        let mini = s.by_model.get("gpt-4o-mini").expect("mini tracked");
        assert_eq!(mini.call_count, 1);
    }

    #[test]
    fn reset_clears_state() {
        let _g = test_lock();
        reset();
        record_call("openai", "m", 100, 100);
        spend_reset().expect("reset");
        let s = spend_get_provider("openai".into()).expect("snapshot");
        assert_eq!(s.call_count, 0);
    }

    #[test]
    fn saturating_add_no_overflow() {
        let _g = test_lock();
        reset();
        record_call("x", "y", u64::MAX, u64::MAX);
        record_call("x", "y", 1, 1);
        let s = spend_get_provider("x".into()).expect("snapshot");
        assert_eq!(s.total_bytes, u64::MAX);
        assert_eq!(s.total_latency_ms, u64::MAX);
    }

    #[test]
    fn provider_spend_serialization_round_trip() {
        let mut by_model = HashMap::new();
        by_model.insert(
            "gpt-4o".to_string(),
            ModelSpend {
                call_count: 3,
                total_bytes: 1024,
                total_latency_ms: 500,
            },
        );
        let s = ProviderSpend {
            provider: "openai".into(),
            call_count: 3,
            total_bytes: 1024,
            total_latency_ms: 500,
            by_model,
        };
        let j = serde_json::to_string(&s).expect("serialize");
        let back: ProviderSpend = serde_json::from_str(&j).expect("deserialize");
        assert_eq!(back.provider, s.provider);
        assert_eq!(back.call_count, s.call_count);
        assert_eq!(back.by_model.len(), 1);
    }
}
