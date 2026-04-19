// SPDX-License-Identifier: MIT
//! ImpForge MIT freemium app — Tauri 2.10 + Svelte 5 backend.

pub mod error;

pub mod chat_lite;
pub mod chat_session_memory;
pub mod hyperchat_lite;
pub mod knowledge_lite;
pub mod slash_commands;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
