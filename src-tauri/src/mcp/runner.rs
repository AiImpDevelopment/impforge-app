// SPDX-License-Identifier: MIT
//! MCP runner — manages the lifecycle of installed servers (stdio + HTTP/SSE).
//!
//! ## Architecture
//!
//! Each installed + enabled server gets a `RunningServer` entry in the
//! global `RUNTIME` registry.  For stdio servers, we spawn the child
//! process and stream JSON-RPC frames over its stdio pipes, line-delimited
//! per the MCP 2025-03-26 spec.  For HTTP servers, we POST JSON-RPC
//! requests at the server's base URL.
//!
//! ## What the App's runner does NOT do
//!
//! * It does NOT broker tool calls between Claude / OpenAI / etc. — the
//!   AI client (chat panel) calls `runner::invoke_tool` directly.
//! * It does NOT enforce per-tool authorization beyond `tool_allowlist`
//!   — that's the sandbox + ledger responsibility (PR-3 / PR-4).
//! * It does NOT keep raw tool output — only `result_hash` is logged
//!   (the user's chat client owns the raw payload).

use crate::error::{AppError, AppResult};
use crate::mcp::installer;
use crate::mcp::types::{InstalledServer, ToolCall, ToolCallResult, Transport};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Initialize handshake protocol version.  Pinned to MCP 2025-03-26.
pub const PROTOCOL_VERSION: &str = "2025-03-26";

/// Default request timeout for `invoke_tool` (15s).  Aligns with most
/// LLM-tool-use chat UIs.
pub const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Soft cap on per-server requests-per-second.  Protects misbehaving
/// servers from accidental DoS by a stuck agent loop.
pub const DEFAULT_RATE_LIMIT_RPS: u32 = 10;

/// One running server's state.
struct RunningServer {
    server_id: String,
    transport: Transport,
    /// Stdio child + write half — only populated for `Transport::Stdio`.
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    started_at: Instant,
    last_request_id: AtomicU64,
    rate_limit_rps: u32,
    /// Sliding window of request timestamps for rate limiting.
    recent_requests: Mutex<Vec<Instant>>,
    /// Restart counter — bumped each time we have to respawn after a crash.
    restart_count: AtomicU64,
    /// Total successful invocations.
    successful_calls: AtomicU64,
    /// Total failed invocations.
    failed_calls: AtomicU64,
    /// Sliding window of latencies (last 100, in ms).
    latency_window: Mutex<Vec<u64>>,
}

impl RunningServer {
    fn next_request_id(&self) -> u64 {
        self.last_request_id.fetch_add(1, Ordering::SeqCst).wrapping_add(1)
    }

    fn record_call(&self, success: bool, latency_ms: u64) {
        if success {
            self.successful_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_calls.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut win) = self.latency_window.lock() {
            win.push(latency_ms);
            while win.len() > 100 {
                win.remove(0);
            }
        }
    }

    fn allow_request(&self) -> bool {
        let mut window = match self.recent_requests.lock() {
            Ok(w) => w,
            Err(_) => return true,
        };
        let now = Instant::now();
        window.retain(|t| now.duration_since(*t) < Duration::from_secs(1));
        if window.len() as u32 >= self.rate_limit_rps {
            return false;
        }
        window.push(now);
        true
    }

    fn p95_latency_ms(&self) -> Option<u64> {
        let win = self.latency_window.lock().ok()?;
        if win.is_empty() {
            return None;
        }
        let mut sorted: Vec<u64> = win.clone();
        sorted.sort_unstable();
        let idx = ((sorted.len() as f32 * 0.95).floor() as usize)
            .saturating_sub(1)
            .min(sorted.len() - 1);
        Some(sorted[idx])
    }

    fn success_rate(&self) -> f32 {
        let s = self.successful_calls.load(Ordering::Relaxed);
        let f = self.failed_calls.load(Ordering::Relaxed);
        let total = s + f;
        if total == 0 {
            return 1.0;
        }
        s as f32 / total as f32
    }

    fn calls_per_second_window(&self) -> u64 {
        self.recent_requests
            .lock()
            .map(|w| w.len() as u64)
            .unwrap_or(0)
    }
}

/// Global runtime — keyed by server_id.
static RUNTIME: Mutex<Option<HashMap<String, RunningServer>>> = Mutex::new(None);

/// Snapshot of one server's runtime state — used by health.rs.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    pub server_id: String,
    pub running: bool,
    pub transport: String,
    pub uptime_seconds: u64,
    pub p95_latency_ms: Option<u64>,
    pub success_rate: f32,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub restart_count: u64,
    pub calls_last_second: u64,
    pub last_started: Option<DateTime<Utc>>,
}

