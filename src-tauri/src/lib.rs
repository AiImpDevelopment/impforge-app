// SPDX-License-Identifier: MIT
//! ImpForge MIT freemium app — Tauri 2.10 + Svelte 5 backend.
//!
//! ## Crown-Jewel module map
//!
//! Every domain lives in its own module; large modules use the
//! [`commands.rs` sub-split pattern](docs/superpowers/specs) to keep
//! the wire surface separate from business logic.  This file is a
//! pure orchestrator: module declarations + Tauri builder + a single
//! grouped handler-registration block organised by the **5-layer
//! architecture** documented in [`crate::layers`].
//!
//! ### 5 architectural layers (see `layers/` for descriptors + tests)
//!
//! 1. **Foundation** — privacy, security, compliance, crypto
//! 2. **AI Stack** — providers, inference, routing, cortex bus
//! 3. **Knowledge** — RAG, memory, auto-digest, document parse
//! 4. **User-Facing** — chat UX, sandbox, DigiImp, widgets, nudges
//! 5. **Enterprise** — MCP marketplace + Pro hand-off staging
//!
//! New commands MUST be registered in this file under the matching
//! layer section.  Frontend bindings are emitted from the same names,
//! so renames are breaking changes.

pub mod error;
pub mod layers;

// ─── Layer 1: Foundation ─────────────────────────────────────────────────
pub mod crypto_lite;
pub mod digu_privacy_full;
pub mod eu_ai_act_full;
pub mod feature_flags;
pub mod injection_firewall;
pub mod keys;
pub mod pii_scrubber;
pub mod spend;

// ─── Layer 2: AI Stack ───────────────────────────────────────────────────
pub mod chat_lite;
pub mod chat_session_memory;
pub mod cortex_lite;
pub mod module_emergence_lite;
pub mod providers;
pub mod slash_commands;

// ─── Layer 3: Knowledge ──────────────────────────────────────────────────
pub mod auto_digest_lite;
pub mod browser_import_oneshot;
pub mod digest_browser;
pub mod digest_clipboard;
pub mod digest_screenshots;
pub mod document_parse;
pub mod knowledge_lite;
pub mod memory_lite;
pub mod wikipedia_fetch;

// ─── Layer 4: User-Facing ────────────────────────────────────────────────
pub mod code_sandbox_lite;
pub mod digiimp_bridge;
pub mod hyperchat_lite;
pub mod universal_lite;
pub mod upgrade_nudge;
pub mod widgets_lite;

// ─── Layer 5: Enterprise ─────────────────────────────────────────────────
pub mod mcp;

use error::AppResult;

