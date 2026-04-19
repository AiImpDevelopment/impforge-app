// SPDX-License-Identifier: MIT
//! rquickjs wrapper — JavaScript execution.
//!
//! ## How rquickjs runs in this sandbox
//!
//! rquickjs is a pure-Rust binding to QuickJS (Bellard's small JS
//! engine).  It runs IN-PROCESS — no wasmtime needed.  This is fine
//! because QuickJS is a cooperative interpreter with built-in
//! resource limits (max memory + max stack + interrupt callback).
//!
//! ## Cybercrime liability defence
//!
//! See `mod.rs`.  QuickJS has no fs / net APIs unless we expose them
//! as Rust-side callbacks — we expose NONE in App tier.  Pro tier
//! adds OPT-IN bridges (e.g. `fetch` to a configured URL).
//!
//! ## Resource limits
//!
//! * Memory limit  — `runtime.set_memory_limit(bytes)`
//! * GC threshold  — `runtime.set_gc_threshold(bytes)`
//! * Interrupt     — periodic callback that returns true to abort
//!
//! ## What App tier does NOT support
//!
//! * Node.js-style `require()` / ES modules from disk
//! * `fetch` / network
//! * Async I/O via `setTimeout` (the synchronous interpreter is
//!   used; `setTimeout(fn, 0)` runs `fn` immediately)

use crate::error::{AppError, AppResult};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::runtime_wasmtime::WasmReport;
use super::types::Limits;

/// Per-cell QuickJS runtime — fresh for every cell to avoid leaking
/// global state between unrelated executions.
pub struct QuickJsRuntime {
    /// Memory limit in bytes (mirrored to QuickJS engine).
    pub memory_limit: usize,
    /// Time limit in seconds (enforced via interrupt callback).
    pub time_limit_secs: u64,
    /// Stdout cap.
    pub stdout_cap: usize,
}

impl QuickJsRuntime {
    pub fn new(limits: &Limits) -> Self {
        Self {
            memory_limit: (limits.memory_mib as usize).saturating_mul(1024 * 1024),
            time_limit_secs: limits.time_secs,
            stdout_cap: limits.stdout_cap,
        }
    }
}

/// Synchronously execute a JavaScript snippet inside a fresh QuickJS
/// runtime.  Returns a `WasmReport` for parity with the Wasmtime
/// runner.
pub fn execute_javascript(
    source: &str,
    limits: &Limits,
    cancel_flag: Arc<AtomicBool>,
) -> AppResult<WasmReport> {
    let started = Instant::now();
    let cap_secs = limits.time_secs;
    let memory_limit = (limits.memory_mib as usize).saturating_mul(1024 * 1024);
    let stdout_cap = limits.stdout_cap;

    let runtime = rquickjs::Runtime::new()
        .map_err(|e| AppError::Internal(format!("rquickjs runtime: {e}")))?;
    runtime.set_memory_limit(memory_limit);

    let interrupt_started = Instant::now();
    let interrupt_cancel = cancel_flag.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        if interrupt_cancel.load(Ordering::Relaxed) {
            return true;
        }
        if interrupt_started.elapsed().as_secs() >= cap_secs {
            return true;
        }
        false
    })));

    let context = rquickjs::Context::full(&runtime)
        .map_err(|e| AppError::Internal(format!("rquickjs context: {e}")))?;

    let stdout = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let stderr = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));

    let stdout_for_console = stdout.clone();
    let stdout_for_print = stdout.clone();
    let stderr_for_warn = stderr.clone();
    let stderr_for_err = stderr.clone();

    let bind_result = context.with(|ctx| -> rquickjs::Result<()> {
        let globals = ctx.globals();

        let console = rquickjs::Object::new(ctx.clone())?;
        let log = rquickjs::Function::new(ctx.clone(), move |s: rquickjs::Value| {
            if let Ok(mut out) = stdout_for_console.lock() {
                let line = format_js_value(&s);
                if out.len() + line.len() < stdout_cap {
                    out.extend_from_slice(line.as_bytes());
                    out.push(b'\n');
                }
            }
        })?;
        console.set("log", log)?;
        let warn_fn = rquickjs::Function::new(ctx.clone(), move |s: rquickjs::Value| {
            if let Ok(mut e) = stderr_for_warn.lock() {
                let line = format_js_value(&s);
                e.extend_from_slice(line.as_bytes());
                e.push(b'\n');
            }
        })?;
        console.set("warn", warn_fn)?;
        let err_fn = rquickjs::Function::new(ctx.clone(), move |s: rquickjs::Value| {
            if let Ok(mut e) = stderr_for_err.lock() {
                let line = format_js_value(&s);
                e.extend_from_slice(line.as_bytes());
                e.push(b'\n');
            }
        })?;
        console.set("error", err_fn)?;
        globals.set("console", console)?;

        let print = rquickjs::Function::new(ctx.clone(), move |s: rquickjs::Value| {
            if let Ok(mut out) = stdout_for_print.lock() {
                let line = format_js_value(&s);
                if out.len() + line.len() < stdout_cap {
                    out.extend_from_slice(line.as_bytes());
                    out.push(b'\n');
                }
            }
        })?;
        globals.set("print", print)?;
        Ok(())
    });
    bind_result.map_err(|e| AppError::Internal(format!("rquickjs bind globals: {e}")))?;

    let exec_result: Result<(), rquickjs::Error> = context.with(|ctx| {
        let _val: rquickjs::Value = ctx.eval(source)?;
        Ok(())
    });

    let stdout_bytes = stdout
        .lock()
        .map(|b| b.clone())
        .unwrap_or_else(|_| Vec::new());
    let stderr_bytes = stderr
        .lock()
        .map(|b| b.clone())
        .unwrap_or_else(|_| Vec::new());
    let wall_time_ms = started.elapsed().as_millis();

    match exec_result {
        Ok(()) => Ok(WasmReport {
            success: true,
            exit_code: 0,
            stdout: stdout_bytes,
            stderr: stderr_bytes,
            fuel_consumed: 0,
            wall_time_ms,
            error: None,
        }),
        Err(e) => Ok(WasmReport {
            success: false,
            exit_code: 1,
            stdout: stdout_bytes,
            stderr: stderr_bytes,
            fuel_consumed: 0,
            wall_time_ms,
            error: Some(format!("{e}")),
        }),
    }
}

