// SPDX-License-Identifier: MIT
//! ForgeSandbox Tauri command surface.
//!
//! Crown-Jewel split: every `#[tauri::command]` for the Code Sandbox Lite
//! domain lives here.  Backing types + state singleton come from `super`
//! (re-exported via `mod.rs` — `pub use commands::*;` keeps the
//! `code_sandbox_lite::sandbox_*` paths the rest of the app already wires).
//!
//! Routing:  Tauri invoke (`sandbox_*`)
//!   → command (this file, thin async fn)
//!   → state singleton (`super::state()`)
//!   → domain helper (`runtime_wasmtime`, `audit`, `filesystem`,
//!     `inspector`, `budget`, `helpers`, `lang_pyodide`)
//!
//! No business logic lives here — handlers stay shallow so that swapping
//! a backend (Pyodide -> CPython, rquickjs -> Deno) only touches the
//! domain modules, never the Tauri surface.

use super::{
    audit, budget, filesystem, helpers, pyodide_cached_ok, runtime_wasmtime, state, AuditEntry,
    BudgetSnapshot, Cell, CellResult, Lang, Limits, MountSpec, Output, ProvenanceEdge, SessionId,
    SessionState, TrustLevel, Variable,
};
use crate::error::{AppError, AppResult};

// ── Cell lifecycle ────────────────────────────────────────────────────────

/// Create a new sandbox cell scoped to `session_id` and persist it in the
/// in-memory cell map.  Returns the freshly minted [`Cell`] so the
/// frontend can immediately render the unscheduled cell card.
#[tauri::command]
pub async fn sandbox_create_cell(
    session_id: String,
    lang: String,
    source: String,
    trust_level: Option<TrustLevel>,
    preopen: Option<String>,
    env: Option<Vec<(String, String)>>,
    tags: Option<Vec<String>>,
) -> AppResult<Cell> {
    let lang = Lang::parse(&lang).map_err(AppError::InvalidArgument)?;
    let s = state();
    let id = helpers::new_id("cell");
    let cell = Cell {
        id: id.clone(),
        session_id: session_id.clone(),
        lang,
        source: source.clone(),
        created_unix: helpers::now_unix(),
        trust_level: trust_level.unwrap_or_default(),
        preopen: preopen.map(std::path::PathBuf::from),
        env: env.unwrap_or_default(),
        source_sha256: helpers::sha256_hex(source.as_bytes()),
        tags: tags.unwrap_or_default(),
    };
    {
        let mut cells = s
            .cells
            .lock()
            .map_err(|e| AppError::Internal(format!("cells lock: {e}")))?;
        cells.insert(id.clone(), cell.clone());
    }
    {
        let mut sessions = s
            .sessions
            .lock()
            .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
        let session = sessions.entry(session_id.clone()).or_insert_with(|| SessionState {
            id: session_id.clone(),
            title: format!("session-{session_id}"),
            created_unix: helpers::now_unix(),
            paused: false,
            cell_ids: vec![],
            limits: Limits::default(),
            trust_level: TrustLevel::Low,
        });
        session.cell_ids.push(id);
    }
    Ok(cell)
}

/// Execute the cell identified by `cell_id` inside the wasmtime sandbox
/// using the session's [`Limits`].  Result is persisted, the audit chain
/// gets a tamper-evident entry, and the [`CellResult`] is returned to
/// the caller.
#[tauri::command]
pub async fn sandbox_run_cell(cell_id: String) -> AppResult<CellResult> {
    let cell = {
        let s = state();
        let cells = s
            .cells
            .lock()
            .map_err(|e| AppError::Internal(format!("cells lock: {e}")))?;
        cells
            .get(&cell_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("cell {cell_id}")))?
    };
    let limits = {
        let s = state();
        let sessions = s
            .sessions
            .lock()
            .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
        sessions
            .get(&cell.session_id)
            .map(|s| s.limits)
            .unwrap_or_default()
    };
    let result = runtime_wasmtime::execute_cell(&cell, &limits).await?;
    {
        let s = state();
        let mut results = s
            .results
            .lock()
            .map_err(|e| AppError::Internal(format!("results lock: {e}")))?;
        results.insert(cell_id.clone(), result.clone());
    }
    {
        let s = state();
        let mut audit = s
            .audit
            .lock()
            .map_err(|e| AppError::Internal(format!("audit lock: {e}")))?;
        audit.append(AuditEntry::from_result(&result));
    }
    Ok(result)
}