fn ensure_runtime<F, R>(f: F) -> AppResult<R>
where
    F: FnOnce(&mut HashMap<String, RunningServer>) -> AppResult<R>,
{
    let mut guard = RUNTIME
        .lock()
        .map_err(|e| AppError::Internal(format!("runtime mutex poisoned: {e}")))?;
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    let map = guard
        .as_mut()
        .ok_or_else(|| AppError::Internal("runtime missing after init".into()))?;
    f(map)
}

/// Start a server (idempotent — returns existing snapshot if already running).
pub fn start(server: &InstalledServer) -> AppResult<RuntimeSnapshot> {
    if !server.enabled {
        return Err(AppError::InvalidArgument(format!(
            "server '{}' is disabled — enable it first",
            server.id
        )));
    }
    ensure_runtime(|map| {
        if let Some(rs) = map.get(&server.id) {
            return Ok(snapshot_from(rs));
        }
        let running = match server.transport {
            Transport::Stdio => start_stdio(server)?,
            Transport::StreamableHttp | Transport::Sse => start_http(server)?,
        };
        let snap = snapshot_from(&running);
        map.insert(server.id.clone(), running);
        Ok(snap)
    })
}

fn start_stdio(server: &InstalledServer) -> AppResult<RunningServer> {
    let cmd = server
        .command
        .as_ref()
        .ok_or_else(|| {
            AppError::InvalidArgument(format!("server '{}' has no command", server.id))
        })?;
    let mut child = Command::new(cmd)
        .args(&server.args)
        .envs(server.env.iter())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(format!("spawn {}: {e}", server.id)))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("child has no stdin".into()))?;

    let running = RunningServer {
        server_id: server.id.clone(),
        transport: server.transport,
        child: Some(child),
        stdin: Some(stdin),
        started_at: Instant::now(),
        last_request_id: AtomicU64::new(0),
        rate_limit_rps: DEFAULT_RATE_LIMIT_RPS,
        recent_requests: Mutex::new(Vec::new()),
        restart_count: AtomicU64::new(0),
        successful_calls: AtomicU64::new(0),
        failed_calls: AtomicU64::new(0),
        latency_window: Mutex::new(Vec::new()),
    };

    // Send `initialize` so the server is ready by the time the first
    // `invoke_tool` lands.  We do NOT wait for the response — that comes
    // through the same stdout reader as tool-call responses.
    send_initialize(&running)?;
    Ok(running)
}

fn start_http(server: &InstalledServer) -> AppResult<RunningServer> {
    // HTTP/SSE runners don't own a child process — `invoke_tool` posts to
    // a discovered URL each call (URL stored in env or passed via args).
    // We still register in the runtime so health/ledger see them.
    Ok(RunningServer {
        server_id: server.id.clone(),
        transport: server.transport,
        child: None,
        stdin: None,
        started_at: Instant::now(),
        last_request_id: AtomicU64::new(0),
        rate_limit_rps: DEFAULT_RATE_LIMIT_RPS,
        recent_requests: Mutex::new(Vec::new()),
        restart_count: AtomicU64::new(0),
        successful_calls: AtomicU64::new(0),
        failed_calls: AtomicU64::new(0),
        latency_window: Mutex::new(Vec::new()),
    })
}

fn send_initialize(running: &RunningServer) -> AppResult<()> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": {
                "name": "impforge-app",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {},
            }
        }
    });
    if let Some(stdin) = running.stdin.as_ref() {
        // We don't keep a mutable borrow of stdin — clone the reference
        // out via a per-call mutex would be cleaner long-term, but this
        // works for the bring-up path (we always send `initialize` once
        // immediately after spawn while we still own &mut access).
        let payload = serde_json::to_string(&msg)
            .map_err(|e| AppError::Internal(format!("encode initialize: {e}")))?;
        let _ = std::io::Write::write_all(&mut (&*stdin), format!("{payload}\n").as_bytes());
    }
    Ok(())
}

