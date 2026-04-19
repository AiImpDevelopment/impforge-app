// SPDX-License-Identifier: MIT
//! Slash-command catalog + dispatcher. MIT-rewritten per
//! `docs/superpowers/specs/2026-04-19-hyperchat-freemium-split-design.md` §4.2 SHIP-DEGRADED #3.
//!
//! Phase 2: static catalog (~50 commands across 16 categories) + dispatcher.
//! v0.5+ may grow toward 200; Pro `intent_hub` ships 50K patterns + fuzzy +
//! clustering — those stay Pro-side.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// One registered slash command (e.g. `/help`, `/clear`).
///
/// Owns its strings (not `&'static str`) so the catalog can be returned
/// across the Tauri IPC boundary as JSON without lifetimes leaking out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub category: String,
}

/// Result of dispatching a slash command.
///
/// Internally-tagged enum (`tag = "kind"`); newtype variants are expressed as
/// struct variants because serde-json cannot embed a primitive into the tagged
/// object form (Plan #1 Task 7 deviation).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlashOutcome {
    /// Command produced direct text output (e.g. help).
    Text { text: String },
    /// Command resolved to an intent for the host to handle.
    Intent { name: String, arg: Option<String> },
    /// Input did not match a known slash command.
    Unknown { input: String },
}

/// Helper to keep the catalog literal short and consistent.
fn cmd(name: &str, description: &str, category: &str) -> SlashCommand {
    SlashCommand {
        name: name.to_string(),
        description: description.to_string(),
        category: category.to_string(),
    }
}

/// Return the static catalog of slash commands shipping in v0.2.0.
///
/// 50 commands across 16 categories. Order is grouped by category for
/// readability of `/help` output.
pub fn catalog() -> Vec<SlashCommand> {
    vec![
        // chat (5)
        cmd("/help", "Show help", "chat"),
        cmd("/clear", "Clear current thread", "chat"),
        cmd("/new", "New thread", "chat"),
        cmd("/list", "List threads", "chat"),
        cmd("/switch", "Switch thread", "chat"),
        // knowledge (5)
        cmd("/search", "Search SQLite knowledge base", "knowledge"),
        cmd("/wiki", "Fetch Wikipedia article", "knowledge"),
        cmd("/notes", "Add to notes", "knowledge"),
        cmd("/digest", "Show recent auto-digest items", "knowledge"),
        cmd("/recall", "Search memory", "knowledge"),
        // system (6)
        cmd("/model", "Switch Ollama model", "system"),
        cmd("/ollama", "Direct Ollama prompt", "system"),
        cmd("/settings", "Open settings", "system"),
        cmd("/about", "About ImpForge", "system"),
        cmd("/version", "Show version", "system"),
        cmd("/quit", "Quit ImpForge", "system"),
        // privacy (2)
        cmd("/privacy", "Set privacy tier", "privacy"),
        cmd("/scrub", "PII-scrub clipboard", "privacy"),
        // files (3)
        cmd("/import", "Import a file", "files"),
        cmd("/parse", "Parse a document", "files"),
        cmd("/watch", "Watch a folder for changes", "files"),
        // pro (2)
        cmd("/pro", "Open Pro upgrade page", "pro"),
        cmd("/upgrade", "Open Pro upgrade page (alias)", "pro"),
        // digiimp (4)
        cmd("/digiimp", "Toggle DigiImp visibility", "digiimp"),
        cmd("/rest", "DigiImp rest mode", "digiimp"),
        cmd("/wake", "Wake DigiImp", "digiimp"),
        cmd("/adventure", "Trigger DigiImp Adventure", "digiimp"),
        // auto-digest (2)
        cmd("/rss", "Add RSS feed", "auto-digest"),
        cmd("/feeds", "List RSS feeds", "auto-digest"),
        // format (3)
        cmd("/code", "Format as code block", "format"),
        cmd("/quote", "Format as quote", "format"),
        cmd("/list-md", "Format as markdown list", "format"),
        // browser (2)
        cmd("/bookmarks", "Import browser bookmarks", "browser"),
        cmd("/history", "Import browser history", "browser"),
        // mcp (2)
        cmd("/mcp", "List MCP servers", "mcp"),
        cmd("/tool", "Run an MCP tool", "mcp"),
        // widgets (3)
        cmd("/widgets", "List widgets (Clock/Notes/Monitor)", "widgets"),
        cmd("/clock", "Open clock widget", "widgets"),
        cmd("/monitor", "Open system monitor", "widgets"),
        // quality (1)
        cmd("/lint", "Run Crown-Jewel guardian on a file", "quality"),
        // settings (3)
        cmd("/theme", "Cycle DigiImp color preset", "settings"),
        cmd("/sound", "Toggle DigiImp sounds", "settings"),
        cmd("/motion", "Toggle reduced motion", "settings"),
        // fun (2)
        cmd("/joke", "Tell a joke (local Ollama)", "fun"),
        cmd("/poem", "Write a haiku", "fun"),
        // ai (6)
        cmd("/translate", "Translate text", "ai"),
        cmd("/summarize", "Summarize text", "ai"),
        cmd("/explain", "Explain text", "ai"),
        cmd("/refactor", "Suggest refactoring", "ai"),
        cmd("/test", "Generate tests", "ai"),
        cmd("/doc", "Generate docstring", "ai"),
    ]
}

