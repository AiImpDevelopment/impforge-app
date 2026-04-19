// SPDX-License-Identifier: MIT
//! **Layer 4 — User-Facing**
//!
//! Chat UX (HyperChat), code sandbox (Jupyter-style cells), DigiImp
//! companion bridge, widgets, universal-server adapter, upgrade
//! nudges.  Depends on every lower layer.  This is where the
//! "feature-complete free tier" experience lives.
//!
//! Modules:
//! - `hyperchat_lite`     — Mode machine + event bus + sessions
//! - `code_sandbox_lite`  — wasmtime + Pyodide + QuickJS code cells
//! - `digiimp_bridge`     — DigiImp companion-app state sync
//! - `widgets_lite`       — Floating mini-windows (Tauri webview pool)
//! - `universal_lite`     — Universal-server tool registration shim
//! - `upgrade_nudge`      — Capability-gated Pro upgrade prompts

use super::LayerDescriptor;

/// Static descriptor consumed by `layers::ALL_LAYERS`.
pub const DESCRIPTOR: LayerDescriptor = LayerDescriptor {
    id: "user_facing",
    display_name: "User-Facing",
    purpose: "Chat UX, code sandbox, DigiImp companion, widgets, upgrade nudges.",
    modules: &[
        "code_sandbox_lite",
        "digiimp_bridge",
        "hyperchat_lite",
        "universal_lite",
        "upgrade_nudge",
        "widgets_lite",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_id_matches_module_name() {
        assert_eq!(DESCRIPTOR.id, "user_facing");
    }

    #[test]
    fn descriptor_modules_alphabetical() {
        let mut sorted = DESCRIPTOR.modules.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted.as_slice(), DESCRIPTOR.modules);
    }

    #[test]
    fn user_facing_has_six_modules() {
        assert_eq!(DESCRIPTOR.modules.len(), 6);
    }
}
