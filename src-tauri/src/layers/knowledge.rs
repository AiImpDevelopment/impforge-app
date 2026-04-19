// SPDX-License-Identifier: MIT
//! **Layer 3 — Knowledge**
//!
//! RAG (retrieval-augmented generation), persistent memory, auto-digest
//! pipelines, document parsing.  Depends on Foundation + AI Stack.
//! This is where the App turns user input + ambient sources (folders,
//! feeds, screenshots, browser history) into a searchable corpus.
//!
//! Modules:
//! - `knowledge_lite`         — Dual-table FTS5 + RRF hybrid search core
//! - `memory_lite`            — Transient session memory
//! - `auto_digest_lite`       — RSS + folder watcher + scheduler
//! - `digest_clipboard`       — Opt-in clipboard capture
//! - `digest_screenshots`     — Opt-in screenshot OCR pipeline
//! - `digest_browser`         — Opt-in browser bookmarks/history import
//! - `browser_import_oneshot` — Manual browser-import command
//! - `document_parse`         — PDF / DOCX / XLSX / HTML / MD extractors
//! - `wikipedia_fetch`        — Wikipedia search + article fetch

use super::LayerDescriptor;

/// Static descriptor consumed by `layers::ALL_LAYERS`.
pub const DESCRIPTOR: LayerDescriptor = LayerDescriptor {
    id: "knowledge",
    display_name: "Knowledge",
    purpose: "RAG core, memory, ambient digest pipelines, document parsing.",
    modules: &[
        "auto_digest_lite",
        "browser_import_oneshot",
        "digest_browser",
        "digest_clipboard",
        "digest_screenshots",
        "document_parse",
        "knowledge_lite",
        "memory_lite",
        "wikipedia_fetch",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_id_matches_module_name() {
        assert_eq!(DESCRIPTOR.id, "knowledge");
    }

    #[test]
    fn descriptor_modules_alphabetical() {
        let mut sorted = DESCRIPTOR.modules.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted.as_slice(), DESCRIPTOR.modules);
    }

    #[test]
    fn knowledge_has_nine_modules() {
        assert_eq!(DESCRIPTOR.modules.len(), 9);
    }
}
