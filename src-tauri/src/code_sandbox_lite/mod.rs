// SPDX-License-Identifier: MIT
//! ForgeSandbox — Code Interpreter Sandbox (Feature 5, Tier 2 / App).
//!
//! This commit ships the foundational layer: types, helpers,
//! limiter, budget, filesystem isolation, inspector, streaming,
//! audit chain.  Runtime + language modules + Tauri command
//! facade land in subsequent commits.
//!
//! ## Cybercrime liability defence (CANONICAL)
//!
//! User-supplied code runs in a wasmtime sandbox ON THE USER'S
//! MACHINE.  Under the **Microsoft Word principle**, ImpForge ships
//! the tool; the user is the deployer of their code under EU AI Act
//! Art 26.  StGB §202c liability is shielded by sandbox isolation,
//! fuel limit, memory cap, explicit preopens, Trust Levels and
//! Merkle audit chain.  No code ever leaves the user's machine.

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
