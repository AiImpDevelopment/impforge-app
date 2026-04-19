// SPDX-License-Identifier: MIT
//! Sandbox limiter — combines fuel + epoch + memory + table caps into
//! a single `wasmtime::ResourceLimiter` impl.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::types::Limits;

/// `ResourceLimiter` impl shared by every sandbox `Store`.  Tracks
/// peak memory usage so the UI can display "you used X / Y MiB".
pub struct SandboxLimiter {
    pub max_memory_bytes: usize,
    pub peak_memory: Arc<AtomicU64>,
    pub growth_failures: u32,
    pub max_table_size: usize,
}

impl SandboxLimiter {
    pub fn new(max_memory_bytes: usize) -> Self {
        Self {
            max_memory_bytes,
            peak_memory: Arc::new(AtomicU64::new(0)),
            growth_failures: 0,
            max_table_size: 100_000,
        }
    }

    pub fn from_limits(limits: &Limits) -> Self {
        Self::new((limits.memory_mib as usize).saturating_mul(1024 * 1024))
    }

    pub fn peak_memory_bytes(&self) -> u64 {
        self.peak_memory.load(Ordering::Relaxed)
    }
}

impl wasmtime::ResourceLimiter for SandboxLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.max_memory_bytes {
            self.growth_failures = self.growth_failures.saturating_add(1);
            return Ok(false);
        }
        // Track peak.
        self.peak_memory
            .fetch_max(desired as u64, Ordering::Relaxed);
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired < self.max_table_size)
    }

    fn instances(&self) -> usize {
        // 1 cell per Store — keep tight.
        1
    }
    fn tables(&self) -> usize {
        16
    }
    fn memories(&self) -> usize {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime::ResourceLimiter;

    #[test]
    fn limiter_rejects_oversized_growth() {
        let mut l = SandboxLimiter::new(1024); // 1 KiB cap
        assert!(matches!(
            l.memory_growing(0, 2048, None),
            Ok(false)
        ));
        assert_eq!(l.growth_failures, 1);
    }

    #[test]
    fn limiter_tracks_peak_memory() {
        let mut l = SandboxLimiter::new(1024 * 1024);
        let _ = l.memory_growing(0, 4096, None);
        let _ = l.memory_growing(4096, 8192, None);
        let _ = l.memory_growing(8192, 4096, None); // shrink
        assert_eq!(l.peak_memory_bytes(), 8192);
    }

    #[test]
    fn limiter_allows_growth_under_cap() {
        let mut l = SandboxLimiter::new(1024 * 1024);
        for n in 1..=10 {
            assert!(l.memory_growing(0, n * 1024, None).expect("grow ok"));
        }
    }

    #[test]
    fn limiter_table_growth_capped() {
        let mut l = SandboxLimiter::new(1024 * 1024);
        assert!(l.table_growing(0, 1000, None).expect("ok"));
        assert!(!l.table_growing(0, 1_000_000, None).expect("ok"));
    }

    #[test]
    fn limiter_from_limits_uses_mib() {
        let lim = Limits {
            memory_mib: 256,
            ..Limits::default()
        };
        let l = SandboxLimiter::from_limits(&lim);
        assert_eq!(l.max_memory_bytes, 256 * 1024 * 1024);
    }

    #[test]
    fn limiter_advisory_counts_sane() {
        let l = SandboxLimiter::new(1024);
        assert_eq!(l.instances(), 1);
        assert_eq!(l.tables(), 16);
        assert_eq!(l.memories(), 4);
    }
}
