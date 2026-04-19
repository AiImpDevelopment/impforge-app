// SPDX-License-Identifier: MIT
//! Wasmtime runtime — the core execution engine for the App-tier
//! sandbox.
//!
//! ## What it provides
//!
//! * `compile_wasm(bytes)` — module compilation (cached by hash).
//! * `run_wasm_module(module, plan)` — single-shot execution under
//!   fuel + epoch + ResourceLimiter caps.
//! * `execute_cell(cell, limits)` — high-level entry point used by
//!   `mod.rs` Tauri commands.  Routes by language to Pyodide /
//!   QuickJS / raw Wasm.
//! * `cancel_cell(cell_id)` — kill an in-flight execution.
//!
//! ## Cybercrime liability defence
//!
//! See `mod.rs` for the canonical defence.  This module enforces the
//! technical isolation: fuel + epoch + memory limits + no network.
//!
//! ## WASIp3 streaming
//!
//! Plan calls for WASIp3 `stream<T>` — wasmtime 27 does not yet
//! expose stable APIs for that, so we use chunked polling via
//! `streaming::StreamingBuffer`.  Tier-3 (Pro) upgrades to wasmtime
//! 37+ when the API stabilises.

use crate::error::{AppError, AppResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::audit;
use super::helpers::{now_unix, sha256_hex};
use super::lang_pyodide;
use super::lang_quickjs;
use super::limiter::SandboxLimiter;
use super::types::{
    Cell, CellId, CellResult, CellStatus, Lang, Limits, Output, OutputStream, ResourceUsage,
    FUEL_PER_SECOND,
};

/// Returned by `run_wasm_module` for low-level callers (lang_pyodide,
/// lang_quickjs).  Higher-level `execute_cell` translates this into
/// `CellResult`.
#[derive(Debug, Clone)]
pub struct WasmReport {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub fuel_consumed: u64,
    pub wall_time_ms: u128,
    pub error: Option<String>,
}

/// Compiles a Wasm module.  Caches by SHA-256 of the bytes.
pub fn compile_wasm(engine: &wasmtime::Engine, bytes: &[u8]) -> AppResult<wasmtime::Module> {
    let key = sha256_hex(bytes);
    let cache = module_cache();
    if let Some(m) = cache.lock()
        .map_err(|e| AppError::Internal(format!("module cache lock: {e}")))?
        .get(&key)
        .cloned()
    {
        return Ok(m);
    }
    let module = wasmtime::Module::new(engine, bytes)
        .map_err(|e| AppError::Internal(format!("wasmtime module compile: {e}")))?;
    cache.lock()
        .map_err(|e| AppError::Internal(format!("module cache lock: {e}")))?
        .insert(key, module.clone());
    Ok(module)
}

fn module_cache() -> Arc<Mutex<HashMap<String, wasmtime::Module>>> {
    static CACHE: OnceLock<Arc<Mutex<HashMap<String, wasmtime::Module>>>> = OnceLock::new();
    CACHE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

/// Builds a fresh wasmtime engine with fuel + epoch enabled.
pub fn build_engine() -> AppResult<wasmtime::Engine> {
    let mut cfg = wasmtime::Config::new();
    cfg.consume_fuel(true);
    cfg.epoch_interruption(true);
    wasmtime::Engine::new(&cfg).map_err(|e| AppError::Internal(format!("engine init: {e}")))
}

/// Cancellation registry — `cancel_cell` raises a flag the executing
/// thread polls between epochs.
fn cancel_registry() -> Arc<Mutex<HashMap<CellId, Arc<std::sync::atomic::AtomicBool>>>> {
    static REG: OnceLock<Arc<Mutex<HashMap<CellId, Arc<std::sync::atomic::AtomicBool>>>>> =
        OnceLock::new();
    REG.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))).clone()
}

/// Walks the anyhow chain looking for a `wasmtime_wasi::I32Exit`.
fn find_i32_exit(e: &anyhow::Error) -> Option<i32> {
    for cause in e.chain() {
        if let Some(exit) = cause.downcast_ref::<wasmtime_wasi::I32Exit>() {
            return Some(exit.0);
        }
    }
    None
}

