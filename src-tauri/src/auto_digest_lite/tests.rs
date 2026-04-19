// SPDX-License-Identifier: MIT
//! Behavioural tests for `auto_digest_lite`.  Lives in its own
//! sub-module so it can borrow private helpers via `super::*` while
//! the directory split keeps the production code tree tidy.

#![cfg(test)]

use super::feed::{parse_feed, strip_html};
use super::helpers::{in_window, sanitize_for_path, scrub_for_ingest};
use super::scheduler::{run_once, runtime, TEST_ENV_LOCK};
use super::state::{
    add_clipboard, add_feed, add_folder, append_history, compute_stats, load_state,
    save_state,
};
use super::types::{
    DigestEntry, DigestSource, DigestState, DEBOUNCER_TIMEOUT, DEFAULT_QUIET_HOURS,
    MIN_POLL_SECS,
};
use super::watcher::{ingest_folder_once, install_folder_watchers};
use std::path::PathBuf;
use std::time::Duration;

struct HomeGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
}

fn isolated_home() -> HomeGuard {
    let guard = match TEST_ENV_LOCK.lock() {
        Ok(g) => g,
        Err(p) => {
            TEST_ENV_LOCK.clear_poison();
            p.into_inner()
        }
    };
    let dir = tempfile::tempdir().expect("tempdir");
    std::env::set_var("IMPFORGE_APP_HOME", dir.path());
    // Force RUNTIME to drop any cross-test in-memory state.
    let _ = runtime().reload_from_disk();
    HomeGuard {
        _lock: guard,
        _dir: dir,
    }
}

#[test]
fn default_state_is_safe() {
    let _h = isolated_home();
    let state = load_state().expect("load");
    assert!(!state.paused);
    assert!(state.sources.is_empty());
    assert!(state.quiet_hours_enabled);
    assert_eq!(state.quiet_hours_start, DEFAULT_QUIET_HOURS.0);
}

#[test]
fn add_feed_validates_scheme() {
    let _h = isolated_home();
    let mut state = load_state().expect("load");
    assert!(add_feed(&mut state, "ftp://example.com/feed", 60).is_err());
    assert!(add_feed(&mut state, "not a url", 60).is_err());
}

#[test]
fn add_feed_clamps_interval_below_minimum() {
    let _h = isolated_home();
    let mut state = load_state().expect("load");
    let src = add_feed(&mut state, "https://example.com/rss", 5).expect("add");
    if let DigestSource::Feed { interval_secs, .. } = src {
        assert_eq!(interval_secs, MIN_POLL_SECS);
    } else {
        panic!("expected Feed");
    }
}

#[test]
fn add_folder_requires_existing_dir() {
    let _h = isolated_home();
    let mut state = load_state().expect("load");
    let bad = add_folder(
        &mut state,
        PathBuf::from("/nonexistent/missing/missing"),
        false,
        vec![],
    );
    assert!(bad.is_err());
}

#[test]
fn add_clipboard_replaces_previous() {
    let _h = isolated_home();
    let mut state = load_state().expect("load");
    let _a = add_clipboard(&mut state).expect("first");
    let _b = add_clipboard(&mut state).expect("second");
    let count = state
        .sources
        .iter()
        .filter(|s| matches!(s, DigestSource::Clipboard { .. }))
        .count();
    assert_eq!(count, 1, "second clipboard registration must replace first");
}

#[test]
fn quiet_hours_window_simple() {
    assert!(in_window(23, 22, 8));
    assert!(in_window(2, 22, 8));
    assert!(!in_window(15, 22, 8));
    assert!(in_window(10, 9, 17));
    assert!(!in_window(8, 9, 17));
}

#[test]
fn parse_feed_atom_works() {
    let atom = br#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>X</title><id>urn:x</id>
  <updated>2026-04-19T00:00:00Z</updated>
  <entry>
    <id>urn:e1</id><updated>2026-04-19T00:00:00Z</updated>
    <title>Hello</title><summary>World</summary>
  </entry>
</feed>"#;
    let entries = parse_feed(atom).expect("parse");
    assert_eq!(entries.len(), 1);
}

