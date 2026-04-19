// SPDX-License-Identifier: MIT
//! Variable inspector — surfaces guest-side globals between cells.
//!
//! ## How it works
//!
//! After every cell run, the runtime serialises the guest's global
//! namespace (Python's `globals()` minus dunders, JS's `globalThis`
//! own-keys, etc.) and stores them keyed by session_id.  The UI
//! displays the latest snapshot in the inspector panel.
//!
//! Tier-2 implementation is a simple in-memory key-value store
//! that the language runtimes feed into.  Tier-3 (Pro) extends with
//! Apache-Arrow-backed shared columns for polyglot variable access.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::types::{SessionId, Variable};

/// One immutable snapshot of a session's variables — used by the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSnapshot {
    pub session_id: SessionId,
    pub variables: Vec<Variable>,
    pub captured_unix: i64,
}

/// In-memory inspector — backs `sandbox_get_variables` /
/// `sandbox_inspect_variable`.
pub struct Inspector {
    by_session: HashMap<SessionId, HashMap<String, Variable>>,
}

impl Inspector {
    pub fn new() -> Self {
        Self {
            by_session: HashMap::new(),
        }
    }

    /// Replace the variable set for a session — called by the runtime
    /// after a cell's globals are serialised.
    pub fn put_variables(&mut self, session_id: &str, vars: Vec<Variable>) {
        let entry = self
            .by_session
            .entry(session_id.to_string())
            .or_default();
        entry.clear();
        for v in vars {
            entry.insert(v.name.clone(), v);
        }
    }

    /// Append-style update — used when a cell adds variables without
    /// clearing previous state.
    pub fn update_variables(&mut self, session_id: &str, vars: Vec<Variable>) {
        let entry = self
            .by_session
            .entry(session_id.to_string())
            .or_default();
        for v in vars {
            entry.insert(v.name.clone(), v);
        }
    }

    pub fn list_variables(&self, session_id: &str) -> Vec<Variable> {
        self.by_session
            .get(session_id)
            .map(|m| {
                let mut v: Vec<Variable> = m.values().cloned().collect();
                v.sort_by(|a, b| a.name.cmp(&b.name));
                v
            })
            .unwrap_or_default()
    }

    pub fn get_variable(&self, session_id: &str, name: &str) -> Option<Variable> {
        self.by_session.get(session_id).and_then(|m| m.get(name).cloned())
    }

    pub fn clear_session(&mut self, session_id: &str) {
        self.by_session.remove(session_id);
    }

    pub fn session_count(&self) -> usize {
        self.by_session.len()
    }
}

impl Default for Inspector {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns a global, lock-guarded inspector.  Used by command code
/// that doesn't carry an explicit reference.
pub fn global_inspector() -> &'static Mutex<Inspector> {
    static INS: OnceLock<Mutex<Inspector>> = OnceLock::new();
    INS.get_or_init(|| Mutex::new(Inspector::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_var(name: &str) -> Variable {
        Variable {
            name: name.into(),
            type_name: "int".into(),
            repr: "42".into(),
            size_bytes: 8,
            value_json: Some(serde_json::json!(42)),
        }
    }

    #[test]
    fn put_then_list_returns_alphabetical() {
        let mut i = Inspector::new();
        i.put_variables("s1", vec![fake_var("z"), fake_var("a"), fake_var("m")]);
        let v = i.list_variables("s1");
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].name, "a");
        assert_eq!(v[1].name, "m");
        assert_eq!(v[2].name, "z");
    }

    #[test]
    fn put_replaces_previous_set() {
        let mut i = Inspector::new();
        i.put_variables("s1", vec![fake_var("x"), fake_var("y")]);
        i.put_variables("s1", vec![fake_var("z")]);
        let v = i.list_variables("s1");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "z");
    }

    #[test]
    fn update_appends_without_clearing() {
        let mut i = Inspector::new();
        i.put_variables("s1", vec![fake_var("x")]);
        i.update_variables("s1", vec![fake_var("y")]);
        assert_eq!(i.list_variables("s1").len(), 2);
    }

    #[test]
    fn get_variable_returns_match() {
        let mut i = Inspector::new();
        i.put_variables("s1", vec![fake_var("foo")]);
        assert!(i.get_variable("s1", "foo").is_some());
        assert!(i.get_variable("s1", "bar").is_none());
    }

    #[test]
    fn clear_session_removes_all() {
        let mut i = Inspector::new();
        i.put_variables("s1", vec![fake_var("x")]);
        i.put_variables("s2", vec![fake_var("y")]);
        i.clear_session("s1");
        assert_eq!(i.list_variables("s1").len(), 0);
        assert_eq!(i.list_variables("s2").len(), 1);
    }

    #[test]
    fn unknown_session_returns_empty() {
        let i = Inspector::new();
        assert!(i.list_variables("never").is_empty());
        assert!(i.get_variable("never", "x").is_none());
    }

    #[test]
    fn session_count_tracks_active_sessions() {
        let mut i = Inspector::new();
        assert_eq!(i.session_count(), 0);
        i.put_variables("s1", vec![fake_var("x")]);
        i.put_variables("s2", vec![fake_var("x")]);
        assert_eq!(i.session_count(), 2);
        i.clear_session("s1");
        assert_eq!(i.session_count(), 1);
    }

    #[test]
    fn global_inspector_is_lazy_singleton() {
        let g1 = global_inspector();
        let g2 = global_inspector();
        let p1 = g1 as *const _;
        let p2 = g2 as *const _;
        assert_eq!(p1, p2, "should be same singleton");
    }
}