/// Executes a Wasm module under WASI preview-1 with stdin / stdout /
/// stderr captured in memory.
pub fn run_wasm_module(
    module: &wasmtime::Module,
    engine: &wasmtime::Engine,
    limits: &Limits,
    stdin_payload: &[u8],
    preopens: &[(std::path::PathBuf, String, bool)],
    envs: &[(String, String)],
    cancel_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
) -> AppResult<WasmReport> {
    use wasmtime_wasi::pipe::{MemoryInputPipe, MemoryOutputPipe};
    use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

    let started = Instant::now();
    let max_bytes = (limits.memory_mib as usize).saturating_mul(1024 * 1024);
    let fuel_budget = FUEL_PER_SECOND.saturating_mul(limits.time_secs);

    let stdin_pipe = MemoryInputPipe::new(stdin_payload.to_vec());
    let stdout_pipe = MemoryOutputPipe::new(limits.stdout_cap);
    let stderr_pipe = MemoryOutputPipe::new(limits.stderr_cap);

    let mut wasi_builder = WasiCtxBuilder::new();
    wasi_builder
        .stdin(stdin_pipe)
        .stdout(stdout_pipe.clone())
        .stderr(stderr_pipe.clone());
    for (k, v) in envs {
        wasi_builder.env(k, v);
    }
    for (host_path, guest_path, read_only) in preopens {
        let dir_perms = if *read_only {
            DirPerms::READ
        } else {
            DirPerms::all()
        };
        let file_perms = if *read_only {
            FilePerms::READ
        } else {
            FilePerms::all()
        };
        wasi_builder
            .preopened_dir(host_path, guest_path.as_str(), dir_perms, file_perms)
            .map_err(|e| AppError::Internal(format!("preopen {}: {e}", host_path.display())))?;
    }
    let wasi_ctx = wasi_builder.build_p1();

    struct StoreData {
        wasi: wasmtime_wasi::preview1::WasiP1Ctx,
        limiter: SandboxLimiter,
    }

    let mut store = wasmtime::Store::new(
        engine,
        StoreData {
            wasi: wasi_ctx,
            limiter: SandboxLimiter::new(max_bytes),
        },
    );
    store.limiter(|d| &mut d.limiter);
    store
        .set_fuel(fuel_budget)
        .map_err(|e| AppError::Internal(format!("set_fuel: {e}")))?;
    store.set_epoch_deadline(1);

    // Background thread: tick epochs every 50 ms; if cancel flag fires
    // OR wall-clock cap reached, advance the epoch (which traps the
    // guest at next epoch check).  Lifetime of this thread is bounded
    // by the .join() at the bottom (or by elapsed time + cancel).
    let engine_for_tick = engine.clone();
    let cap = Duration::from_secs(limits.time_secs);
    let cancel = cancel_flag.unwrap_or_else(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let cancel_for_tick = cancel.clone();
    let tick_started = Instant::now();
    let tick_handle = std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let elapsed = tick_started.elapsed();
            if cancel_for_tick.load(std::sync::atomic::Ordering::Relaxed) {
                engine_for_tick.increment_epoch();
                break;
            }
            if elapsed >= cap {
                engine_for_tick.increment_epoch();
                break;
            }
        }
    });

    let mut linker = wasmtime::Linker::<StoreData>::new(engine);
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |d| &mut d.wasi)
        .map_err(|e| AppError::Internal(format!("add WASI to linker: {e}")))?;

    let result: Result<i32, anyhow::Error> = (|| {
        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|e| anyhow::anyhow!("instantiate: {e}"))?;
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "main"))
            .map_err(|e| anyhow::anyhow!("missing _start/main: {e}"))?;
        match start.call(&mut store, ()) {
            Ok(()) => Ok(0),
            Err(e) => {
                if let Some(exit) = find_i32_exit(&e) {
                    Ok(exit)
                } else {
                    Err(anyhow::anyhow!("guest trapped: {e}"))
                }
            }
        }
    })();

    // Signal the tick thread to wake up early in success path.
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = tick_handle.join();

    let fuel_consumed = fuel_budget.saturating_sub(store.get_fuel().unwrap_or(0));
    let stdout = stdout_pipe.contents().to_vec();
    let stderr = stderr_pipe.contents().to_vec();
    let wall_time_ms = started.elapsed().as_millis();

    match result {
        Ok(code) => Ok(WasmReport {
            success: code == 0,
            exit_code: code,
            stdout,
            stderr,
            fuel_consumed,
            wall_time_ms,
            error: None,
        }),
        Err(e) => Ok(WasmReport {
            success: false,
            exit_code: 1,
            stdout,
            stderr,
            fuel_consumed,
            wall_time_ms,
            error: Some(format!("{e:#}")),
        }),
    }
}