/// Dispatch a slash-command input.
///
/// - Input not starting with `/` -> `Unknown` (caller can route to chat)
/// - `/help` -> `Text` enumerating the catalog
/// - Known command -> `Intent { name, arg }` for the host to handle
/// - Unknown `/...` -> `Unknown { input }`
pub fn dispatch(input: String, _thread_id: Option<String>) -> SlashOutcome {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return SlashOutcome::Unknown { input };
    }

    let (name, arg) = match trimmed.split_once(' ') {
        Some((n, a)) => {
            let a = a.trim();
            (
                n.to_string(),
                if a.is_empty() {
                    None
                } else {
                    Some(a.to_string())
                },
            )
        }
        None => (trimmed.to_string(), None),
    };

    if name == "/help" {
        let mut lines = String::from("Available commands:\n");
        for c in catalog() {
            lines.push_str(&format!("  {} -- {}\n", c.name, c.description));
        }
        return SlashOutcome::Text { text: lines };
    }

    if catalog().iter().any(|c| c.name == name) {
        return SlashOutcome::Intent { name, arg };
    }

    SlashOutcome::Unknown { input }
}

/// Tauri: return the catalog of all registered slash commands.
#[tauri::command]
pub async fn slash_catalog() -> AppResult<Vec<SlashCommand>> {
    Ok(catalog())
}

/// Tauri: dispatch a slash-command input string in the context of an optional thread.
#[tauri::command]
pub async fn slash_dispatch(input: String, thread_id: Option<String>) -> AppResult<SlashOutcome> {
    if input.is_empty() {
        return Err(AppError::InvalidArgument(
            "slash dispatch input may not be empty".into(),
        ));
    }
    Ok(dispatch(input, thread_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_chat_text_returns_unknown() {
        let r = dispatch("just a message".into(), None);
        assert!(matches!(r, SlashOutcome::Unknown { .. }));
    }

    #[test]
    fn dispatch_strips_leading_whitespace() {
        let r = dispatch("   /clear   ".into(), None);
        assert!(matches!(r, SlashOutcome::Intent { ref name, .. } if name == "/clear"));
    }

    #[test]
    fn dispatch_extracts_args_after_first_space() {
        let r = dispatch("/search hello world".into(), None);
        match r {
            SlashOutcome::Intent { name, arg } => {
                assert_eq!(name, "/search");
                assert_eq!(arg.as_deref(), Some("hello world"));
            }
            other => panic!("expected Intent, got {other:?}"),
        }
    }
}
