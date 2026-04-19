// SPDX-License-Identifier: MIT
//! ImpForge MIT freemium app — Tauri 2.10 + Svelte 5 backend.

pub mod error;

pub mod auto_digest_lite;
pub mod browser_import_oneshot;
pub mod chat_lite;
pub mod chat_session_memory;
pub mod document_parse;
pub mod hyperchat_lite;
pub mod knowledge_lite;
pub mod memory_lite;
pub mod slash_commands;
pub mod wikipedia_fetch;

use error::AppResult;

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
            ping,
            chat_lite::chat_stream,
            chat_session_memory::session_create_thread,
            chat_session_memory::session_append_message,
            chat_session_memory::session_list_threads,
            chat_session_memory::session_get_messages,
            slash_commands::slash_catalog,
            slash_commands::slash_dispatch,
            hyperchat_lite::hyperchat_mode_transition,
            hyperchat_lite::hyperchat_event_stats,
            hyperchat_lite::hyperchat_session_new,
            knowledge_lite::knowledge_insert,
            knowledge_lite::knowledge_search,
            knowledge_lite::knowledge_count,
            memory_lite::memory_store,
            memory_lite::memory_search,
            memory_lite::memory_recall_recent,
            auto_digest_lite::digest_add_source,
            auto_digest_lite::digest_remove_source,
            auto_digest_lite::digest_list_sources,
            auto_digest_lite::digest_run_once,
            auto_digest_lite::digest_history,
            browser_import_oneshot::browser_detect_profiles,
            browser_import_oneshot::browser_import_bookmarks,
            browser_import_oneshot::browser_import_history,
            document_parse::document_detect_format,
            document_parse::document_extract_text,
            wikipedia_fetch::wikipedia_search,
            wikipedia_fetch::wikipedia_fetch_article,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