/// High-level entry point used by `mod.rs::sandbox_run_cell`.
pub async fn execute_cell(cell: &Cell, limits: &Limits) -> AppResult<CellResult> {
    let started_unix = now_unix();
    let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    cancel_registry()
        .lock()
        .map_err(|e| AppError::Internal(format!("cancel reg lock: {e}")))?
        .insert(cell.id.clone(), cancel_flag.clone());

    let preopens: Vec<(std::path::PathBuf, String, bool)> = cell
        .preopen
        .iter()
        .map(|p| (p.clone(), "/data".to_string(), true))
        .collect();

    let report = match cell.lang {
        Lang::Python => {
            lang_pyodide::execute_python(&cell.source, limits, &preopens, &cell.env, cancel_flag.clone())
                .await
        }
        Lang::JavaScript => {
            lang_quickjs::execute_javascript(&cell.source, limits, cancel_flag.clone())
        }
        Lang::Wasm => {
            let bytes = if cell.source.starts_with("(module") {
                wat::parse_str(&cell.source)
                    .map_err(|e| AppError::InvalidArgument(format!("WAT: {e}")))?
            } else {
                cell.source.as_bytes().to_vec()
            };
            let engine = build_engine()?;
            let module = compile_wasm(&engine, &bytes)?;
            run_wasm_module(&module, &engine, limits, &[], &preopens, &cell.env, Some(cancel_flag.clone()))
        }
    }?;

    cancel_registry()
        .lock()
        .map_err(|e| AppError::Internal(format!("cancel reg lock: {e}")))?
        .remove(&cell.id);

    let finished_unix = now_unix();

    // Convert raw bytes to Output entries.
    let mut outputs = Vec::new();
    if !report.stdout.is_empty() {
        outputs.push(Output::Text {
            stream: OutputStream::Stdout,
            data: String::from_utf8_lossy(&report.stdout).into_owned(),
            ts_unix: finished_unix,
        });
    }
    if !report.stderr.is_empty() {
        outputs.push(Output::Text {
            stream: OutputStream::Stderr,
            data: String::from_utf8_lossy(&report.stderr).into_owned(),
            ts_unix: finished_unix,
        });
    }
    if let Some(err) = &report.error {
        outputs.push(Output::Error {
            ename: "GuestTrap".into(),
            evalue: err.clone(),
            traceback: vec![err.clone()],
            ts_unix: finished_unix,
        });
    }

    let usage = ResourceUsage {
        memory_peak_bytes: 0,
        fuel_consumed: report.fuel_consumed,
        wall_time_ms: report.wall_time_ms,
        cpu_time_ms: report.wall_time_ms,
        disk_bytes_written: 0,
        stdout_bytes: report.stdout.len(),
        stderr_bytes: report.stderr.len(),
    };

    let status = if report.success {
        CellStatus::Succeeded
    } else if report
        .error
        .as_deref()
        .map(|e| e.contains("interrupt") || e.contains("epoch"))
        .unwrap_or(false)
    {
        CellStatus::LimitExceeded
    } else {
        CellStatus::Failed
    };

    let result = CellResult {
        cell_id: cell.id.clone(),
        status,
        exit_code: report.exit_code,
        outputs,
        usage,
        started_unix,
        finished_unix,
        error: report.error,
        audit_entry_id: None,
    };

    // Append to provenance + audit.
    audit::record_provenance(cell, &result);

    Ok(result)
}

