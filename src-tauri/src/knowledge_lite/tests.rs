// SPDX-License-Identifier: MIT
//! Behavioural tests for `knowledge_lite`.

#![cfg(test)]

use crate::document_parse::parse_file;
use crate::error::AppError;
use rusqlite::params;
use std::io::Write;

use super::chunk::chunk_text;
use super::commands::knowledge_pro_teaser_count;
use super::ingest::ingest_one;
use super::schema::{with_conn, CONN};
use super::search::{hybrid_search, make_snippet};

fn isolated_home() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::env::set_var("IMPFORGE_APP_HOME", dir.path());
    // Reset singleton so each test gets a fresh DB.
    if let Ok(mut g) = CONN.lock() {
        *g = None;
    }
    dir
}

fn write_temp(suffix: &str, content: &[u8]) -> tempfile::NamedTempFile {
    let tmp = tempfile::NamedTempFile::with_suffix(suffix).expect("tmp");
    tmp.as_file().write_all(content).expect("write");
    tmp.as_file().sync_all().expect("sync");
    tmp
}

#[test]
fn chunk_text_emits_at_least_one_chunk_for_short_text() {
    let chunks = chunk_text("Just a short snippet.");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].line_start, 1);
}

#[test]
fn chunk_text_creates_overlapping_chunks_for_long_text() {
    let line = "This is a sentence with several words. ";
    let big = line.repeat(200); // ~7800 chars → multiple chunks
    let chunks = chunk_text(&big);
    assert!(chunks.len() >= 4, "got {} chunks", chunks.len());
    // Adjacent chunks must share text (overlap).
    if chunks.len() >= 2 {
        let tail = &chunks[0].text[chunks[0].text.len().saturating_sub(50)..];
        let head = &chunks[1].text[..chunks[1].text.len().min(200)];
        // Some of the overlap window should be present in chunk 1's
        // head.  Very loose assertion — just guarantees overlap > 0.
        let overlap_evidence = tail
            .split_whitespace()
            .any(|tok| head.contains(tok) && tok.len() > 4);
        assert!(overlap_evidence, "expected overlap evidence in adjacent chunks");
    }
}

#[test]
fn chunk_text_empty_input_returns_no_chunks() {
    assert!(chunk_text("").is_empty());
    assert!(chunk_text("   \n\n  \n").is_empty());
}

#[test]
fn ingest_real_text_file_creates_doc_and_searchable_chunks() {
    let _home = isolated_home();
    let tmp = write_temp(
        ".txt",
        b"The quick brown fox jumps over the lazy dog.\n\
          Reciprocal rank fusion is the moat that ImpForge ships free.\n\
          Hybrid retrieval merges porter and trigram FTS5 rankings.",
    );
    let parse = parse_file(tmp.path()).expect("parse");
    let outcome = with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");
    assert!(!outcome.skipped_duplicate);
    assert!(outcome.chunk_count >= 1);

    let hits = with_conn(|c| hybrid_search(c, "fox", 10)).expect("search");
    assert!(!hits.is_empty(), "expected at least one fox hit");
    assert!(hits[0].snippet.to_lowercase().contains("fox"));
    assert!(hits[0].entry.title.is_empty() || hits[0].entry.source.contains(".txt"));
}

#[test]
fn ingest_dedups_by_hash() {
    let _home = isolated_home();
    let tmp = write_temp(".txt", b"deduplicate this exact content");
    let parse = parse_file(tmp.path()).expect("parse");
    let first = with_conn(|c| ingest_one(c, tmp.path(), parse.clone())).expect("first");
    assert!(!first.skipped_duplicate);
    let second = with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("second");
    assert!(second.skipped_duplicate);
    assert_eq!(first.doc_id, second.doc_id);
}

