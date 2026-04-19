// SPDX-License-Identifier: MIT
//! HyperChat Lite — chat / edit / agent mode machine, event stream, and session state.
//!
//! MIT-rewritten per `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md`
//! §4.2 SHIP-DEGRADED #2.  Pro creator/monaco/xterm bridges removed.
//!
//! Phase 2: real mode-machine + MPSC event bus + memory-only session manager.
//!
//! ## Module map
//!
//! | File              | Purpose                                                           |
//! |-------------------|-------------------------------------------------------------------|
//! | `modes.rs`        | Chat / Edit / Agent finite-state machine + transition rules       |
//! | `event_stream.rs` | Process-wide MPSC event bus + subscription stats                  |
//! | `session.rs`      | Memory-only session container + capped event ring buffer          |
//! | `commands.rs`     | Tauri command surface (3 commands, Crown-Jewel split)             |

pub mod commands;
pub mod event_stream;
pub mod modes;
pub mod session;

pub use event_stream::{hub, Event, EventHub, EventStreamStats, EventSubscription};
pub use modes::{transition, HyperChatMode, ModeState, ModeTransition, TransitionError};
pub use session::{HyperChatSession, MAX_RESIDENT_EVENTS};

// Re-export the Tauri command surface so external callers (and the
// existing handler list in `lib.rs`) can keep using
// `hyperchat_lite::hyperchat_*` paths.
pub use commands::{hyperchat_event_stats, hyperchat_mode_transition, hyperchat_session_new};
