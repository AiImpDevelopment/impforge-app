// SPDX-License-Identifier: MIT
//! MCP plugin marketplace for impforge-app (Feature 4 Tier 2).
//!
//! Module layout (per `docs/research/2026-04-19-mcp-plugin-ui.md` §D.2):
//!
//! ```text
//! mcp/
//! ├── mod.rs               (this file — module root + re-exports)
//! ├── types.rs             (Server / Manifest / Capability / TrustScore types)
//! ├── marketplace.rs       (browse, search, mirror sync, offline cache)
//! ├── installer.rs         (install, uninstall, version pin)
//! ├── runner.rs            (stdio transport + JSON-RPC client)
//! ├── health.rs            (latency / error-rate tracking)
//! ├── ledger.rs            (tamper-evident SQLite ledger of every invocation)
//! ├── oauth.rs             (PKCE + loopback redirect for OAuth-required servers)
//! ├── sandbox.rs           (Linux Landlock + seccomp profiles, best-effort)
//! ├── capability_diff.rs   (rug-pull defense per ETDI arXiv 2506.01333)
//! └── bundle.rs            (5 starter bundle templates)
//! ```
//!
//! ## Bridge architecture (REGEL 000-BRIDGE-NOT-PROCESS)
//!
//! The App orchestrates discovery + install + audit on the user's
//! machine.  The App **never** routes a tool call through any
//! impforge.com endpoint — every invocation goes user-app → user-MCP-server
//! directly.  Marketplace mirror sync uses one read-only HTTP GET against
//! the public CDN (32 MB cap, 10s timeout, anonymous, gzipped).
//!
//! ## Crown-Jewel discipline
//!
//! Every public symbol below is wired into a real Tauri command in
//! `lib.rs`.  Every module declares itself emergent in
//! `module_emergence_lite.rs`.  Zero `.unwrap()` outside tests.

pub mod bundle;
pub mod capability_diff;
pub mod health;
pub mod installer;
pub mod ledger;
pub mod marketplace;
pub mod oauth;
pub mod runner;
pub mod sandbox;
pub mod types;