#[test]
fn strip_html_collapses() {
    assert_eq!(strip_html("<p>Hi  there</p>"), "Hi there");
    assert_eq!(strip_html(""), "");
}

#[test]
fn append_history_then_tail() {
    let _h = isolated_home();
    for i in 0..3 {
        append_history(&DigestEntry {
            source_id: format!("s{i}"),
            kind: "test".into(),
            title: format!("t{i}"),
            url_or_path: format!("u{i}"),
            fetched_at: chrono::Utc::now(),
            pii_redactions: i as u64,
            bytes: 0,
        })
        .expect("append");
    }
    let rows = super::state::tail_history(2).expect("tail");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source_id, "s2");
}

#[test]
fn folder_watcher_detects_new_file_app() {
    // Behavioural test: install watcher, drop file, expect event.
    let _h = isolated_home();
    let tmp = tempfile::tempdir().expect("tempdir watcher");
    let mut state = load_state().expect("load");
    add_folder(
        &mut state,
        tmp.path().to_path_buf(),
        false,
        vec!["txt".into()],
    )
    .expect("add folder");
    save_state(&state).expect("save");

    let (_watchers, rx) = install_folder_watchers(&state).expect("install");
    std::thread::sleep(Duration::from_millis(150));

    std::fs::write(tmp.path().join("hello.txt"), b"hi").expect("write");

    let received = rx.recv_timeout(DEBOUNCER_TIMEOUT + Duration::from_secs(2));
    assert!(received.is_ok(), "watcher must observe new file");
}

#[test]
fn rss_scheduler_clamps_interval() {
    // Behavioural test: feed added with interval=10 must be
    // persisted with interval=MIN_POLL_SECS so the daemon doesn't
    // poll faster than once a minute (Recall lesson #1).
    let _h = isolated_home();
    let mut state = load_state().expect("load");
    let _src =
        add_feed(&mut state, "https://example.com/rss", 10).expect("add");
    save_state(&state).expect("save");

    let reload = load_state().expect("reload");
    if let DigestSource::Feed { interval_secs, .. } = &reload.sources[0] {
        assert_eq!(*interval_secs, MIN_POLL_SECS);
    } else {
        panic!("expected Feed");
    }
}

#[test]
fn run_once_errors_when_paused() {
    let _h = isolated_home();
    let mut state = DigestState {
        paused: true,
        ..Default::default()
    };
    let res = futures::executor::block_on(run_once(&mut state));
    assert!(res.is_err(), "run_once must refuse when paused");
}

#[test]
fn compute_stats_counts_each_source_type() {
    let _h = isolated_home();
    let tmp = tempfile::tempdir().expect("tempdir for stats");
    let mut state = load_state().expect("load");
    let _ = add_feed(&mut state, "https://example.com/a", 60);
    let _ = add_folder(&mut state, tmp.path().to_path_buf(), false, vec![]);
    let _ = add_clipboard(&mut state);
    let stats = compute_stats(&state).expect("stats");
    assert_eq!(stats.feed_count, 1);
    assert_eq!(stats.folder_count, 1);
    assert!(stats.clipboard_active);
}

#[test]
fn scrub_for_ingest_passes_through_when_no_pii() {
    // PII scrubber fallthrough — even with the stubbed scrubber
    // the function returns text + 0 redactions.
    let (out, n) = scrub_for_ingest("hello world");
    assert_eq!(out, "hello world");
    assert_eq!(n, 0);
}

#[test]
fn sanitize_for_path_safe() {
    let s = sanitize_for_path("https://example.com/feed?x=1");
    assert!(s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
}

#[test]
fn ingest_folder_once_picks_up_files_app() {
    let _h = isolated_home();
    let tmp = tempfile::tempdir().expect("tempdir ingest");
    std::fs::write(
        tmp.path().join("note.md"),
        "# Hi\n\nMore body to chunk.",
    )
    .expect("write");

    let src = DigestSource::Folder {
        id: "f-test".into(),
        path: tmp.path().canonicalize().expect("canon"),
        recursive: false,
        allow_ext: vec![],
    };
    let (files, _chunks) = ingest_folder_once(&src).expect("ingest");
    assert!(files >= 1);
}