/// Liveness probe — returns `"pong"`.  Frontend uses this on first
/// load to confirm the Tauri runtime is alive.  Stays in `lib.rs` on
/// purpose: it's the only command without a domain home.
#[tauri::command]
fn ping() -> AppResult<String> {
    Ok("pong".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "impforge_app=info,warn".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            // ╔═══════════════════════════════════════════════════════════╗
            // ║ Health                                                    ║
            // ╚═══════════════════════════════════════════════════════════╝
            ping,
            // ╔═══════════════════════════════════════════════════════════╗
            // ║ LAYER 1 — FOUNDATION                                      ║
            // ║ Privacy, security, compliance, crypto                     ║
            // ╚═══════════════════════════════════════════════════════════╝
            // Privacy & compliance (DiGu + EU AI Act + PII)
            digu_privacy_full::privacy_get_tier,
            digu_privacy_full::privacy_set_tier,
            digu_privacy_full::privacy_get_policy,
            digu_privacy_full::privacy_check_operation,
            eu_ai_act_full::eu_ai_classify_risk,
            eu_ai_act_full::eu_ai_log_decision,
            eu_ai_act_full::eu_ai_export_report,
            pii_scrubber::pii_detect,
            pii_scrubber::pii_scrub,
            // Security (injection firewall + crypto)
            injection_firewall::injection_scan,
            injection_firewall::injection_sanitize,
            crypto_lite::crypto_encrypt,
            crypto_lite::crypto_decrypt,
            crypto_lite::crypto_derive_key,
            // Spend ledger
            spend::spend_get_all,
            spend::spend_get_provider,
            spend::spend_reset,
            // Feature flags
            feature_flags::feature_flag_list,
            feature_flags::feature_flag_set,
            feature_flags::feature_flag_stats,
            feature_flags::feature_flag_reset_defaults,
            // ╔═══════════════════════════════════════════════════════════╗
            // ║ LAYER 2 — AI STACK                                        ║
            // ║ Providers, inference, routing, cortex bus                 ║
            // ╚═══════════════════════════════════════════════════════════╝
            // Chat (multi-provider stream + session memory)
            chat_lite::chat_stream,
            chat_session_memory::session_create_thread,
            chat_session_memory::session_append_message,
            chat_session_memory::session_list_threads,
            chat_session_memory::session_get_messages,
            // Slash commands
            slash_commands::slash_catalog,
            slash_commands::slash_dispatch,
            // Providers (BYOK keys + chat stream)
            providers::provider_list,
            providers::provider_add,
            providers::provider_remove,
            providers::provider_chat_stream,
            // Cortex + Emergence (event bus + capability registry)
            cortex_lite::cortex_invoke_tool,
            cortex_lite::cortex_publish_event,
            module_emergence_lite::emergence_register,
            module_emergence_lite::emergence_ask,
            module_emergence_lite::emergence_has_capability,
            // ╔═══════════════════════════════════════════════════════════╗
            // ║ LAYER 3 — KNOWLEDGE                                       ║
            // ║ RAG, memory, auto-digest, document parse                  ║
            // ╚═══════════════════════════════════════════════════════════╝
            // Knowledge (RAG: ingest + search + cite)
            knowledge_lite::knowledge_insert,
            knowledge_lite::knowledge_search,
            knowledge_lite::knowledge_count,
            knowledge_lite::knowledge_ingest_path,
            knowledge_lite::knowledge_ingest_dir,
            knowledge_lite::knowledge_delete_doc,
            knowledge_lite::knowledge_get_citation,
            knowledge_lite::knowledge_stats,
            knowledge_lite::knowledge_pro_teaser_count,
            // Memory (transient session memory)
            memory_lite::memory_store,
            memory_lite::memory_search,
            memory_lite::memory_recall_recent,
            // Auto-digest (RSS + folders + screenshots + browser)
            auto_digest_lite::commands::digest_add_source,
            auto_digest_lite::commands::digest_remove_source,
            auto_digest_lite::commands::digest_list_sources,
            auto_digest_lite::commands::digest_run_once,
            auto_digest_lite::commands::digest_history,
            auto_digest_lite::commands::digest_pause,
            auto_digest_lite::commands::digest_resume,
            auto_digest_lite::commands::digest_stats,
            auto_digest_lite::commands::digest_set_quiet_hours,
            digest_clipboard::digest_clipboard_enable,
            digest_clipboard::digest_clipboard_disable,
            digest_clipboard::digest_clipboard_status,
            digest_screenshots::digest_screenshots_enable,
            digest_screenshots::digest_screenshots_disable,
            digest_screenshots::digest_screenshots_status,
            digest_screenshots::digest_screenshots_default_folder,
            digest_browser::digest_browser_profiles,
            digest_browser::digest_browser_import_bookmarks,
            digest_browser::digest_browser_import_history,
            browser_import_oneshot::browser_detect_profiles,
            browser_import_oneshot::browser_import_bookmarks_cmd,
            browser_import_oneshot::browser_import_history_cmd,
            // Document parsing (PDF / DOCX / XLSX / HTML / MD)
            document_parse::document_detect_format,
            document_parse::document_extract_text,
            // Wikipedia
            wikipedia_fetch::wikipedia_search,
            wikipedia_fetch::wikipedia_fetch_article,
            // ╔═══════════════════════════════════════════════════════════╗
            // ║ LAYER 4 — USER-FACING                                     ║
            // ║ Chat UX, sandbox, DigiImp, widgets, nudges                ║
            // ╚═══════════════════════════════════════════════════════════╝
            // HyperChat (mode machine + event bus + sessions)
            hyperchat_lite::commands::hyperchat_mode_transition,
            hyperchat_lite::commands::hyperchat_event_stats,
            hyperchat_lite::commands::hyperchat_session_new,
            // Code interpreter sandbox (wasmtime + Pyodide + QuickJS)
            code_sandbox_lite::commands::sandbox_create_cell,
            code_sandbox_lite::commands::sandbox_run_cell,
            code_sandbox_lite::commands::sandbox_stop_cell,
            code_sandbox_lite::commands::sandbox_clear_cell,
            code_sandbox_lite::commands::sandbox_get_outputs,
            code_sandbox_lite::commands::sandbox_get_variables,
            code_sandbox_lite::commands::sandbox_inspect_variable,
            code_sandbox_lite::commands::sandbox_resource_status,
            code_sandbox_lite::commands::sandbox_set_limits,
            code_sandbox_lite::commands::sandbox_get_limits,
            code_sandbox_lite::commands::sandbox_supported_languages,
            code_sandbox_lite::commands::sandbox_get_history,
            code_sandbox_lite::commands::sandbox_export_notebook,
            code_sandbox_lite::commands::sandbox_import_notebook,
            code_sandbox_lite::commands::sandbox_attach_file,
            code_sandbox_lite::commands::sandbox_detach_file,
            code_sandbox_lite::commands::sandbox_list_attached_files,
            code_sandbox_lite::commands::sandbox_provenance_chain,
            code_sandbox_lite::commands::sandbox_audit_log,
            code_sandbox_lite::commands::sandbox_pause_session,
            code_sandbox_lite::commands::sandbox_resume_session,
            code_sandbox_lite::commands::sandbox_session_status,
            code_sandbox_lite::commands::sandbox_quick_python,
            code_sandbox_lite::commands::sandbox_quick_js,
            code_sandbox_lite::commands::sandbox_health_check,
            // DigiImp companion bridge
            digiimp_bridge::digiimp_set_state,
            digiimp_bridge::digiimp_set_energy,
            digiimp_bridge::digiimp_set_glow,
            digiimp_bridge::digiimp_set_color_preset,
            digiimp_bridge::digiimp_set_display_mode,
            digiimp_bridge::digiimp_get_state,
            digiimp_bridge::digiimp_rest_mode,
            digiimp_bridge::digiimp_wake,
            // Widgets + Universal Server
            widgets_lite::widget_create,
            widgets_lite::widget_list,
            widgets_lite::widget_suspend,
            widgets_lite::widget_resume,
            widgets_lite::widget_close,
            universal_lite::universal_register_tool,
            universal_lite::universal_list_tools,
            universal_lite::universal_invoke,
            // Upgrade nudges
            upgrade_nudge::nudge_evaluate,
            upgrade_nudge::nudge_dismiss,
            upgrade_nudge::nudge_global_disable,
            // ╔═══════════════════════════════════════════════════════════╗
            // ║ LAYER 5 — ENTERPRISE                                      ║
            // ║ MCP marketplace + Pro hand-off staging                    ║
            // ╚═══════════════════════════════════════════════════════════╝
            // MCP marketplace + runner + health + ledger + bundle
            mcp::commands::mcp_marketplace_browse,
            mcp::commands::mcp_marketplace_search,
            mcp::commands::mcp_marketplace_sync_mirror,
            mcp::commands::mcp_marketplace_get_offline_status,
            mcp::commands::mcp_marketplace_get,
            mcp::commands::mcp_trust_score,
            mcp::commands::mcp_install_server,
            mcp::commands::mcp_uninstall_server,
            mcp::commands::mcp_pin_version,
            mcp::commands::mcp_update_server,
            mcp::commands::mcp_list_installed,
            mcp::commands::mcp_set_enabled,
            mcp::commands::mcp_set_tool_allowlist,
            mcp::commands::mcp_get_server_state,
            mcp::commands::mcp_start_server,
            mcp::commands::mcp_stop_server,
            mcp::commands::mcp_invoke_tool,
            mcp::commands::mcp_get_tool_schema,
            mcp::commands::mcp_health_ping,
            mcp::commands::mcp_health_dashboard_data,
            mcp::commands::mcp_oauth_start,
            mcp::commands::mcp_oauth_callback,
            mcp::commands::mcp_oauth_revoke,
            mcp::commands::mcp_oauth_get_tokens,
            mcp::commands::mcp_capability_diff,
            mcp::commands::mcp_ledger_query,
            mcp::commands::mcp_ledger_verify_chain,
            mcp::commands::mcp_ledger_export,
            mcp::commands::mcp_sandbox_plan,
            mcp::commands::mcp_sandbox_status,
            mcp::commands::mcp_bundle_list,
            mcp::commands::mcp_bundle_install,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