#[test]
fn rrf_fusion_ranks_doc_present_in_both_indexes_higher() {
    let _home = isolated_home();
    // Doc A: contains "rust" once.
    let f1 = write_temp(
        ".txt",
        b"rust programming is good.  some other text.",
    );
    // Doc B: contains "rust" three times AND a partial word "rusty".
    let f2 = write_temp(
        ".txt",
        b"rust rust rust is dominant.  this rusty hammer matters too.",
    );
    let p1 = parse_file(f1.path()).expect("p1");
    let p2 = parse_file(f2.path()).expect("p2");
    with_conn(|c| ingest_one(c, f1.path(), p1)).expect("ingest 1");
    with_conn(|c| ingest_one(c, f2.path(), p2)).expect("ingest 2");

    let hits = with_conn(|c| hybrid_search(c, "rust", 10)).expect("search");
    assert!(!hits.is_empty(), "expected at least one hit");
    // RRF should rank doc B (with multiple rust occurrences) ahead.
    let top_path = &hits[0].entry.source;
    let f2_path = f2
        .path()
        .canonicalize()
        .expect("canon")
        .to_string_lossy()
        .to_string();
    assert_eq!(top_path, &f2_path, "doc with 3x rust should rank top");
}

#[test]
fn search_returns_empty_for_unknown_term() {
    let _home = isolated_home();
    let tmp = write_temp(".txt", b"basic content here");
    let parse = parse_file(tmp.path()).expect("parse");
    with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");
    let hits = with_conn(|c| hybrid_search(c, "xyzimpossible1234", 10)).expect("search");
    assert!(hits.is_empty());
}

#[test]
fn search_handles_german_umlauts_via_trigram() {
    let _home = isolated_home();
    let tmp = write_temp(
        ".txt",
        "Über die Bedeutung der Künstlichen Intelligenz heute.\n\
         Wir schätzen Privatsphäre und Datenschutz mehr als alles.\n\
         Diese Datei testet ä ö ü ß im FTS5 Trigramm-Tokenisierer."
            .as_bytes(),
    );
    let parse = parse_file(tmp.path()).expect("parse");
    with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");

    // Trigram tokeniser should match on partial Umlaute strings.
    // The porter tokeniser would butcher 'Künstlich' → 'kunstlich',
    // but trigram's character n-grams match either way.
    let hits = with_conn(|c| hybrid_search(c, "Künstlich", 10)).expect("search");
    assert!(!hits.is_empty(), "expected umlaut hit via trigram");
}

#[test]
fn delete_doc_removes_from_both_tables() {
    let _home = isolated_home();
    let tmp = write_temp(".txt", b"to be deleted soon");
    let parse = parse_file(tmp.path()).expect("parse");
    let outcome = with_conn(|c| ingest_one(c, tmp.path(), parse)).expect("ingest");

    // Verify it's there.
    let hits = with_conn(|c| hybrid_search(c, "deleted", 10)).expect("pre");
    assert!(!hits.is_empty());

    with_conn(|c| {
        c.execute("DELETE FROM chunks_porter WHERE doc_id = ?1", params![outcome.doc_id])
            .map_err(|e| AppError::Internal(e.to_string()))?;
        c.execute("DELETE FROM chunks_trigram WHERE doc_id = ?1", params![outcome.doc_id])
            .map_err(|e| AppError::Internal(e.to_string()))?;
        c.execute("DELETE FROM documents WHERE id = ?1", params![outcome.doc_id])
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    })
    .expect("delete");

    let hits = with_conn(|c| hybrid_search(c, "deleted", 10)).expect("post");
    assert!(hits.is_empty(), "post-delete must be empty");
}

#[test]
fn pro_teaser_count_is_zero_at_mit_tier() {
    let _home = isolated_home();
    // Synchronous wrapper for the async tauri command.
    let count = futures::executor::block_on(knowledge_pro_teaser_count("anything".into()))
        .expect("teaser");
    assert_eq!(count, 0);
}

#[test]
fn make_snippet_truncates_long_text() {
    let long = "word ".repeat(200);
    let snip = make_snippet(&long, 50);
    assert!(snip.chars().count() <= 51);
    assert!(snip.ends_with('…'));
}

#[test]
fn make_snippet_keeps_short_text_intact() {
    let snip = make_snippet("short", 240);
    assert_eq!(snip, "short");
}
