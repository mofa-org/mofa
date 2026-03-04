//! Similarity computation functions for embedding vectors

use mofa_kernel::rag::SimilarityMetric;

/// Compute similarity between two embedding vectors using the given metric.
///
/// For Cosine and DotProduct, higher values mean more similar.
/// For Euclidean, the raw distance is converted so that higher values
/// still mean more similar (using 1 / (1 + distance)).
pub fn compute_similarity(a: &[f32], b: &[f32], metric: SimilarityMetric) -> f32 {
    match metric {
        SimilarityMetric::Cosine => cosine_similarity(a, b),
        SimilarityMetric::Euclidean => {
            let dist = euclidean_distance(a, b);
            1.0 / (1.0 + dist)
        }
        SimilarityMetric::DotProduct => dot_product(a, b),
        _ => 0.0,
    }
}

/// Cosine similarity between two vectors.
///
/// Returns a value between -1.0 and 1.0 (1.0 for identical direction,
/// 0.0 for orthogonal, -1.0 for opposite direction).
/// Returns 0.0 if either vector has zero magnitude.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Euclidean (L2) distance between two vectors.
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Dot product between two vectors.
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_euclidean_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let dist = euclidean_distance(&a, &b);
        assert!(dist.abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_known_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product_known() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dp = dot_product(&a, &b);
        assert!((dp - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_similarity_cosine() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        let sim = compute_similarity(&a, &b, SimilarityMetric::Cosine);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_similarity_euclidean() {
        let a = vec![0.0, 0.0];
        let b = vec![0.0, 0.0];
        let sim = compute_similarity(&a, &b, SimilarityMetric::Euclidean);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_similarity_dot_product() {
        let a = vec![2.0, 3.0];
        let b = vec![4.0, 5.0];
        let sim = compute_similarity(&a, &b, SimilarityMetric::DotProduct);
        assert!((sim - 23.0).abs() < 1e-6);
    }
}
