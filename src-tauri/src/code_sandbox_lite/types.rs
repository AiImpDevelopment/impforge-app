// SPDX-License-Identifier: MIT
//! Core types for the Code Interpreter Sandbox (Feature 5, Tier 2).
//!
//! ## Cybercrime liability defence (CANONICAL — read before editing)
//!
//! User-supplied code runs in a wasmtime sandbox ON THE USER'S MACHINE.
//! Under the **Microsoft Word principle** (BGH VI ZR 335/12), ImpForge
//! ships the tool; the user is the DEPLOYER of their code under EU AI
//! Act Art 26.  StGB §202c liability is shielded by:
//!   1. sandbox isolation — no network, no FS write, no syscall escape
//!   2. fuel limit          — CPU bombs cannot DoS the host
//!   3. memory cap          — memory bombs cannot OOM the host
//!   4. explicit preopens   — the user picks which directory is read
//!   5. Trust Levels        — user consents per cell via `TrustLevel`
//!   6. Merkle audit chain  — every cell execution is signed + logged
//!   7. provenance chain    — every output is provenance-linked to the
//!      user's Knowledge Fabric (not ours)
//!
//! No code ever leaves the user's machine.  No telemetry.  No LLM call
//! routes through impforge.com.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ───────────────────────────────────────────────────────────────────────────
// Languages
// ───────────────────────────────────────────────────────────────────────────

/// Supported source languages in the Tier-2 (App) sandbox.  Pro adds
/// Ruby, Go, R, Lua on top of these two.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Python,
    JavaScript,
    Wasm,
}

impl Lang {
    /// Parses a user-supplied string into a `Lang`.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "python" | "py" | "pyodide" => Ok(Self::Python),
            "js" | "javascript" | "node" | "quickjs" => Ok(Self::JavaScript),
            "wasm" | "webassembly" => Ok(Self::Wasm),
            other => Err(format!("unsupported language `{other}`")),
        }
    }
    /// Short machine-friendly string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::Wasm => "wasm",
        }
    }
    /// True if the language needs a large pre-packaged runtime blob
    /// (Pyodide), false if it runs pure wasmtime.
    pub fn needs_blob(self) -> bool {
        matches!(self, Self::Python)
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Trust levels
// ───────────────────────────────────────────────────────────────────────────

/// Trust-level enum — cybercrime-liability gating per cell.
///
/// User explicitly elevates per cell.  App defaults to Low; passing
/// a preopen raises to Medium; Pro adds High via Trust Levels API.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// No network, no FS.  Default.
    #[default]
    Low,
    /// Preopen a single directory read-only.
    Medium,
    /// Preopen writable + full sandbox + GPU (Pro only).
    High,
}

// ───────────────────────────────────────────────────────────────────────────
// Resource limits
// ───────────────────────────────────────────────────────────────────────────

/// User-supplied limits, bounded by [`MIN_*`] / [`MAX_*`] constants.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Limits {
    pub memory_mib: u32,
    pub time_secs: u64,
    pub stdout_cap: usize,
    pub stderr_cap: usize,
    pub disk_mib: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_mib: 512,
            time_secs: 60,
            stdout_cap: 16 * 1024 * 1024,
            stderr_cap: 16 * 1024 * 1024,
            disk_mib: 64,
        }
    }
}

pub const MIN_MEMORY_MIB: u32 = 32;
pub const MAX_MEMORY_MIB: u32 = 8192;
pub const MIN_TIME_SECS: u64 = 1;
pub const MAX_TIME_SECS: u64 = 1800;
pub const MAX_STDOUT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_DISK_MIB: u64 = 1024;

/// One unit of WASI fuel ≈ one Wasm operation.  We tune the budget at
/// 100 M ops per second — Pyodide hello-world burns ~100 M.
pub const FUEL_PER_SECOND: u64 = 100_000_000;

// ───────────────────────────────────────────────────────────────────────────
// Cells
// ───────────────────────────────────────────────────────────────────────────

/// Unique cell identifier — UUID v4 for opaque pointer, random enough
/// to prevent races between simultaneous runs.
pub type CellId = String;

/// Unique session identifier — a notebook session is a group of cells
/// sharing variables.
pub type SessionId = String;

/// A Jupyter-style code cell — source + metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub id: CellId,
    pub session_id: SessionId,
    pub lang: Lang,
    pub source: String,
    pub created_unix: i64,
    pub trust_level: TrustLevel,
    /// Optional preopen dir when trust level is Medium+.
    pub preopen: Option<PathBuf>,
    /// Env vars passed to the guest.
    pub env: Vec<(String, String)>,
    /// Provenance — SHA-256 of source, linked to Knowledge Fabric.
    pub source_sha256: String,
    /// Cell tags (user-facing) — e.g. "plot", "benchmark".
    pub tags: Vec<String>,
}

/// Status of a cell's execution lifecycle.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellStatus {
    /// Never run.
    Pending,
    /// Currently running.
    Running,
    /// Succeeded (exit 0).
    Succeeded,
    /// Failed (exit != 0, trap, or error).
    Failed,
    /// Cancelled by user.
    Cancelled,
    /// Hit the resource limit (fuel / memory / time).
    LimitExceeded,
}

impl CellStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::LimitExceeded
        )
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Outputs — mime-typed, Jupyter-compatible
// ───────────────────────────────────────────────────────────────────────────

