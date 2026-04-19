// SPDX-License-Identifier: MIT
//! Crown-Jewel **5-Layer Emergent Registry** — Phase 2 of the
//! `lib.rs` orchestrator refactor.
//!
//! ## Why 5 layers?
//!
//! The App holds 33 domain modules.  A flat list scales badly (humans
//! lose track around 8 items, [Miller's law](https://en.wikipedia.org/wiki/The_Magical_Number_Seven,_Plus_or_Minus_Two))
//! and ad-hoc section comments don't enforce the architectural shape.
//! The 5-layer model below is **memory-informed routing** for humans:
//! each layer has a single purpose and a clear dependency direction
//! (Foundation -> AI Stack -> Knowledge -> User-Facing -> Enterprise).
//!
//! ## Emergent micro-program declaration
//!
//! `module_emergence_lite` is a Phase-1 skeleton (returns `Internal`
//! errors) so we don't *call* it at startup — that would crash on
//! launch.  Instead, every layer defines a [`LayerDescriptor`] that
//! the future Phase-2 emergence kernel can iterate over to register
//! capabilities en masse.  This keeps the registry **declarative**
//! and crash-free until the kernel comes online.
//!
//! ## Tauri macro constraint
//!
//! `tauri::generate_handler!` requires literal command paths — you
//! cannot iterate or splice a `Vec` in there.  So `lib.rs` keeps a
//! single `generate_handler!` block, *grouped* by these 5 layers
//! using section comments, and this module documents the membership.
//!
//! Run `cargo test --lib layers::` to verify every module ends up in
//! exactly one layer (no duplicates, no orphans).

pub mod ai_stack;
pub mod enterprise;
pub mod foundation;
pub mod knowledge;
pub mod user_facing;

/// Static descriptor for one architectural layer.
///
/// Stored in `static` consts so iteration is allocation-free and the
/// emergence kernel (Phase 2) can register capabilities at startup
/// without heap traffic.
#[derive(Debug, Clone, Copy)]
pub struct LayerDescriptor {
    /// Stable layer identifier (`"foundation"`, `"ai_stack"`, ...).
    pub id: &'static str,
    /// Human-readable layer name shown in dashboard / docs.
    pub display_name: &'static str,
    /// One-sentence purpose statement — drives upgrade-nudge copy.
    pub purpose: &'static str,
    /// Module paths that belong to this layer (relative to `crate::`).
    /// Order is alphabetical for deterministic diffs.
    pub modules: &'static [&'static str],
}

/// Every layer descriptor in dependency order.
///
/// Read this from top to bottom: lower layers MUST NOT depend on
/// higher layers.  Foundation has zero in-crate dependencies; AI
/// Stack depends only on Foundation; Knowledge depends on Foundation
/// plus AI Stack; User-Facing depends on all three lower layers;
/// Enterprise wraps everything for distribution / Pro hand-off.
pub const ALL_LAYERS: &[LayerDescriptor] = &[
    foundation::DESCRIPTOR,
    ai_stack::DESCRIPTOR,
    knowledge::DESCRIPTOR,
    user_facing::DESCRIPTOR,
    enterprise::DESCRIPTOR,
];

/// Find which layer a module belongs to.  Returns the layer `id`
/// or `None` if the module is missing from the registry (a bug).
pub fn layer_of(module_path: &str) -> Option<&'static str> {
    for layer in ALL_LAYERS {
        if layer.modules.contains(&module_path) {
            return Some(layer.id);
        }
    }
    None
}

/// Total count of modules across every layer — sanity check for the
/// `every_module_in_exactly_one_layer` test.
pub fn total_module_count() -> usize {
    ALL_LAYERS.iter().map(|l| l.modules.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn five_layers_present() {
        assert_eq!(ALL_LAYERS.len(), 5, "exactly 5 architectural layers");
    }

    #[test]
    fn layer_ids_unique() {
        let ids: HashSet<&str> = ALL_LAYERS.iter().map(|l| l.id).collect();
        assert_eq!(ids.len(), ALL_LAYERS.len(), "layer ids must be unique");
    }

    #[test]
    fn no_module_in_two_layers() {
        let mut seen: HashSet<&str> = HashSet::new();
        for layer in ALL_LAYERS {
            for m in layer.modules {
                assert!(
                    seen.insert(m),
                    "module {m} appears in more than one layer"
                );
            }
        }
    }

    #[test]
    fn layer_of_lookup_works() {
        // foundation
        assert_eq!(layer_of("crypto_lite"), Some("foundation"));
        assert_eq!(layer_of("digu_privacy_full"), Some("foundation"));
        // ai_stack
        assert_eq!(layer_of("chat_lite"), Some("ai_stack"));
        // knowledge
        assert_eq!(layer_of("knowledge_lite"), Some("knowledge"));
        assert_eq!(layer_of("auto_digest_lite"), Some("knowledge"));
        // user_facing
        assert_eq!(layer_of("hyperchat_lite"), Some("user_facing"));
        // enterprise
        assert_eq!(layer_of("mcp"), Some("enterprise"));
        // not registered
        assert_eq!(layer_of("nonexistent_module"), None);
    }

    #[test]
    fn module_count_matches_lib_rs_declarations() {
        // 32 declared `pub mod`s in lib.rs minus `error` (infra) and
        // `layers` (this module) leaves exactly 30 domain modules
        // across the 5 layers (8 + 6 + 9 + 6 + 1).  If you add or
        // remove a `pub mod` in lib.rs, update the matching layer
        // descriptor.
        assert_eq!(total_module_count(), 30, "30 domain modules across 5 layers");
    }
}
