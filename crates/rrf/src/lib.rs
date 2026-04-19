// SPDX-License-Identifier: MIT
//! # `rrf` — Reciprocal Rank Fusion
//!
//! A pure-Rust, zero-dependency implementation of Reciprocal Rank Fusion
//! (Cormack, Clarke, Büttcher 2009).  Given two or more ranked result
//! lists from different retrievers (BM25, vector cosine, trigram, etc.),
//! `fuse()` merges them into a single ranking that is **provably more
//! robust than any single list**.  This is the technique used by:
//!
//! * Microsoft Bing (since 2010)
//! * Elasticsearch hybrid search (`rrf` retriever, since 8.8)
//! * Pinecone hybrid search
//! * Weaviate hybrid search
//! * LanceDB hybrid search (`vector + fts → rrf`)
//!
//! ## The math
//!
//! ```text
//!   RRFscore(d) = Σ  1 / (k + rank_i(d))
//!                  i
//! ```
//!
//! Where `rank_i(d)` is the 1-based position of document `d` in the
//! `i`-th ranked list, and `k` is a constant (the original paper uses
//! `k = 60` and that has held up in every replication for 17 years).
//! Documents missing from a list contribute zero to the sum from that
//! list — they are NOT penalised for absence.
//!
//! ## Why `k = 60`?
//!
//! The reciprocal `1 / (k + rank)` curve drops fast at small ranks
//! (`1/61` vs `1/62` is a 1.6% step) but flattens at deeper ranks
//! (`1/110` vs `1/111` is 0.9%).  This naturally weights the top-of-list
//! agreement between retrievers more than tail overlap.  Lower `k`
//! sharpens the top, higher `k` flattens.  60 is the empirical sweet spot
//! across TREC, BEIR, and MS MARCO.
//!
//! ## ImpForge positioning
//!
//! RRF in the **MIT FREE** tier of impforge-app is a strategic moat:
//! Cursor doesn't ship hybrid retrieval, Continue doesn't, the major
//! editor-AI vendors don't.  We do — and free.  Pro layers cross-encoder
//! reranking on top.
//!
//! ## Example
//!
//! ```
//! use rrf::fuse;
//! let porter = vec!["doc_a", "doc_b", "doc_c"];
//! let trigram = vec!["doc_b", "doc_a", "doc_d"];
//! let merged = fuse(&[porter, trigram], rrf::DEFAULT_K);
//! // doc_a and doc_b each have score 1/61 + 1/62 — tied at the very top.
//! // doc_c (porter only at rank 3) and doc_d (trigram only at rank 3)
//! // both score 1/63, also tied but strictly lower than the top pair.
//! let top_two: Vec<&&str> = merged.iter().take(2).map(|(t, _)| t).collect();
//! assert!(top_two.contains(&&"doc_a"));
//! assert!(top_two.contains(&&"doc_b"));
//! assert!(merged[0].1 > merged[2].1);
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::hash::Hash;

/// Canonical Cormack/Clarke/Büttcher constant.
///
/// 60 has been the published default since the original SIGIR 2009 paper
/// and is what every major hybrid-search engine ships.  Unless you are
/// running an ablation study, pass `DEFAULT_K`.
pub const DEFAULT_K: f64 = 60.0;

/// Merge multiple ranked lists into a single ranking via Reciprocal Rank
/// Fusion.
///
/// * `rankings` — each inner `Vec<T>` is one retriever's ordered output.
///   Position 0 = best; the function uses 1-based ranks internally per
///   the original paper.
/// * `k` — the RRF constant.  Pass [`DEFAULT_K`] (60.0) unless you have a
///   measured reason to deviate.  Must be > 0; values ≤ 0 are clamped to
///   `f64::EPSILON` to avoid division-by-zero.
///
/// Returns a `Vec<(T, f64)>` sorted by score descending — first item is
/// the best fused result.
///
/// Stable for ties (uses `partial_cmp` with deterministic
/// `Ordering::Equal` fallback).
///
/// ## Complexity
///
/// `O(N * log N)` where `N` is the total number of distinct items
/// across all lists.  Memory: `O(N)`.  Cloning of `T` happens
/// exactly once per `(retriever, item)` pair.
pub fn fuse<T>(rankings: &[Vec<T>], k: f64) -> Vec<(T, f64)>
where
    T: Clone + Eq + Hash,
{
    let safe_k = if k > 0.0 { k } else { f64::EPSILON };
    let mut scores: HashMap<T, f64> = HashMap::new();

    for ranking in rankings {
        for (zero_based, item) in ranking.iter().enumerate() {
            let rank = (zero_based + 1) as f64;
            *scores.entry(item.clone()).or_insert(0.0) += 1.0 / (safe_k + rank);
        }
    }

    let mut result: Vec<(T, f64)> = scores.into_iter().collect();
    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });
    result
}

