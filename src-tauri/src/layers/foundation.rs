// SPDX-License-Identifier: MIT
//! **Layer 1 — Foundation**
//!
//! Privacy, security, compliance, crypto.  Zero in-crate dependencies.
//! Every other layer rests on this one, so its surface MUST stay
//! deterministic, side-effect-free where possible, and crash-free
//! under hostile input.
//!
//! Modules:
//! - `digu_privacy_full` — DiGu (German Digital Markets Act) policy engine
//! - `eu_ai_act_full`    — EU AI Act risk-classification + decision log
//! - `pii_scrubber`      — PII detection + redaction (NER + regex)
//! - `injection_firewall`— Prompt-injection scan + sanitisation
//! - `crypto_lite`       — AES-256-GCM helpers + Argon2id KDF
//! - `keys`              — BYOK keystore (file-encrypted)
//! - `spend`             — Per-provider $ ledger + caps
//! - `feature_flags`     — Runtime toggles (free vs. teaser vs. pro)

use super::LayerDescriptor;

/// Static descriptor consumed by `layers::ALL_LAYERS`.
pub const DESCRIPTOR: LayerDescriptor = LayerDescriptor {
    id: "foundation",
    display_name: "Foundation",
    purpose: "Privacy, security, compliance, crypto — the bedrock every other layer trusts.",
    modules: &[
        "crypto_lite",
        "digu_privacy_full",
        "eu_ai_act_full",
        "feature_flags",
        "injection_firewall",
        "keys",
        "pii_scrubber",
        "spend",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_id_matches_module_name() {
        assert_eq!(DESCRIPTOR.id, "foundation");
    }

    #[test]
    fn descriptor_modules_alphabetical() {
        let mut sorted = DESCRIPTOR.modules.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            sorted.as_slice(),
            DESCRIPTOR.modules,
            "modules must stay alphabetical for deterministic diffs"
        );
    }

    #[test]
    fn foundation_has_eight_modules() {
        assert_eq!(DESCRIPTOR.modules.len(), 8);
    }
}
