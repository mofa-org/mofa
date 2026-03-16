//! Reciprocal Rank Fusion (RRF) implementation
//!
//! RRF is a rank aggregation method that combines multiple ranked lists
//! into a single unified ranking. It's particularly effective for combining
//! dense (semantic) and sparse (keyword) retrieval results.
//!
//! Formula: RRF_score(d) = Σ 1 / (rank(d) + k)
//! where k is a constant (typically 60) that prevents division by zero
//! and reduces the impact of very low rankings.

use mofa_kernel::rag::ScoredDocument;
use std::collections::{HashMap, HashSet};

/// Default RRF k parameter commonly used in production systems.
pub const DEFAULT_RRF_K: f64 = 60.0;

/// Compute Reciprocal Rank Fusion scores from multiple ranked result lists.
///
/// This function takes multiple vectors of scored documents (e.g., from
/// dense and sparse retrievers) and combines them using the RRF formula.
///
/// # Arguments
/// * `result_lists` - A slice of ranked document vectors to fuse
/// * `k` - The RRF k parameter (higher values give more weight to lower ranks)
/// * `top_k` - Maximum number of results to return
///
/// # Returns
/// A vector of fused and re-ranked documents sorted by RRF score (descending).
///
/// # Example
/// ```ignore
/// use mofa_foundation::rag::hybrid::reciprocal_rank_fusion;
///
/// let dense_results = vec![doc1, doc2, doc3];
/// let sparse_results = vec![doc2, doc1, doc4];
/// let fused = reciprocal_rank_fusion(&[dense_results, sparse_results], 60.0, 3);
/// ```
pub fn reciprocal_rank_fusion(
    result_lists: &[Vec<ScoredDocument>],
    k: f64,
    top_k: usize,
) -> Vec<ScoredDocument> {
    if result_lists.is_empty() {
        return Vec::new();
    }

    // If only one list, just return top_k from it
    if result_lists.len() == 1 {
        let mut results = result_lists[0].clone();
        results.truncate(top_k);
        return results;
    }

    // Map: document_id -> (rrf_score, document)
    let mut rrf_scores: HashMap<String, (f64, ScoredDocument)> = HashMap::new();
    let mut all_ids: HashSet<String> = HashSet::new();

    // Process each result list
    for (rank, results) in result_lists.iter().enumerate() {
        // Assign unique source label if not already present
        let source_suffix = if result_lists.len() > 1 {
            Some(format!(
                "{}{}",
                results.first().and_then(|d| d.source.as_deref()).unwrap_or(""),
                if rank == 0 { "/dense" } else { "/sparse" }
            ))
        } else {
            None
        };

        for (position, doc) in results.iter().enumerate() {
            let position = position as f64;
            // RRF formula: 1 / (rank + k)
            let rrf_score = 1.0 / (position + k);

            let entry = rrf_scores
                .entry(doc.document.id.clone())
                .or_insert((0.0, doc.clone()));

            entry.0 += rrf_score;

            // Update source label if this is from a hybrid result
            if let Some(ref suffix) = source_suffix && entry.1.source.is_none() {
                entry.1.source = Some(suffix.clone());
            }
            all_ids.insert(doc.document.id.clone());
        }
    }

    // Convert to vector and sort by RRF score (descending)
    let mut fused: Vec<ScoredDocument> = rrf_scores
        .into_values()
        .map(|(score, mut doc)| {
            doc.score = score as f32;
            doc
        })
        .collect();

    fused.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Return top_k results
    fused.truncate(top_k);
    fused
}

/// Compute RRF with default k parameter (60.0).
pub fn reciprocal_rank_fusion_default(
    result_lists: &[Vec<ScoredDocument>],
    top_k: usize,
) -> Vec<ScoredDocument> {
    reciprocal_rank_fusion(result_lists, DEFAULT_RRF_K, top_k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::rag::Document;

    fn doc(id: &str, text: &str, score: f32, source: &str) -> ScoredDocument {
        ScoredDocument::new(
            Document::new(id, text),
            score,
            Some(source.to_string()),
        )
    }

    #[test]
    fn test_empty_input() {
        let results: Vec<Vec<ScoredDocument>> = vec![];
        let fused = reciprocal_rank_fusion(&results, 60.0, 5);
        assert!(fused.is_empty());
    }

    #[test]
    fn test_single_list() {
        let dense = vec![
            doc("d1", "doc1", 0.9, "dense"),
            doc("d2", "doc2", 0.8, "dense"),
        ];
        let fused = reciprocal_rank_fusion(&[dense], 60.0, 5);
        assert_eq!(fused.len(), 2);
    }

    #[test]
    fn test_two_lists_different_order() {
        // Dense returns: d1 > d2 > d3
        let dense = vec![
            doc("d1", "doc1", 0.9, "dense"),
            doc("d2", "doc2", 0.8, "dense"),
            doc("d3", "doc3", 0.7, "dense"),
        ];
        // Sparse returns: d2 > d1 > d4
        let sparse = vec![
            doc("d2", "doc2", 0.95, "sparse"),
            doc("d1", "doc1", 0.85, "sparse"),
            doc("d4", "doc4", 0.75, "sparse"),
        ];

        let fused = reciprocal_rank_fusion(&[dense, sparse], 60.0, 3);

        // d1 and d2 appear in both lists with same RRF scores, so they should rank higher
        // d3 and d4 appear in only one list each
        assert_eq!(fused.len(), 3);
        // Both d1 and d2 should be in top results
        let ids: Vec<_> = fused.iter().map(|d| d.document.id.as_str()).collect();
        assert!(ids.contains(&"d1"));
        assert!(ids.contains(&"d2"));
    }

    #[test]
    fn test_top_k_filtering() {
        let dense = vec![
            doc("d1", "doc1", 0.9, "dense"),
            doc("d2", "doc2", 0.8, "dense"),
            doc("d3", "doc3", 0.7, "dense"),
            doc("d4", "doc4", 0.6, "dense"),
        ];
        let sparse = vec![
            doc("s1", "sparse1", 0.95, "sparse"),
            doc("s2", "sparse2", 0.85, "sparse"),
        ];

        let fused = reciprocal_rank_fusion(&[dense, sparse], 60.0, 2);
        assert_eq!(fused.len(), 2);
    }

    #[test]
    fn test_rrf_score_calculation() {
        // With k=60, the RRF score for rank 0 is 1/60 ≈ 0.0167
        let dense = vec![doc("d1", "doc1", 0.9, "dense")];
        let sparse = vec![doc("d1", "doc1", 0.9, "sparse")];

        let fused = reciprocal_rank_fusion(&[dense, sparse], 60.0, 1);

        // Both lists have doc1 at rank 0, so score should be 2 * (1/60)
        let expected = 2.0 * (1.0 / 60.0) as f32;
        assert!((fused[0].score - expected).abs() < 0.001);
    }

    #[test]
    fn test_default_k_parameter() {
        let dense = vec![doc("d1", "doc1", 0.9, "dense")];
        let sparse = vec![doc("d1", "doc1", 0.9, "sparse")];

        let fused_default = reciprocal_rank_fusion_default(&[dense.clone(), sparse.clone()], 1);
        let fused_custom = reciprocal_rank_fusion(&[dense, sparse], 60.0, 1);

        assert_eq!(fused_default[0].score, fused_custom[0].score);
    }
}