/// One output chunk from a cell execution.  Tagged union — mirrors
/// Jupyter's output schema (stream | display_data | execute_result |
/// error) but flattened for JSON-friendly Tauri IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Output {
    /// Plain text, e.g. stdout line.
    Text {
        stream: OutputStream,
        data: String,
        ts_unix: i64,
    },
    /// Raw bytes (png / jpeg / svg / etc.).
    Image {
        mime: String,
        /// Base64-encoded payload for Tauri IPC friendliness.
        base64: String,
        ts_unix: i64,
    },
    /// HTML / markdown payload (matplotlib canvas, rich-text).
    Html {
        data: String,
        ts_unix: i64,
    },
    /// Tabular data — Arrow-style JSON rows.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<serde_json::Value>>,
        ts_unix: i64,
    },
    /// A file dropped into the virtual FS (e.g. plot.png).
    File {
        path: String,
        mime: String,
        bytes: usize,
        ts_unix: i64,
    },
    /// Error with traceback.
    Error {
        ename: String,
        evalue: String,
        traceback: Vec<String>,
        ts_unix: i64,
    },
    /// The final `return`-value of an expression.
    Result {
        repr: String,
        ts_unix: i64,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

// ───────────────────────────────────────────────────────────────────────────
// Resource metrics
// ───────────────────────────────────────────────────────────────────────────

/// Post-run metrics — sent to the UI's ResourceMonitor.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_peak_bytes: u64,
    pub fuel_consumed: u64,
    pub wall_time_ms: u128,
    pub cpu_time_ms: u128,
    pub disk_bytes_written: u64,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

// ───────────────────────────────────────────────────────────────────────────
// Cell execution result
// ───────────────────────────────────────────────────────────────────────────

/// Full result of executing one cell — status + outputs + usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellResult {
    pub cell_id: CellId,
    pub status: CellStatus,
    pub exit_code: i32,
    pub outputs: Vec<Output>,
    pub usage: ResourceUsage,
    pub started_unix: i64,
    pub finished_unix: i64,
    pub error: Option<String>,
    /// Audit-chain entry ID — points at the Merkle-audit log entry.
    pub audit_entry_id: Option<String>,
}

// ───────────────────────────────────────────────────────────────────────────
// Variables — inspector between cells
// ───────────────────────────────────────────────────────────────────────────

/// A variable visible to the inspector UI.  Mirrors the guest-side
/// Python / JS globals after a cell runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub type_name: String,
    pub repr: String,
    pub size_bytes: u64,
    /// Serialized JSON value for scalars, else `null`.
    pub value_json: Option<serde_json::Value>,
}

// ───────────────────────────────────────────────────────────────────────────
// Provenance chain — links to Knowledge Fabric
// ───────────────────────────────────────────────────────────────────────────

/// A single edge in the provenance chain.  `input_ids` are citation
/// IDs from the Knowledge Fabric (F2 + F3); `output_sha256` is the
/// hash of the cell's outputs.  Every cell produces ≥ 1 entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceEdge {
    pub cell_id: CellId,
    pub session_id: SessionId,
    pub input_ids: Vec<String>,
    pub output_sha256: String,
    pub model_name: Option<String>,
    pub timestamp_unix: i64,
    pub trust_level: TrustLevel,
}

// ───────────────────────────────────────────────────────────────────────────
// Sessions — a notebook document is a session
// ───────────────────────────────────────────────────────────────────────────

/// Session state — notebook-level scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub id: SessionId,
    pub title: String,
    pub created_unix: i64,
    pub paused: bool,
    pub cell_ids: Vec<CellId>,
    pub limits: Limits,
    pub trust_level: TrustLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_parses_known_aliases() {
        assert_eq!(Lang::parse("python").expect("py"), Lang::Python);
        assert_eq!(Lang::parse("JS").expect("js"), Lang::JavaScript);
        assert_eq!(Lang::parse("wasm").expect("wasm"), Lang::Wasm);
        assert!(Lang::parse("ruby").is_err(), "Ruby is Pro-only");
    }

    #[test]
    fn lang_as_str_round_trip() {
        for lang in [Lang::Python, Lang::JavaScript, Lang::Wasm] {
            assert_eq!(Lang::parse(lang.as_str()).expect("round-trip"), lang);
        }
    }

    #[test]
    fn lang_needs_blob_only_for_python() {
        assert!(Lang::Python.needs_blob());
        assert!(!Lang::JavaScript.needs_blob());
        assert!(!Lang::Wasm.needs_blob());
    }

    #[test]
    fn trust_level_defaults_to_low() {
        assert_eq!(TrustLevel::default(), TrustLevel::Low);
    }

    #[test]
    fn cell_status_terminal_correct() {
        assert!(!CellStatus::Pending.is_terminal());
        assert!(!CellStatus::Running.is_terminal());
        assert!(CellStatus::Succeeded.is_terminal());
        assert!(CellStatus::Failed.is_terminal());
        assert!(CellStatus::Cancelled.is_terminal());
        assert!(CellStatus::LimitExceeded.is_terminal());
    }

    #[test]
    fn limits_default_sensible_values() {
        let l = Limits::default();
        assert!((MIN_MEMORY_MIB..=MAX_MEMORY_MIB).contains(&l.memory_mib));
        assert!((MIN_TIME_SECS..=MAX_TIME_SECS).contains(&l.time_secs));
        assert!(l.stdout_cap <= MAX_STDOUT_BYTES);
    }

    #[test]
    fn resource_usage_default_zeroed() {
        let u = ResourceUsage::default();
        assert_eq!(u.memory_peak_bytes, 0);
        assert_eq!(u.fuel_consumed, 0);
        assert_eq!(u.wall_time_ms, 0);
    }

    #[test]
    fn output_serializes_with_kind_tag() {
        let out = Output::Text {
            stream: OutputStream::Stdout,
            data: "hi".into(),
            ts_unix: 0,
        };
        let json = serde_json::to_string(&out).expect("serialize");
        assert!(json.contains("\"kind\":\"text\""));
        assert!(json.contains("\"stream\":\"stdout\""));
    }
}
