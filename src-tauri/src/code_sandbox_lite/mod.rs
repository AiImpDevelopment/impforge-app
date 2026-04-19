// SPDX-License-Identifier: MIT
//! ForgeSandbox — Code Interpreter Sandbox (Feature 5, Tier 2 / App).
//!
//! Jupyter-style notebook with wasmtime + Pyodide + rquickjs cells.
//! Hard CPU + memory limits via WASI fuel + ResourceLimiter.  No
//! network, no filesystem unless the user explicitly attaches a
//! preopen.
//!
//! ## Module map
//!
//! | File                  | Purpose                                                     |
//! |-----------------------|-------------------------------------------------------------|
//! | `types.rs`            | Cell / Output / TrustLevel / Limits / Provenance            |
//! | `helpers.rs`          | sha256, hex, paths, validators                              |
//! | `runtime_wasmtime.rs` | wasmtime engine + WASI preview-1 + fuel + epoch             |
//! | `lang_pyodide.rs`     | Pyodide 0.28 wrapper (Python)                                |
//! | `lang_quickjs.rs`     | rquickjs wrapper (JavaScript)                                |
//! | `limiter.rs`          | dual-budget fuel + epoch limiter                             |
//! | `budget.rs`           | memory + time + disk accounting                              |
//! | `filesystem.rs`       | preopen + landlock                                          |
//! | `inspector.rs`        | between-cells variable inspector                             |
//! | `streaming.rs`        | chunked stdout/stderr polling (WASIp3 stream<T> in Pro)      |
//! | `audit.rs`            | tamper-evident execution log (Merkle hash chain)             |
//! | `tests.rs`            | integration tests + monty smoke-test                         |
//!
//! ## Cybercrime liability defence (CANONICAL)
//!
//! User-supplied code runs in a wasmtime sandbox ON THE USER'S MACHINE.
//! Under the **Microsoft Word principle**, ImpForge ships the tool;
//! the user is the deployer of their code under EU AI Act Art 26.
//! StGB §202c liability is shielded by sandbox isolation, fuel limit,
//! memory cap, explicit preopens, Trust Levels and Merkle audit chain.
//!
//! No code ever leaves the user's machine.

pub mod audit;
pub mod budget;
pub mod filesystem;
pub mod helpers;
pub mod inspector;
pub mod lang_pyodide;
pub mod lang_quickjs;
pub mod limiter;
pub mod runtime_wasmtime;
pub mod streaming;
pub mod tests;
pub mod types;

// Re-exports for the Tauri command surface.
pub use audit::{audit_chain_root, AuditEntry, AuditLog};
pub use budget::{Budget, BudgetSnapshot};
pub use filesystem::{FsPolicy, MountSpec};
pub use helpers::{new_id, now_unix, sandbox_cache_dir, sha256_hex, validate_limits};
pub use inspector::{Inspector, VariableSnapshot};
pub use lang_pyodide::{
    pyodide_cached_ok, pyodide_pin_version, PyodideRuntime, PyodideStatus, PYODIDE_VERSION,
};
pub use lang_quickjs::QuickJsRuntime;
pub use limiter::SandboxLimiter;
pub use runtime_wasmtime::{compile_wasm, run_wasm_module, WasmReport};
pub use streaming::{StreamReader, StreamingBuffer};
pub use types::{
    Cell, CellId, CellResult, CellStatus, Lang, Limits, Output, OutputStream, ProvenanceEdge,
    ResourceUsage, SessionId, SessionState, TrustLevel, Variable, FUEL_PER_SECOND,
    MAX_DISK_MIB, MAX_MEMORY_MIB, MAX_STDOUT_BYTES, MAX_TIME_SECS, MIN_MEMORY_MIB, MIN_TIME_SECS,
};

// ──────────────────────────────────────────────────────────────────────────
// Tauri command facade
// ──────────────────────────────────────────────────────────────────────────

use crate::error::AppResult;
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;
use std::collections::HashMap;

/// Global sandbox session manager — shared across Tauri commands.
struct SandboxState {
    sessions: Mutex<HashMap<SessionId, SessionState>>,
    cells: Mutex<HashMap<CellId, Cell>>,
    results: Mutex<HashMap<CellId, CellResult>>,
    audit: Mutex<AuditLog>,
    inspector: Mutex<Inspector>,
}