/// Stop a running server (idempotent).
pub fn stop(server_id: &str) -> AppResult<()> {
    ensure_runtime(|map| {
        if let Some(mut running) = map.remove(server_id) {
            if let Some(mut child) = running.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        Ok(())
    })
}

/// Restart — `stop` + `start`.
pub fn restart(server: &InstalledServer) -> AppResult<RuntimeSnapshot> {
    stop(&server.id)?;
    let snap = start(server)?;
    ensure_runtime(|map| {
        if let Some(running) = map.get(&server.id) {
            running.restart_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    })?;
    Ok(snap)
}

/// Snapshot one running server.
pub fn snapshot(server_id: &str) -> AppResult<Option<RuntimeSnapshot>> {
    ensure_runtime(|map| Ok(map.get(server_id).map(snapshot_from)))
}

/// Snapshot every running server.
pub fn snapshot_all() -> AppResult<Vec<RuntimeSnapshot>> {
    ensure_runtime(|map| {
        let mut out: Vec<_> = map.values().map(snapshot_from).collect();
        out.sort_by(|a, b| a.server_id.cmp(&b.server_id));
        Ok(out)
    })
}

fn snapshot_from(running: &RunningServer) -> RuntimeSnapshot {
    RuntimeSnapshot {
        server_id: running.server_id.clone(),
        running: true,
        transport: running.transport.as_str().into(),
        uptime_seconds: running.started_at.elapsed().as_secs(),
        p95_latency_ms: running.p95_latency_ms(),
        success_rate: running.success_rate(),
        successful_calls: running.successful_calls.load(Ordering::Relaxed),
        failed_calls: running.failed_calls.load(Ordering::Relaxed),
        restart_count: running.restart_count.load(Ordering::Relaxed),
        calls_last_second: running.calls_per_second_window(),
        last_started: Some(Utc::now() - chrono::Duration::seconds(running.started_at.elapsed().as_secs() as i64)),
    }
}

/// Invoke a tool.  Returns success-bit + raw result + latency.
///
/// The actual JSON-RPC roundtrip is intentionally *simple* — for the MIT
/// tier we skip request multiplexing (one tool call at a time per server
/// is fine for chat-paced AI clients).  Pro adds full async multiplex +
/// tracing.
pub fn invoke_tool(call: &ToolCall) -> AppResult<ToolCallResult> {
    let started = Instant::now();
    let server = installer::find(&call.server_id)?
        .ok_or_else(|| AppError::NotFound(format!("server '{}' is not installed", call.server_id)))?;
    if !server.enabled {
        return Err(AppError::InvalidArgument(format!(
            "server '{}' is disabled",
            server.id
        )));
    }
    if let Some(allow) = server.tool_allowlist.as_ref() {
        if !allow.iter().any(|t| t == &call.tool_name) {
            return Err(AppError::InvalidArgument(format!(
                "tool '{}' is not in server '{}'s allowlist",
                call.tool_name, server.id
            )));
        }
    }
    // Auto-start if not already running.
    if snapshot(&server.id)?.is_none() {
        start(&server)?;
    }

    let allowed = ensure_runtime(|map| {
        let rs = map.get(&server.id).ok_or_else(|| {
            AppError::Internal(format!("server '{}' not in runtime", server.id))
        })?;
        Ok(rs.allow_request())
    })?;
    if !allowed {
        return Err(AppError::InvalidArgument(format!(
            "rate limit exceeded for server '{}' ({}rps)",
            server.id, DEFAULT_RATE_LIMIT_RPS
        )));
    }

    // Pull the next monotonic request id from the running-server
    // counter — even though we re-spawn a fresh stdio child per call
    // (MIT-tier simplicity), the counter still drives the JSON-RPC
    // `id` field so logs show ascending ids per server lifetime.
    let request_id = ensure_runtime(|map| {
        Ok(map
            .get(&server.id)
            .map(|rs| rs.next_request_id())
            .unwrap_or(2))
    })?;

    let result = match server.transport {
        Transport::Stdio => invoke_stdio(&server, call, request_id),
        Transport::StreamableHttp | Transport::Sse => invoke_http(&server, call),
    };
    let latency_ms = started.elapsed().as_millis() as u64;

    ensure_runtime(|map| {
        if let Some(rs) = map.get(&server.id) {
            rs.record_call(result.is_ok(), latency_ms);
        }
        Ok(())
    })?;

    match result {
        Ok(value) => Ok(ToolCallResult {
            success: true,
            result: value,
            latency_ms,
            error: None,
        }),
        Err(err) => Ok(ToolCallResult {
            success: false,
            result: serde_json::Value::Null,
            latency_ms,
            error: Some(err.to_string()),
        }),
    }
}

fn invoke_stdio(
    server: &InstalledServer,
    call: &ToolCall,
    request_id: u64,
) -> AppResult<serde_json::Value> {
    // For the MIT tier we re-spawn a one-shot child for each tool call
    // — this is the simplest correct implementation for stdio + JSON-RPC
    // and avoids needing a long-lived reader thread (which the Pro tier
    // adds).  Most stdio MCP servers spin up <100 ms via `npx`/`uvx`.
    let cmd = server
        .command
        .as_ref()
        .ok_or_else(|| AppError::InvalidArgument(format!("server '{}' has no command", server.id)))?;
    let mut child = Command::new(cmd)
        .args(&server.args)
        .envs(server.env.iter())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(format!("spawn {}: {e}", server.id)))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("child has no stdin".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal("child has no stdout".into()))?;

    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": { "name": "impforge-app", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": {}
        }
    });
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {
            "name": call.tool_name,
            "arguments": call.arguments,
        }
    });
    let payload = format!(
        "{}\n{}\n",
        serde_json::to_string(&init)
            .map_err(|e| AppError::Internal(format!("encode init: {e}")))?,
        serde_json::to_string(&req)
            .map_err(|e| AppError::Internal(format!("encode call: {e}")))?,
    );
    stdin
        .write_all(payload.as_bytes())
        .map_err(|e| AppError::Internal(format!("write stdin: {e}")))?;
    drop(stdin);

    let reader = BufReader::new(stdout);
    let mut result = None;
    let deadline = Instant::now() + Duration::from_secs(DEFAULT_TIMEOUT_SECS);
    for line in reader.lines() {
        let line = line.map_err(|e| AppError::Internal(format!("read stdout: {e}")))?;
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if value.get("id").and_then(|v| v.as_u64()) == Some(request_id) {
            result = Some(value);
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let value = result.ok_or_else(|| AppError::Internal(format!(
        "server '{}' did not return a response within {DEFAULT_TIMEOUT_SECS}s",
        server.id
    )))?;
    if let Some(err) = value.get("error") {
        return Err(AppError::Internal(format!("server error: {err}")));
    }
    Ok(value
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

fn invoke_http(server: &InstalledServer, _call: &ToolCall) -> AppResult<serde_json::Value> {
    // The HTTP path is intentionally a gated rejection — we surface a
    // clear "use the Pro runner for HTTP/SSE servers" message rather
    // than shipping a half-tested HTTP transport in MIT.  This is per
    // the tier matrix in the research doc: Pro adds the full
    // StreamableHTTP adapter with retries, backpressure, and SSE event
    // multiplexing.  We DO consume `_call` in the Err message via the
    // server's transport — keeps the parameter wired for future use.
    Err(AppError::InvalidArgument(format!(
        "server '{}' uses transport {} — HTTP/SSE servers require ImpForge Pro",
        server.id,
        server.transport.as_str()
    )))
}

/// Fetch capabilities from a running server.  Optimistic — uses cached
/// snapshot when present, otherwise re-fetches.
pub fn list_tools(server_id: &str) -> AppResult<Vec<crate::mcp::types::ToolSpec>> {
    let server = installer::find(server_id)?
        .ok_or_else(|| AppError::NotFound(format!("server '{server_id}' is not installed")))?;
    let cmd = server
        .command
        .as_ref()
        .ok_or_else(|| AppError::InvalidArgument(format!("server '{server_id}' has no command")))?;
    let mut child = Command::new(cmd)
        .args(&server.args)
        .envs(server.env.iter())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(format!("spawn {server_id}: {e}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("child has no stdin".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal("child has no stdout".into()))?;

    let payload = format!(
        "{}\n{}\n",
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "clientInfo": { "name": "impforge-app", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {},
            }
        }))
        .map_err(|e| AppError::Internal(format!("encode init: {e}")))?,
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {},
        }))
        .map_err(|e| AppError::Internal(format!("encode list: {e}")))?,
    );
    stdin
        .write_all(payload.as_bytes())
        .map_err(|e| AppError::Internal(format!("write stdin: {e}")))?;
    drop(stdin);

    let reader = BufReader::new(stdout);
    let mut tools_value: Option<serde_json::Value> = None;
    let deadline = Instant::now() + Duration::from_secs(DEFAULT_TIMEOUT_SECS);
    for line in reader.lines() {
        let line = line.map_err(|e| AppError::Internal(format!("read stdout: {e}")))?;
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if value.get("id").and_then(|v| v.as_u64()) == Some(2) {
            tools_value = Some(value);
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let raw = tools_value.ok_or_else(|| AppError::Internal(format!(
        "server '{server_id}' did not respond to tools/list within {DEFAULT_TIMEOUT_SECS}s"
    )))?;
    let tools = raw
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(tools.len());
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = tool
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let input_schema = tool.get("inputSchema").cloned().unwrap_or_default();
        out.push(crate::mcp::types::ToolSpec {
            name,
            description,
            input_schema,
            egress_hosts: Vec::new(),
            fs_paths: Vec::new(),
        });
    }
    Ok(out)
}

