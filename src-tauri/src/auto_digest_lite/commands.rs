// SPDX-License-Identifier: MIT
//! Tauri command surface for auto-digest.  Every wire-level operation
//! the frontend invokes lives here so the rest of the module stays
//! Tauri-free and unit-testable.

use crate::error::AppResult;
use std::sync::atomic::Ordering;

use super::helpers::next_source_id;
use super::scheduler::{run_once, runtime};
use super::state::{
    add_clipboard, add_feed, add_folder, add_screenshots, compute_stats,
    remove_source, save_state, tail_history,
};
use super::types::{DigestEntry, DigestSource, DigestState, DigestStats, RunOnceSummary};

#[tauri::command]
pub async fn digest_add_source(source: DigestSource) -> AppResult<DigestSource> {
    let rt = runtime();
    rt.with_state(|state| {
        match source {
            DigestSource::Feed {
                url,
                interval_secs,
                ..
            } => add_feed(state, &url, interval_secs),
            DigestSource::Folder {
                path,
                recursive,
                allow_ext,
                ..
            } => add_folder(state, path, recursive, allow_ext),
            DigestSource::Clipboard { .. } => add_clipboard(state),
            DigestSource::Screenshots { path, .. } => add_screenshots(state, path),
            DigestSource::Browser { family, .. } => {
                let src = DigestSource::Browser {
                    id: next_source_id("browser"),
                    family,
                };
                state.sources.push(src.clone());
                Ok(src)
            }
        }
        .and_then(|src| save_state(state).map(|_| src))
    })
}

#[tauri::command]
pub async fn digest_remove_source(id: String) -> AppResult<()> {
    let rt = runtime();
    rt.with_state(|state| {
        remove_source(state, &id)?;
        save_state(state)
    })
}

#[tauri::command]
pub async fn digest_list_sources() -> AppResult<Vec<DigestSource>> {
    Ok(runtime().snapshot()?.sources)
}

#[tauri::command]
pub async fn digest_run_once() -> AppResult<RunOnceSummary> {
    let rt = runtime();
    let mut state = rt.snapshot()?;
    let summary = run_once(&mut state).await?;
    rt.with_state(|s| {
        *s = state.clone();
        Ok(())
    })?;
    Ok(summary)
}

#[tauri::command]
pub async fn digest_history(limit: u32) -> AppResult<Vec<DigestEntry>> {
    tail_history(limit as usize)
}

#[tauri::command]
pub async fn digest_pause() -> AppResult<()> {
    let rt = runtime();
    rt.paused.store(true, Ordering::SeqCst);
    rt.with_state(|state| {
        state.paused = true;
        save_state(state)
    })
}

#[tauri::command]
pub async fn digest_resume() -> AppResult<()> {
    let rt = runtime();
    rt.paused.store(false, Ordering::SeqCst);
    rt.with_state(|state| {
        state.paused = false;
        save_state(state)
    })
}

#[tauri::command]
pub async fn digest_stats() -> AppResult<DigestStats> {
    let state = runtime().snapshot()?;
    compute_stats(&state)
}

#[tauri::command]
pub async fn digest_set_quiet_hours(
    enabled: bool,
    start: u32,
    end: u32,
) -> AppResult<DigestState> {
    let rt = runtime();
    rt.with_state(|state| {
        state.quiet_hours_enabled = enabled;
        state.quiet_hours_start = start.min(23);
        state.quiet_hours_end = end.min(23);
        save_state(state)?;
        Ok(state.clone())
    })
}