impl SandboxState {
    fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            cells: Mutex::new(HashMap::new()),
            results: Mutex::new(HashMap::new()),
            audit: Mutex::new(AuditLog::new()),
            inspector: Mutex::new(Inspector::new()),
        }
    }
}

fn state() -> Arc<SandboxState> {
    static STATE: OnceLock<Arc<SandboxState>> = OnceLock::new();
    STATE
        .get_or_init(|| Arc::new(SandboxState::new()))
        .clone()
}

// ── Cell lifecycle ────────────────────────────────────────────────────────

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
    let lang = Lang::parse(&lang).map_err(crate::error::AppError::InvalidArgument)?;
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
        let mut cells = s.cells.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("cells lock: {e}")))?;
        cells.insert(id.clone(), cell.clone());
    }
    {
        let mut sessions = s.sessions.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
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

#[tauri::command]
pub async fn sandbox_run_cell(cell_id: String) -> AppResult<CellResult> {
    let cell = {
        let s = state();
        let cells = s.cells.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("cells lock: {e}")))?;
        cells.get(&cell_id)
            .cloned()
            .ok_or_else(|| crate::error::AppError::NotFound(format!("cell {cell_id}")))?
    };
    let limits = {
        let s = state();
        let sessions = s.sessions.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
        sessions
            .get(&cell.session_id)
            .map(|s| s.limits)
            .unwrap_or_default()
    };
    let result = runtime_wasmtime::execute_cell(&cell, &limits).await?;
    {
        let s = state();
        let mut results = s.results.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("results lock: {e}")))?;
        results.insert(cell_id.clone(), result.clone());
    }
    {
        let s = state();
        let mut audit = s.audit.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("audit lock: {e}")))?;
        audit.append(AuditEntry::from_result(&result));
    }
    Ok(result)
}

#[tauri::command]
pub async fn sandbox_stop_cell(cell_id: String) -> AppResult<bool> {
    runtime_wasmtime::cancel_cell(&cell_id).await
}

#[tauri::command]
pub async fn sandbox_clear_cell(cell_id: String) -> AppResult<()> {
    let s = state();
    let mut results = s.results.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("results lock: {e}")))?;
    results.remove(&cell_id);
    Ok(())
}

#[tauri::command]
pub async fn sandbox_get_outputs(cell_id: String) -> AppResult<Vec<Output>> {
    let s = state();
    let results = s.results.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("results lock: {e}")))?;
    Ok(results
        .get(&cell_id)
        .map(|r| r.outputs.clone())
        .unwrap_or_default())
}

// ── Variables ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn sandbox_get_variables(session_id: String) -> AppResult<Vec<Variable>> {
    let s = state();
    let inspector = s.inspector.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("inspector lock: {e}")))?;
    Ok(inspector.list_variables(&session_id))
}

#[tauri::command]
pub async fn sandbox_inspect_variable(
    session_id: String,
    name: String,
) -> AppResult<Option<Variable>> {
    let s = state();
    let inspector = s.inspector.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("inspector lock: {e}")))?;
    Ok(inspector.get_variable(&session_id, &name))
}

// ── Resources ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn sandbox_resource_status(session_id: String) -> AppResult<BudgetSnapshot> {
    Ok(budget::session_snapshot(&session_id))
}

#[tauri::command]
pub async fn sandbox_set_limits(session_id: String, limits: Limits) -> AppResult<()> {
    helpers::validate_limits(&limits)
        .map_err(crate::error::AppError::InvalidArgument)?;
    let s = state();
    let mut sessions = s.sessions.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
    if let Some(session) = sessions.get_mut(&session_id) {
        session.limits = limits;
    }
    Ok(())
}

#[tauri::command]
pub async fn sandbox_get_limits(session_id: String) -> AppResult<Limits> {
    let s = state();
    let sessions = s.sessions.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
    Ok(sessions
        .get(&session_id)
        .map(|s| s.limits)
        .unwrap_or_default())
}

#[tauri::command]
pub async fn sandbox_supported_languages() -> AppResult<Vec<String>> {
    Ok(vec!["python".into(), "javascript".into(), "wasm".into()])
}

#[tauri::command]
pub async fn sandbox_get_history(session_id: String, limit: Option<usize>) -> AppResult<Vec<CellResult>> {
    let s = state();
    let cells = s.cells.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("cells lock: {e}")))?;
    let results = s.results.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("results lock: {e}")))?;
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