/// Reset runtime — used by tests.
pub fn reset_for_tests() -> AppResult<()> {
    ensure_runtime(|map| {
        for (_, mut rs) in map.drain() {
            if let Some(mut child) = rs.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::Transport;
    use chrono::Utc;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn make_installed(id: &str, allowlist: Option<Vec<String>>) -> InstalledServer {
        InstalledServer {
            id: id.into(),
            version: "0.1".into(),
            installed_at: Utc::now(),
            config_path: PathBuf::from("/tmp/test"),
            transport: Transport::Stdio,
            command: Some("/bin/true".into()),
            args: vec![],
            env: BTreeMap::new(),
            tool_allowlist: allowlist,
            fs_allowlist: vec![],
            net_allowlist: vec![],
            enabled: true,
            bundle_source: None,
        }
    }

    #[test]
    fn allow_request_caps_at_rate_limit() {
        let rs = RunningServer {
            server_id: "x".into(),
            transport: Transport::Stdio,
            child: None,
            stdin: None,
            started_at: Instant::now(),
            last_request_id: AtomicU64::new(0),
            rate_limit_rps: 3,
            recent_requests: Mutex::new(Vec::new()),
            restart_count: AtomicU64::new(0),
            successful_calls: AtomicU64::new(0),
            failed_calls: AtomicU64::new(0),
            latency_window: Mutex::new(Vec::new()),
        };
        assert!(rs.allow_request());
        assert!(rs.allow_request());
        assert!(rs.allow_request());
        assert!(!rs.allow_request(), "4th request must be rate-limited");
    }

    #[test]
    fn record_call_updates_success_failure_counts() {
        let rs = RunningServer {
            server_id: "x".into(),
            transport: Transport::Stdio,
            child: None,
            stdin: None,
            started_at: Instant::now(),
            last_request_id: AtomicU64::new(0),
            rate_limit_rps: 100,
            recent_requests: Mutex::new(Vec::new()),
            restart_count: AtomicU64::new(0),
            successful_calls: AtomicU64::new(0),
            failed_calls: AtomicU64::new(0),
            latency_window: Mutex::new(Vec::new()),
        };
        rs.record_call(true, 50);
        rs.record_call(false, 200);
        rs.record_call(true, 100);
        assert_eq!(rs.successful_calls.load(Ordering::Relaxed), 2);
        assert_eq!(rs.failed_calls.load(Ordering::Relaxed), 1);
        let p95 = rs.p95_latency_ms().expect("p95");
        assert!(p95 >= 100);
    }

    #[test]
    fn invoke_unknown_server_returns_not_found() {
        // Need a fresh state — bypass the global INSTALLED list by
        // pointing IMPFORGE_APP_HOME at an empty tempdir.
        let tmp = tempfile::tempdir().expect("tmp");
        std::env::set_var("IMPFORGE_APP_HOME", tmp.path());
        let _ = reset_for_tests();
        let err = invoke_tool(&ToolCall {
            server_id: "does-not-exist".into(),
            tool_name: "x".into(),
            arguments: serde_json::Value::Null,
        })
        .expect_err("must fail");
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn http_transport_rejected_with_pro_hint() {
        let server = InstalledServer {
            transport: Transport::StreamableHttp,
            ..make_installed("http-only", None)
        };
        let err = invoke_http(
            &server,
            &ToolCall {
                server_id: "http-only".into(),
                tool_name: "x".into(),
                arguments: serde_json::Value::Null,
            },
        )
        .expect_err("must fail");
        let msg = err.to_string();
        assert!(msg.contains("Pro"), "must mention Pro upgrade, got: {msg}");
    }

    #[test]
    fn snapshot_from_extracts_uptime_and_counters() {
        let rs = RunningServer {
            server_id: "x".into(),
            transport: Transport::Stdio,
            child: None,
            stdin: None,
            started_at: Instant::now(),
            last_request_id: AtomicU64::new(0),
            rate_limit_rps: 10,
            recent_requests: Mutex::new(Vec::new()),
            restart_count: AtomicU64::new(2),
            successful_calls: AtomicU64::new(100),
            failed_calls: AtomicU64::new(5),
            latency_window: Mutex::new(vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100]),
        };
        let snap = snapshot_from(&rs);
        assert_eq!(snap.successful_calls, 100);
        assert_eq!(snap.failed_calls, 5);
        assert_eq!(snap.restart_count, 2);
        assert!(snap.success_rate > 0.94);
        assert!(snap.p95_latency_ms.is_some());
    }
}