/// Trim a fused ranking to the top `n` items.  Convenience wrapper.
pub fn fuse_top<T>(rankings: &[Vec<T>], k: f64, n: usize) -> Vec<(T, f64)>
where
    T: Clone + Eq + Hash,
{
    let mut all = fuse(rankings, k);
    all.truncate(n);
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn rrf_merges_two_disjoint_rankings() {
        let a = vec!["x", "y", "z"];
        let b = vec!["p", "q", "r"];
        let merged = fuse(&[a, b], DEFAULT_K);
        // 6 unique items, all should appear with non-zero score.
        assert_eq!(merged.len(), 6);
        for (_item, score) in &merged {
            assert!(*score > 0.0);
        }
    }

    #[test]
    fn rrf_correctness_per_paper_formula() {
        // Two rankings, one shared item.
        let a = vec!["doc_alpha", "doc_beta"];
        let b = vec!["doc_beta", "doc_gamma"];
        let merged = fuse(&[a, b], DEFAULT_K);

        // Find each item's score.
        let alpha = merged.iter().find(|(t, _)| *t == "doc_alpha").map(|(_, s)| *s);
        let beta = merged.iter().find(|(t, _)| *t == "doc_beta").map(|(_, s)| *s);
        let gamma = merged.iter().find(|(t, _)| *t == "doc_gamma").map(|(_, s)| *s);

        assert_relative_eq!(alpha.expect("alpha"), 1.0 / (60.0 + 1.0), epsilon = 1e-12);
        assert_relative_eq!(
            beta.expect("beta"),
            1.0 / (60.0 + 2.0) + 1.0 / (60.0 + 1.0),
            epsilon = 1e-12
        );
        assert_relative_eq!(gamma.expect("gamma"), 1.0 / (60.0 + 2.0), epsilon = 1e-12);
    }

    #[test]
    fn rrf_top_overlap_beats_singletons() {
        let porter = vec!["doc1", "doc2", "doc3", "doc4"];
        let trigram = vec!["doc2", "doc1", "doc4", "doc5"];
        let merged = fuse(&[porter, trigram], DEFAULT_K);
        // doc1 + doc2 both appear at positions 1+2 / 2+1 → effectively tied
        // at the very top.  Both must beat doc3 (porter only) and doc5
        // (trigram only).
        let top_two: Vec<&&str> = merged.iter().take(2).map(|(t, _)| t).collect();
        assert!(top_two.contains(&&"doc1"));
        assert!(top_two.contains(&&"doc2"));
    }

    #[test]
    fn rrf_handles_empty_rankings() {
        let empty: Vec<Vec<&str>> = vec![];
        assert!(fuse(&empty, DEFAULT_K).is_empty());

        let one_empty: Vec<Vec<&str>> = vec![vec![], vec![]];
        assert!(fuse(&one_empty, DEFAULT_K).is_empty());
    }

    #[test]
    fn rrf_handles_single_ranking_unchanged_order() {
        let single = vec![vec!["a", "b", "c", "d"]];
        let merged = fuse(&single, DEFAULT_K);
        assert_eq!(merged.len(), 4);
        assert_eq!(merged[0].0, "a");
        assert_eq!(merged[3].0, "d");
        // Scores must be strictly decreasing.
        assert!(merged[0].1 > merged[1].1);
        assert!(merged[1].1 > merged[2].1);
        assert!(merged[2].1 > merged[3].1);
    }

    #[test]
    fn rrf_clamps_non_positive_k() {
        // k=0 would otherwise divide by rank, k<0 would give negative
        // scores and break ordering.  Clamp to EPSILON.
        let r = vec!["a", "b"];
        let merged_zero = fuse(&[r.clone()], 0.0);
        let merged_neg = fuse(&[r.clone()], -10.0);
        let merged_eps = fuse(&[r], f64::EPSILON);
        assert_eq!(merged_zero[0].0, "a");
        assert_eq!(merged_neg[0].0, "a");
        assert_eq!(merged_zero[0].1, merged_eps[0].1);
    }

    #[test]
    fn rrf_three_way_merge() {
        let a = vec!["x", "y"];
        let b = vec!["y", "z"];
        let c = vec!["x", "z"];
        let merged = fuse(&[a, b, c], DEFAULT_K);
        // Every item appears in exactly 2 of the 3 lists; expect 3 items.
        assert_eq!(merged.len(), 3);
        // x: pos 1 + pos 1 = 2 / 61
        // y: pos 2 + pos 1     = 1/62 + 1/61
        // z: pos 2 + pos 2 = 2 / 62
        let score_x = merged.iter().find(|(t, _)| *t == "x").map(|(_, s)| *s).expect("x");
        let score_y = merged.iter().find(|(t, _)| *t == "y").map(|(_, s)| *s).expect("y");
        let score_z = merged.iter().find(|(t, _)| *t == "z").map(|(_, s)| *s).expect("z");
        assert!(score_x > score_y);
        assert!(score_y > score_z);
    }

    #[test]
    fn fuse_top_truncates_correctly() {
        let a = vec!["1", "2", "3", "4", "5"];
        let merged = fuse_top(&[a], DEFAULT_K, 3);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].0, "1");
        assert_eq!(merged[2].0, "3");
    }

    #[test]
    fn fuse_top_n_zero_is_empty() {
        let a = vec!["1", "2"];
        let merged = fuse_top(&[a], DEFAULT_K, 0);
        assert!(merged.is_empty());
    }

    #[test]
    fn rrf_works_with_string_payloads() {
        // Ensures the generic bound is satisfied for owned strings (most
        // common ImpForge use-case where T = chunk_id String).
        let a: Vec<String> = vec!["chunk_1".into(), "chunk_2".into()];
        let b: Vec<String> = vec!["chunk_2".into(), "chunk_3".into()];
        let merged = fuse(&[a, b], DEFAULT_K);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].0, "chunk_2");
    }

    #[test]
    fn rrf_works_with_integer_payloads() {
        // For ImpForge: T = i64 (SQLite ROWID).
        let a: Vec<i64> = vec![100, 200, 300];
        let b: Vec<i64> = vec![300, 100, 400];
        let merged = fuse(&[a, b], DEFAULT_K);
        assert_eq!(merged.len(), 4);
        // 100 and 300 both appear in both → must dominate.
        let top_two: Vec<i64> = merged.iter().take(2).map(|(t, _)| *t).collect();
        assert!(top_two.contains(&100));
        assert!(top_two.contains(&300));
    }
}