#[tauri::command]
pub async fn sandbox_export_notebook(session_id: String) -> AppResult<String> {
    let s = state();
    let sessions = s.sessions.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
    let cells = s.cells.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("cells lock: {e}")))?;
    let results = s.results.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("results lock: {e}")))?;
    let session = sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| crate::error::AppError::NotFound(format!("session {session_id}")))?;
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
        .map_err(|e| crate::error::AppError::Internal(format!("nb serialize: {e}")))
}

#[tauri::command]
pub async fn sandbox_import_notebook(json: String) -> AppResult<SessionId> {
    let nb: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| crate::error::AppError::InvalidArgument(format!("invalid notebook json: {e}")))?;
    let session: SessionState = serde_json::from_value(nb["session"].clone())
        .map_err(|e| crate::error::AppError::InvalidArgument(format!("session: {e}")))?;
    let cells: Vec<Cell> = serde_json::from_value(nb["cells"].clone())
        .map_err(|e| crate::error::AppError::InvalidArgument(format!("cells: {e}")))?;
    let s = state();
    {
        let mut sessions = s.sessions.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
        sessions.insert(session.id.clone(), session.clone());
    }
    {
        let mut cells_map = s.cells.lock()
            .map_err(|e| crate::error::AppError::Internal(format!("cells lock: {e}")))?;
        for c in cells {
            cells_map.insert(c.id.clone(), c);
        }
    }
    Ok(session.id)
}

// ── Files ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn sandbox_attach_file(
    session_id: String,
    host_path: String,
    guest_path: String,
    read_only: bool,
) -> AppResult<()> {
    filesystem::attach(&session_id, MountSpec {
        host_path: std::path::PathBuf::from(host_path),
        guest_path,
        read_only,
    })
}

#[tauri::command]
pub async fn sandbox_detach_file(session_id: String, guest_path: String) -> AppResult<()> {
    filesystem::detach(&session_id, &guest_path)
}

#[tauri::command]
pub async fn sandbox_list_attached_files(session_id: String) -> AppResult<Vec<MountSpec>> {
    Ok(filesystem::list(&session_id))
}

// ── Provenance + audit ────────────────────────────────────────────────────

#[tauri::command]
pub async fn sandbox_provenance_chain(cell_id: String) -> AppResult<Vec<ProvenanceEdge>> {
    Ok(audit::provenance_for_cell(&cell_id))
}

#[tauri::command]
pub async fn sandbox_audit_log(limit: Option<usize>) -> AppResult<Vec<AuditEntry>> {
    let s = state();
    let audit = s.audit.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("audit lock: {e}")))?;
    Ok(audit.recent(limit.unwrap_or(64)))
}

// ── Session control ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn sandbox_pause_session(session_id: String) -> AppResult<()> {
    let s = state();
    let mut sessions = s.sessions.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
    if let Some(session) = sessions.get_mut(&session_id) {
        session.paused = true;
    }
    Ok(())
}

#[tauri::command]
pub async fn sandbox_resume_session(session_id: String) -> AppResult<()> {
    let s = state();
    let mut sessions = s.sessions.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
    if let Some(session) = sessions.get_mut(&session_id) {
        session.paused = false;
    }
    Ok(())
}

#[tauri::command]
pub async fn sandbox_session_status(session_id: String) -> AppResult<Option<SessionState>> {
    let s = state();
    let sessions = s.sessions.lock()
        .map_err(|e| crate::error::AppError::Internal(format!("sessions lock: {e}")))?;
    Ok(sessions.get(&session_id).cloned())
}

// ── Quick-run convenience ────────────────────────────────────────────────

#[tauri::command]
pub async fn sandbox_quick_python(source: String) -> AppResult<CellResult> {
    let cell = sandbox_create_cell(
        format!("quick-{}", helpers::now_unix()),
        "python".into(),
        source,
        None, None, None, None,
    )
    .await?;
    sandbox_run_cell(cell.id).await
}

#[tauri::command]
pub async fn sandbox_quick_js(source: String) -> AppResult<CellResult> {
    let cell = sandbox_create_cell(
        format!("quick-{}", helpers::now_unix()),
        "javascript".into(),
        source,
        None, None, None, None,
    )
    .await?;
    sandbox_run_cell(cell.id).await
}

// ── Health ────────────────────────────────────────────────────────────────

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
