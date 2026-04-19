// SPDX-License-Identifier: MIT
//! **Layer 5 — Enterprise**
//!
//! MCP marketplace + future Pro hooks.  This layer is the "expand
//! upward" surface — anything destined for the impforge-pro hand-off
//! lives here so the migration is a clean delete-from-`AiImp` /
//! lift-into-`Pro` operation.
//!
//! Modules:
//! - `mcp` — MCP marketplace + runner + health + ledger + bundle

use super::LayerDescriptor;

/// Static descriptor consumed by `layers::ALL_LAYERS`.
pub const DESCRIPTOR: LayerDescriptor = LayerDescriptor {
    id: "enterprise",
    display_name: "Enterprise",
    purpose: "MCP marketplace + Pro hand-off staging area.",
    modules: &["mcp"],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_id_matches_module_name() {
        assert_eq!(DESCRIPTOR.id, "enterprise");
    }

    #[test]
    fn enterprise_has_one_module() {
        assert_eq!(DESCRIPTOR.modules.len(), 1);
    }

    #[test]
    fn enterprise_owns_mcp() {
        assert_eq!(DESCRIPTOR.modules[0], "mcp");
    }
}