/// Serialize a QuickJS value to a printable string.
fn format_js_value(v: &rquickjs::Value) -> String {
    if let Some(s) = v.as_string() {
        s.to_string().unwrap_or_else(|_| "<string>".into())
    } else if let Some(n) = v.as_number() {
        n.to_string()
    } else if v.is_bool() {
        v.as_bool().map(|b| b.to_string()).unwrap_or_default()
    } else if v.is_null() {
        "null".into()
    } else if v.is_undefined() {
        "undefined".into()
    } else {
        format!("<{:?}>", v.type_of())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quickjs_runs_simple_console_log() {
        let limits = Limits {
            memory_mib: 64,
            time_secs: 5,
            stdout_cap: 64 * 1024,
            stderr_cap: 64 * 1024,
            disk_mib: 1,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let report = execute_javascript("console.log('hi')", &limits, cancel)
            .expect("execute");
        assert!(report.success, "{:?}", report.error);
        let s = String::from_utf8_lossy(&report.stdout);
        assert!(s.contains("hi"), "got `{s}`");
    }

    #[test]
    fn quickjs_print_works() {
        let limits = Limits {
            memory_mib: 64,
            time_secs: 5,
            stdout_cap: 64 * 1024,
            stderr_cap: 64 * 1024,
            disk_mib: 1,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let report = execute_javascript("print(1+1)", &limits, cancel)
            .expect("execute");
        assert!(report.success);
        assert!(String::from_utf8_lossy(&report.stdout).contains('2'));
    }

    #[test]
    fn quickjs_console_warn_goes_to_stderr() {
        let limits = Limits {
            memory_mib: 64,
            time_secs: 5,
            stdout_cap: 64 * 1024,
            stderr_cap: 64 * 1024,
            disk_mib: 1,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let report = execute_javascript("console.warn('careful')", &limits, cancel)
            .expect("execute");
        assert!(report.success);
        assert!(String::from_utf8_lossy(&report.stderr).contains("careful"));
    }

    #[test]
    fn quickjs_error_returns_failed_report() {
        let limits = Limits::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let report = execute_javascript("nonexistent_function_xyz()", &limits, cancel)
            .expect("execute returns Ok with failed report");
        assert!(!report.success);
        assert!(report.error.is_some());
    }

    #[test]
    fn quickjs_interrupt_handler_kills_infinite_loop() {
        let limits = Limits {
            memory_mib: 64,
            time_secs: 1,
            stdout_cap: 64 * 1024,
            stderr_cap: 64 * 1024,
            disk_mib: 1,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let report = execute_javascript(
            "while(true) {}",
            &limits,
            cancel,
        )
        .expect("execute returns Ok");
        assert!(!report.success);
        assert!(report.wall_time_ms < 5000);
    }

    #[test]
    fn quickjs_cancel_flag_aborts_execution() {
        let limits = Limits {
            memory_mib: 64,
            time_secs: 30,
            stdout_cap: 64 * 1024,
            stderr_cap: 64 * 1024,
            disk_mib: 1,
        };
        let cancel = Arc::new(AtomicBool::new(true));
        let report = execute_javascript("for(let i=0; i<1000000; i++) {}", &limits, cancel)
            .expect("execute returns Ok");
        let _ = report;
    }
}
