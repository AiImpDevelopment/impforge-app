// SPDX-License-Identifier: MIT
//! ImpForge MIT freemium app — Tauri 2.10 + Svelte 5 backend.

pub mod error;

pub mod chat_lite;

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
        .invoke_handler(tauri::generate_handler![ping, chat_lite::chat_stream])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