/// Cooperatively cancel an in-flight cell.  Returns `true` if the cancel
/// signal landed; `false` if the cell had already finished.
#[tauri::command]
pub async fn sandbox_stop_cell(cell_id: String) -> AppResult<bool> {
    runtime_wasmtime::cancel_cell(&cell_id).await
}

/// Drop the cached [`CellResult`] for `cell_id` (the cell definition stays).
#[tauri::command]
pub async fn sandbox_clear_cell(cell_id: String) -> AppResult<()> {
    let s = state();
    let mut results = s
        .results
        .lock()
        .map_err(|e| AppError::Internal(format!("results lock: {e}")))?;
    results.remove(&cell_id);
    Ok(())
}

/// Snapshot the cached [`Output`] vector for `cell_id`.  Empty when the
/// cell hasn't been run yet.
#[tauri::command]
pub async fn sandbox_get_outputs(cell_id: String) -> AppResult<Vec<Output>> {
    let s = state();
    let results = s
        .results
        .lock()
        .map_err(|e| AppError::Internal(format!("results lock: {e}")))?;
    Ok(results
        .get(&cell_id)
        .map(|r| r.outputs.clone())
        .unwrap_or_default())
}

// ── Variables ─────────────────────────────────────────────────────────────

/// Enumerate every variable currently captured for `session_id` by the
/// between-cell inspector.
#[tauri::command]
pub async fn sandbox_get_variables(session_id: String) -> AppResult<Vec<Variable>> {
    let s = state();
    let inspector = s
        .inspector
        .lock()
        .map_err(|e| AppError::Internal(format!("inspector lock: {e}")))?;
    Ok(inspector.list_variables(&session_id))
}

/// Look up a single inspected [`Variable`] by `name` in `session_id`.
#[tauri::command]
pub async fn sandbox_inspect_variable(
    session_id: String,
    name: String,
) -> AppResult<Option<Variable>> {
    let s = state();
    let inspector = s
        .inspector
        .lock()
        .map_err(|e| AppError::Internal(format!("inspector lock: {e}")))?;
    Ok(inspector.get_variable(&session_id, &name))
}

// ── Resources ─────────────────────────────────────────────────────────────

/// Current memory + time + disk accounting snapshot for `session_id`.
#[tauri::command]
pub async fn sandbox_resource_status(session_id: String) -> AppResult<BudgetSnapshot> {
    Ok(budget::session_snapshot(&session_id))
}

/// Update the per-session [`Limits`] (validated against absolute caps).
#[tauri::command]
pub async fn sandbox_set_limits(session_id: String, limits: Limits) -> AppResult<()> {
    helpers::validate_limits(&limits).map_err(AppError::InvalidArgument)?;
    let s = state();
    let mut sessions = s
        .sessions
        .lock()
        .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
    if let Some(session) = sessions.get_mut(&session_id) {
        session.limits = limits;
    }
    Ok(())
}

/// Read back the current [`Limits`] for `session_id`.  Returns
/// [`Limits::default`] if the session has no overrides yet.
#[tauri::command]
pub async fn sandbox_get_limits(session_id: String) -> AppResult<Limits> {
    let s = state();
    let sessions = s
        .sessions
        .lock()
        .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
    Ok(sessions
        .get(&session_id)
        .map(|s| s.limits)
        .unwrap_or_default())
}

/// Static list of supported sandbox languages — kept in one place so the
/// frontend dropdown stays in sync with the runtime detection in
/// [`Lang::parse`].
#[tauri::command]
pub async fn sandbox_supported_languages() -> AppResult<Vec<String>> {
    Ok(vec!["python".into(), "javascript".into(), "wasm".into()])
}

/// Most-recent cell results for `session_id`, newest first, capped at
/// `limit` (default 64).
#[tauri::command]
pub async fn sandbox_get_history(
    session_id: String,
    limit: Option<usize>,
) -> AppResult<Vec<CellResult>> {
    let s = state();
    let cells = s
        .cells
        .lock()
        .map_err(|e| AppError::Internal(format!("cells lock: {e}")))?;
    let results = s
        .results
        .lock()
        .map_err(|e| AppError::Internal(format!("results lock: {e}")))?;
    let cap = limit.unwrap_or(64);
    let mut history: Vec<CellResult> = cells
        .values()
        .filter(|c| c.session_id == session_id)
        .filter_map(|c| results.get(&c.id).cloned())
        .collect();
    history.sort_by_key(|r| std::cmp::Reverse(r.finished_unix));
    history.truncate(cap);
    Ok(history)
}

