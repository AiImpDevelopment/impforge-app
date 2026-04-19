// SPDX-License-Identifier: MIT
//! Pure text chunking + line-index utilities.  No I/O.

use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::path::Path;

use super::types::{ChunkSlice, CHUNK_CHARS, CHUNK_OVERLAP_CHARS};

/// Split text into char-bounded chunks with overlap.  Preserves
/// 1-based `(start_line, end_line)` for citation.
///
/// Strategy:
/// 1. Walk the text by `CHUNK_CHARS` chars.
/// 2. Try to back off the boundary to the nearest paragraph break (`\n\n`)
///    so we don't slice a sentence.
/// 3. Emit `CHUNK_OVERLAP_CHARS` of context from the previous chunk into
///    the next — keeps cross-boundary thoughts findable.
pub fn chunk_text(text: &str) -> Vec<ChunkSlice> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<ChunkSlice> = Vec::new();
    let mut start = 0usize;

    // Pre-compute byte→line lookup for citation.  We track 1-based lines.
    let line_at = build_line_index(text);

    while start < chars.len() {
        let mut end = (start + CHUNK_CHARS).min(chars.len());

        // Try to back off to a paragraph break within last 200 chars.
        if end < chars.len() {
            let lookback_min = end.saturating_sub(200);
            if let Some(better) =
                find_paragraph_break(&chars, lookback_min, end)
            {
                end = better;
            }
        }

        let slice: String = chars[start..end].iter().collect();
        let trimmed = slice.trim().to_string();
        if !trimmed.is_empty() {
            let byte_start = byte_offset(&chars, start);
            let byte_end = byte_offset(&chars, end);
            let line_start = lookup_line(&line_at, byte_start);
            let line_end = lookup_line(&line_at, byte_end.saturating_sub(1));
            out.push(ChunkSlice {
                text: trimmed,
                line_start,
                line_end,
            });
        }

        // Advance with overlap.
        if end >= chars.len() {
            break;
        }
        let next = end.saturating_sub(CHUNK_OVERLAP_CHARS);
        start = if next > start { next } else { end };
    }
    out
}

fn build_line_index(text: &str) -> Vec<usize> {
    // Returns ascending byte-offsets where new lines start (the offset
    // immediately AFTER each `\n`).  We add a sentinel 0 so line 1
    // begins at byte 0.
    let mut offsets = vec![0usize];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn lookup_line(line_offsets: &[usize], byte_offset: usize) -> i64 {
    // Binary-search for the largest offset ≤ byte_offset.
    match line_offsets.binary_search(&byte_offset) {
        Ok(idx) => (idx + 1) as i64,
        Err(idx) => idx as i64, // idx is the insertion point; line = idx (1-based starts at sentinel 0)
    }
    .max(1)
}

fn byte_offset(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

fn find_paragraph_break(chars: &[char], min: usize, max: usize) -> Option<usize> {
    if max < 2 {
        return None;
    }
    let limit = max.min(chars.len());
    if min >= limit {
        return None;
    }
    for i in (min..limit - 1).rev() {
        if chars[i] == '\n' && chars[i + 1] == '\n' {
            return Some(i + 2);
        }
    }
    None
}

/// SHA-256 of a file's bytes — used by ingest dedup.
pub(super) fn file_hash(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path).map_err(AppError::Io)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