/// Cancels an in-flight cell execution.  Returns true if a cancel was
/// signalled (cell was found in the registry).
pub async fn cancel_cell(cell_id: &str) -> AppResult<bool> {
    let reg = cancel_registry();
    let guard = reg
        .lock()
        .map_err(|e| AppError::Internal(format!("cancel reg lock: {e}")))?;
    if let Some(flag) = guard.get(cell_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No-op WAT module that exits with proc_exit(0).
    fn no_op_wasm() -> Vec<u8> {
        let wat = r#"
            (module
              (import "wasi_snapshot_preview1" "proc_exit"
                (func $proc_exit (param i32)))
              (memory (export "memory") 1)
              (func $start
                i32.const 0
                call $proc_exit)
              (export "_start" (func $start)))
        "#;
        wat::parse_str(wat).expect("wat parse")
    }

    #[test]
    fn build_engine_succeeds() {
        let _ = build_engine().expect("engine builds");
    }

    #[test]
    fn compile_wasm_caches_modules() {
        let engine = build_engine().expect("engine");
        let bytes = no_op_wasm();
        let m1 = compile_wasm(&engine, &bytes).expect("compile 1");
        let m2 = compile_wasm(&engine, &bytes).expect("compile 2 hits cache");
        // Module's identity isn't stable, but cache should not error.
        let _ = (m1, m2);
    }

    #[test]
    fn run_no_op_wasm_module_succeeds() {
        let engine = build_engine().expect("engine");
        let bytes = no_op_wasm();
        let module = compile_wasm(&engine, &bytes).expect("compile");
        let limits = Limits {
            memory_mib: 64,
            time_secs: 5,
            stdout_cap: 64 * 1024,
            stderr_cap: 64 * 1024,
            disk_mib: 1,
        };
        let report =
            run_wasm_module(&module, &engine, &limits, b"", &[], &[], None).expect("run");
        assert!(report.success, "no-op should succeed: {:?}", report.error);
        assert_eq!(report.exit_code, 0);
    }

    #[test]
    fn fuel_exhaustion_traps_runaway_loop() {
        let wat = r#"
            (module
              (import "wasi_snapshot_preview1" "proc_exit"
                (func $proc_exit (param i32)))
              (memory (export "memory") 1)
              (func $start
                (local $i i32)
                (loop $forever
                  (local.set $i (i32.add (local.get $i) (i32.const 1)))
                  (br $forever)))
              (export "_start" (func $start)))
        "#;
        let bytes = wat::parse_str(wat).expect("wat");
        let engine = build_engine().expect("engine");
        let module = compile_wasm(&engine, &bytes).expect("compile");
        let limits = Limits {
            memory_mib: 64,
            time_secs: 1,
            stdout_cap: 64 * 1024,
            stderr_cap: 64 * 1024,
            disk_mib: 1,
        };
        let report =
            run_wasm_module(&module, &engine, &limits, b"", &[], &[], None).expect("run");
        assert!(!report.success, "infinite loop must be killed");
        let err = report.error.as_deref().unwrap_or("");
        assert!(
            err.contains("trap")
                || err.contains("fuel")
                || err.contains("epoch")
                || err.contains("interrupt"),
            "unexpected error: `{err}`"
        );
    }

    #[tokio::test]
    async fn execute_cell_for_wasm_lang_routes_correctly() {
        let cell = Cell {
            id: "test-cell".into(),
            session_id: "test-sess".into(),
            lang: Lang::Wasm,
            source: r#"
                (module
                  (import "wasi_snapshot_preview1" "proc_exit"
                    (func $proc_exit (param i32)))
                  (memory (export "memory") 1)
                  (func $start
                    i32.const 0
                    call $proc_exit)
                  (export "_start" (func $start)))
            "#.to_string(),
            created_unix: 0,
            trust_level: super::super::types::TrustLevel::Low,
            preopen: None,
            env: vec![],
            source_sha256: "test".into(),
            tags: vec![],
        };
        let limits = Limits::default();
        let result = execute_cell(&cell, &limits).await.expect("execute");
        assert_eq!(result.status, CellStatus::Succeeded, "{:?}", result.error);
        assert_eq!(result.exit_code, 0);
    }
}
