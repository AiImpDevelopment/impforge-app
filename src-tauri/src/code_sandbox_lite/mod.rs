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
//! | `commands.rs`         | Tauri command surface (25 commands, Crown-Jewel split)       |
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
pub mod commands;
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
    ResourceUsage, SessionId, SessionState, TrustLevel, Variable, FUEL_PER_SECOND, MAX_DISK_MIB,
    MAX_MEMORY_MIB, MAX_STDOUT_BYTES, MAX_TIME_SECS, MIN_MEMORY_MIB, MIN_TIME_SECS,
};

// Re-export the Tauri command surface so external callers keep using
// `code_sandbox_lite::sandbox_*` paths.  The handler list in `lib.rs`
// uses the explicit `commands::sandbox_*` form for clarity.
pub use commands::{
    sandbox_attach_file, sandbox_audit_log, sandbox_clear_cell, sandbox_create_cell,
    sandbox_detach_file, sandbox_export_notebook, sandbox_get_history, sandbox_get_limits,
    sandbox_get_outputs, sandbox_get_variables, sandbox_health_check, sandbox_import_notebook,
    sandbox_inspect_variable, sandbox_list_attached_files, sandbox_pause_session,
    sandbox_provenance_chain, sandbox_quick_js, sandbox_quick_python, sandbox_resource_status,
    sandbox_resume_session, sandbox_run_cell, sandbox_session_status, sandbox_set_limits,
    sandbox_stop_cell, sandbox_supported_languages,
};

// ──────────────────────────────────────────────────────────────────────────
// Shared state singleton (consumed by `commands.rs`)
// ──────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

/// Global sandbox session manager — shared across Tauri commands.
///
/// Exposed `pub(crate)` so [`commands`] can reach in via [`state`]
/// without re-exporting the whole struct as a public API.
pub(crate) struct SandboxState {
    pub(crate) sessions: Mutex<HashMap<SessionId, SessionState>>,
    pub(crate) cells: Mutex<HashMap<CellId, Cell>>,
    pub(crate) results: Mutex<HashMap<CellId, CellResult>>,
    pub(crate) audit: Mutex<AuditLog>,
    pub(crate) inspector: Mutex<Inspector>,
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

/// Lazily initialised process-wide sandbox state.  All Tauri command
/// handlers in [`commands`] resolve through this single owner so the
/// audit chain, inspector, and session map stay in sync.
pub(crate) fn state() -> Arc<SandboxState> {
    static STATE: OnceLock<Arc<SandboxState>> = OnceLock::new();
    STATE.get_or_init(|| Arc::new(SandboxState::new())).clone()
}
