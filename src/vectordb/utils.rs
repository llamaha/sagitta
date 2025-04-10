/// Calculate cosine distance between two vectors (range 0.0 to 2.0)
/// Higher values mean less similarity.
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    // Handle zero vectors to avoid division by zero and return max distance
    if norm_a == 0.0 || norm_b == 0.0 {
        return 2.0; // Max distance for cosine similarity interpretation (1.0 - (-1.0))
    }

    // Calculate cosine similarity
    let similarity = dot_product / (norm_a * norm_b);

    // Clamp similarity to [-1.0, 1.0] to handle potential floating point inaccuracies
    let clamped_similarity = similarity.clamp(-1.0, 1.0);

    // Convert similarity to distance: distance = 1.0 - similarity
    1.0 - clamped_similarity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_distance_basic() {
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![1.0, 0.0, 0.0];
        let vec3 = vec![0.0, 1.0, 0.0];
        let vec4 = vec![-1.0, 0.0, 0.0];
        let vec5 = vec![0.0, 0.0, 0.0];

        // Use approximate comparison for floating point results
        assert!((cosine_distance(&vec1, &vec2) - 0.0).abs() < 1e-6);
        assert!((cosine_distance(&vec1, &vec3) - 1.0).abs() < 1e-6); // Orthogonal
        assert!((cosine_distance(&vec1, &vec4) - 2.0).abs() < 1e-6); // Opposite
        assert!((cosine_distance(&vec1, &vec5) - 2.0).abs() < 1e-6); // Zero vector
        assert!((cosine_distance(&vec5, &vec5) - 2.0).abs() < 1e-6); // Zero vector
    }

    #[test]
    fn test_cosine_distance_non_unit() {
        let vec1 = vec![2.0, 0.0];
        let vec2 = vec![4.0, 0.0];
        let vec3 = vec![0.0, 3.0];
        assert!(cosine_distance(&vec1, &vec2) < 1e-6);
        assert!((cosine_distance(&vec1, &vec3) - 1.0).abs() < 1e-6);
    }

    // Remove the near_normalized test as it's covered by non_unit
    // #[test]
    // fn test_cosine_distance_near_normalized() { ... }

} 