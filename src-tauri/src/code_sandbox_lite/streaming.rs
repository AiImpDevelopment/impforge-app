// SPDX-License-Identifier: MIT
//! Chunked streaming buffer — the App-tier substitute for WASIp3
//! `stream<T>` / `future<T>`.
//!
//! Long-running cells produce stdout/stderr in fits and starts; the
//! UI wants chunks as they arrive (think tail -F).  We expose a
//! ring-style buffer the runtime writes into and the UI polls from.
//!
//! Pro tier upgrades to wasmtime 37+ + WASIp3 native streams when
//! the API stabilises — this module's surface stays the same.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::collections::HashMap;

/// One chunk of streaming output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub seq: u64,
    pub stream: super::types::OutputStream,
    pub data: String,
    pub ts_unix: i64,
}

/// Per-cell streaming buffer — capped to avoid unbounded growth.
pub struct StreamingBuffer {
    pub chunks: VecDeque<StreamChunk>,
    pub max_chunks: usize,
    next_seq: u64,
}

impl StreamingBuffer {
    pub fn new(max_chunks: usize) -> Self {
        Self {
            chunks: VecDeque::with_capacity(max_chunks.min(1024)),
            max_chunks,
            next_seq: 0,
        }
    }

    pub fn push(&mut self, stream: super::types::OutputStream, data: String) -> u64 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        let chunk = StreamChunk {
            seq,
            stream,
            data,
            ts_unix: super::helpers::now_unix(),
        };
        if self.chunks.len() >= self.max_chunks {
            self.chunks.pop_front();
        }
        self.chunks.push_back(chunk);
        seq
    }

    pub fn since(&self, since_seq: u64) -> Vec<StreamChunk> {
        self.chunks
            .iter()
            .filter(|c| c.seq >= since_seq)
            .cloned()
            .collect()
    }

    pub fn drain(&mut self) -> Vec<StreamChunk> {
        let v: Vec<StreamChunk> = self.chunks.iter().cloned().collect();
        self.chunks.clear();
        v
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

impl Default for StreamingBuffer {
    fn default() -> Self {
        Self::new(2048)
    }
}

/// Reader handle — exposed to UI code that polls chunks.
pub struct StreamReader {
    pub cell_id: String,
    pub last_seen_seq: u64,
}

impl StreamReader {
    pub fn new(cell_id: String) -> Self {
        Self {
            cell_id,
            last_seen_seq: 0,
        }
    }
}

fn buffers() -> &'static Mutex<HashMap<String, StreamingBuffer>> {
    static BUF: OnceLock<Mutex<HashMap<String, StreamingBuffer>>> = OnceLock::new();
    BUF.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Push a chunk into the buffer for `cell_id`, creating the buffer
/// on first use.
pub fn push_chunk(
    cell_id: &str,
    stream: super::types::OutputStream,
    data: String,
) -> Option<u64> {
    buffers().lock().ok().map(|mut g| {
        let entry = g
            .entry(cell_id.to_string())
            .or_default();
        entry.push(stream, data)
    })
}

/// Read all chunks since `since_seq`.  Returns empty vec if cell has
/// no buffer.
pub fn read_chunks(cell_id: &str, since_seq: u64) -> Vec<StreamChunk> {
    buffers()
        .lock()
        .ok()
        .and_then(|g| g.get(cell_id).map(|b| b.since(since_seq)))
        .unwrap_or_default()
}

/// Drain everything for a cell (used at cell end).
pub fn drain_cell(cell_id: &str) -> Vec<StreamChunk> {
    buffers()
        .lock()
        .ok()
        .and_then(|mut g| g.get_mut(cell_id).map(|b| b.drain()))
        .unwrap_or_default()
}

/// Remove a cell's buffer entirely.
pub fn drop_cell(cell_id: &str) {
    if let Ok(mut g) = buffers().lock() {
        g.remove(cell_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_sandbox_lite::types::OutputStream;

    #[test]
    fn buffer_seq_increments_monotonically() {
        let mut b = StreamingBuffer::new(10);
        let s1 = b.push(OutputStream::Stdout, "a".into());
        let s2 = b.push(OutputStream::Stdout, "b".into());
        let s3 = b.push(OutputStream::Stderr, "c".into());
        assert_eq!(s1, 0);
        assert_eq!(s2, 1);
        assert_eq!(s3, 2);
    }

    #[test]
    fn buffer_caps_at_max_chunks() {
        let mut b = StreamingBuffer::new(3);
        for i in 0..10 {
            b.push(OutputStream::Stdout, format!("chunk-{i}"));
        }
        assert_eq!(b.len(), 3);
        // First chunks dropped — we should see 7, 8, 9.
        let chunks = b.since(0);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].data.contains('7'));
        assert!(chunks[2].data.contains('9'));
    }

    #[test]
    fn since_returns_only_chunks_after_seq() {
        let mut b = StreamingBuffer::new(10);
        for i in 0..5 {
            b.push(OutputStream::Stdout, format!("c{i}"));
        }
        let chunks = b.since(2);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].data.contains('2'));
    }

    #[test]
    fn drain_returns_all_and_clears() {
        let mut b = StreamingBuffer::new(10);
        for i in 0..3 {
            b.push(OutputStream::Stdout, format!("c{i}"));
        }
        let drained = b.drain();
        assert_eq!(drained.len(), 3);
        assert!(b.is_empty());
    }

    #[test]
    fn global_push_and_read() {
        let cell = "test-stream-global-1";
        push_chunk(cell, OutputStream::Stdout, "hello".into());
        push_chunk(cell, OutputStream::Stderr, "warn".into());
        let chunks = read_chunks(cell, 0);
        assert_eq!(chunks.len(), 2);
        drop_cell(cell);
        assert!(read_chunks(cell, 0).is_empty());
    }

    #[test]
    fn read_chunks_for_unknown_cell_is_empty() {
        assert!(read_chunks("never-existed", 0).is_empty());
    }

    #[test]
    fn drain_cell_consumes_buffer() {
        let cell = "test-stream-drain-1";
        push_chunk(cell, OutputStream::Stdout, "x".into());
        let drained = drain_cell(cell);
        assert_eq!(drained.len(), 1);
        let after = read_chunks(cell, 0);
        assert!(after.is_empty());
        drop_cell(cell);
    }

    #[test]
    fn stream_reader_tracks_last_seen() {
        let r = StreamReader::new("cell-x".into());
        assert_eq!(r.last_seen_seq, 0);
        assert_eq!(r.cell_id, "cell-x");
    }
}
