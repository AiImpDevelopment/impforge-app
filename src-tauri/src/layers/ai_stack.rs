// SPDX-License-Identifier: MIT
//! **Layer 2 — AI Stack**
//!
//! Providers, inference, routing, slash commands, cortex bus.  Depends
//! on Foundation only.  Every chat request fans out through this layer
//! so it owns provider key resolution, streaming SSE, and the cortex
//! event bus that lets modules react to chat events without coupling.
//!
//! Modules:
//! - `chat_lite`             — Multi-provider chat stream (OpenAI / Anthropic / local)
//! - `chat_session_memory`   — Per-thread message persistence
//! - `providers`             — BYOK provider registration + chat fan-out
//! - `slash_commands`        — `/help`, `/model`, etc. dispatcher
//! - `cortex_lite`           — Lightweight event bus for emergent module wiring
//! - `module_emergence_lite` — Capability registry skeleton (Phase-2 kernel)

use super::LayerDescriptor;

/// Static descriptor consumed by `layers::ALL_LAYERS`.
pub const DESCRIPTOR: LayerDescriptor = LayerDescriptor {
    id: "ai_stack",
    display_name: "AI Stack",
    purpose: "Providers, streaming inference, routing, cortex event bus.",
    modules: &[
        "chat_lite",
        "chat_session_memory",
        "cortex_lite",
        "module_emergence_lite",
        "providers",
        "slash_commands",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_id_matches_module_name() {
        assert_eq!(DESCRIPTOR.id, "ai_stack");
    }

    #[test]
    fn descriptor_modules_alphabetical() {
        let mut sorted = DESCRIPTOR.modules.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted.as_slice(), DESCRIPTOR.modules);
    }

    #[test]
    fn ai_stack_has_six_modules() {
        assert_eq!(DESCRIPTOR.modules.len(), 6);
    }
}