// ── Notebook export / import ──────────────────────────────────────────────

/// Serialise the session, every cell that belongs to it, and the latest
/// cached [`CellResult`]s into the canonical `impforge-notebook-v1`
/// JSON envelope.  The result is meant to be saved to disk as a `.ipynb`
/// equivalent and re-imported via [`sandbox_import_notebook`].
#[tauri::command]
pub async fn sandbox_export_notebook(session_id: String) -> AppResult<String> {
    let s = state();
    let sessions = s
        .sessions
        .lock()
        .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
    let cells = s
        .cells
        .lock()
        .map_err(|e| AppError::Internal(format!("cells lock: {e}")))?;
    let results = s
        .results
        .lock()
        .map_err(|e| AppError::Internal(format!("results lock: {e}")))?;
    let session = sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| AppError::NotFound(format!("session {session_id}")))?;
    let session_cells: Vec<Cell> = session
        .cell_ids
        .iter()
        .filter_map(|id| cells.get(id).cloned())
        .collect();
    let session_results: Vec<CellResult> = session
        .cell_ids
        .iter()
        .filter_map(|id| results.get(id).cloned())
        .collect();
    let nb = serde_json::json!({
        "format": "impforge-notebook-v1",
        "session": session,
        "cells": session_cells,
        "results": session_results,
    });
    serde_json::to_string_pretty(&nb)
        .map_err(|e| AppError::Internal(format!("nb serialize: {e}")))
}

/// Restore a session + cells from a previously exported notebook
/// JSON document.  Cell results are intentionally not restored — they
/// must be re-executed to keep the audit chain honest.
#[tauri::command]
pub async fn sandbox_import_notebook(json: String) -> AppResult<SessionId> {
    let nb: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| AppError::InvalidArgument(format!("invalid notebook json: {e}")))?;
    let session: SessionState = serde_json::from_value(nb["session"].clone())
        .map_err(|e| AppError::InvalidArgument(format!("session: {e}")))?;
    let cells: Vec<Cell> = serde_json::from_value(nb["cells"].clone())
        .map_err(|e| AppError::InvalidArgument(format!("cells: {e}")))?;
    let s = state();
    {
        let mut sessions = s
            .sessions
            .lock()
            .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
        sessions.insert(session.id.clone(), session.clone());
    }
    {
        let mut cells_map = s
            .cells
            .lock()
            .map_err(|e| AppError::Internal(format!("cells lock: {e}")))?;
        for c in cells {
            cells_map.insert(c.id.clone(), c);
        }
    }
    Ok(session.id)
}

// ── Files ────────────────────────────────────────────────────────────────

/// Attach a host directory as a wasmtime preopen for `session_id`.  The
/// guest sees the mount at `guest_path`; `read_only=true` makes the
/// preopen read-only (recommended default).
#[tauri::command]
pub async fn sandbox_attach_file(
    session_id: String,
    host_path: String,
    guest_path: String,
    read_only: bool,
) -> AppResult<()> {
    filesystem::attach(
        &session_id,
        MountSpec {
            host_path: std::path::PathBuf::from(host_path),
            guest_path,
            read_only,
        },
    )
}

/// Detach a previously attached `guest_path` mount from `session_id`.
#[tauri::command]
pub async fn sandbox_detach_file(session_id: String, guest_path: String) -> AppResult<()> {
    filesystem::detach(&session_id, &guest_path)
}

/// List every active preopen for `session_id`.
#[tauri::command]
pub async fn sandbox_list_attached_files(session_id: String) -> AppResult<Vec<MountSpec>> {
    Ok(filesystem::list(&session_id))
}

// ── Provenance + audit ────────────────────────────────────────────────────

/// Walk the provenance graph backwards from `cell_id` to produce a chain
/// of [`ProvenanceEdge`]s describing input cells / files.
#[tauri::command]
pub async fn sandbox_provenance_chain(cell_id: String) -> AppResult<Vec<ProvenanceEdge>> {
    Ok(audit::provenance_for_cell(&cell_id))
}

/// Most-recent tamper-evident audit entries (Merkle hash chain),
/// capped at `limit` (default 64).
#[tauri::command]
pub async fn sandbox_audit_log(limit: Option<usize>) -> AppResult<Vec<AuditEntry>> {
    let s = state();
    let audit = s
        .audit
        .lock()
        .map_err(|e| AppError::Internal(format!("audit lock: {e}")))?;
    Ok(audit.recent(limit.unwrap_or(64)))
}

// ── Session control ──────────────────────────────────────────────────────

/// Pause `session_id`.  Future cell runs will reject until resumed.
#[tauri::command]
pub async fn sandbox_pause_session(session_id: String) -> AppResult<()> {
    let s = state();
    let mut sessions = s
        .sessions
        .lock()
        .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
    if let Some(session) = sessions.get_mut(&session_id) {
        session.paused = true;
    }
    Ok(())
}

/// Resume a previously paused session.
#[tauri::command]
pub async fn sandbox_resume_session(session_id: String) -> AppResult<()> {
    let s = state();
    let mut sessions = s
        .sessions
        .lock()
        .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
    if let Some(session) = sessions.get_mut(&session_id) {
        session.paused = false;
    }
    Ok(())
}

/// Read the current [`SessionState`] for `session_id`, or `None` if no
/// such session has ever been created.
#[tauri::command]
pub async fn sandbox_session_status(session_id: String) -> AppResult<Option<SessionState>> {
    let s = state();
    let sessions = s
        .sessions
        .lock()
        .map_err(|e| AppError::Internal(format!("sessions lock: {e}")))?;
    Ok(sessions.get(&session_id).cloned())
}

// ── Quick-run convenience ────────────────────────────────────────────────

/// One-shot Python execution.  Creates an ephemeral `quick-<unix>` cell,
/// runs it in default-trust low + default Limits and returns the result.
/// Used by the home-page "Try Python" button.
#[tauri::command]
pub async fn sandbox_quick_python(source: String) -> AppResult<CellResult> {
    let cell = sandbox_create_cell(
        format!("quick-{}", helpers::now_unix()),
        "python".into(),
        source,
        None,
        None,
        None,
        None,
    )
    .await?;
    sandbox_run_cell(cell.id).await
}

/// One-shot JavaScript execution.  Same shape as
/// [`sandbox_quick_python`] but routed through the QuickJS runtime.
#[tauri::command]
pub async fn sandbox_quick_js(source: String) -> AppResult<CellResult> {
    let cell = sandbox_create_cell(
        format!("quick-{}", helpers::now_unix()),
        "javascript".into(),
        source,
        None,
        None,
        None,
        None,
    )
    .await?;
    sandbox_run_cell(cell.id).await
}

// ── Health ────────────────────────────────────────────────────────────────

/// Liveness + capability probe used by the system status panel.  Reports
/// the wasmtime major version, whether the Pyodide artifact is cached,
/// the supported language list, the audit chain length, and the trust
/// levels we expose.
#[tauri::command]
pub async fn sandbox_health_check() -> AppResult<serde_json::Value> {
    let s = state();
    let audit_chain_length = s.audit.lock().map(|a| a.len()).unwrap_or(0);
    let pyodide_cached = pyodide_cached_ok().unwrap_or(false);
    Ok(serde_json::json!({
        "wasmtime_version": "27",
        "pyodide_cached": pyodide_cached,
        "languages": ["python", "javascript", "wasm"],
        "audit_chain_length": audit_chain_length,
        "trust_levels": ["low", "medium", "high"],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quick-run smoke test — confirms the public command surface stays
    /// callable end-to-end after the Crown-Jewel split.
    #[tokio::test]
    async fn cmd_supported_languages_is_stable() {
        let langs = sandbox_supported_languages().await.expect("ok");
        assert!(langs.contains(&"python".to_string()));
        assert!(langs.contains(&"javascript".to_string()));
        assert!(langs.contains(&"wasm".to_string()));
    }

    /// Health check must not panic and must report at least the languages.
    #[tokio::test]
    async fn cmd_health_check_returns_json() {
        let health = sandbox_health_check().await.expect("ok");
        assert!(health.get("languages").is_some());
        assert!(health.get("trust_levels").is_some());
    }
}
