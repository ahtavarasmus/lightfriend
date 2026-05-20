//! Embedding similarity helpers for the commitment detection signal memory.
//!
//! Embeddings are persisted as raw little-endian f32 bytes in
//! `commitment_label_embeddings.embedding`. These helpers convert between the
//! BYTEA representation and an `f32` slice, and provide brute-force cosine
//! similarity against a candidate set. Brute force is fine at the per-user
//! scale (hundreds of vectors at most). If/when it isn't, swap the backing
//! store for pgvector and use this module only for tests.

/// Pack an embedding (slice of f32) into a little-endian byte buffer suitable
/// for storage in a BYTEA column.
pub fn pack_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(embedding.len() * 4);
    for v in embedding {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Unpack a BYTEA-stored embedding back into f32 values. Returns None when the
/// byte length is not a multiple of 4.
pub fn unpack_embedding(bytes: &[u8]) -> Option<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        out.push(f32::from_le_bytes(arr));
    }
    Some(out)
}

/// Cosine similarity between two equal-length f32 vectors. Returns 0.0 when
/// either vector is the zero vector (no meaningful similarity).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Return the highest cosine similarity between `query` and any vector in
/// `candidates`. Returns 0.0 when `candidates` is empty. Skips candidates of
/// the wrong dimension rather than erroring - in practice this shouldn't
/// happen, but a model swap could leave older rows of a different size.
pub fn max_similarity(query: &[f32], candidates: &[Vec<f32>]) -> f32 {
    candidates
        .iter()
        .filter(|c| c.len() == query.len())
        .map(|c| cosine_similarity(query, c))
        .fold(0.0f32, |best, s| if s > best { s } else { best })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_roundtrip() {
        let original = vec![0.1f32, -0.2, 0.3, 0.4];
        let packed = pack_embedding(&original);
        let unpacked = unpack_embedding(&packed).unwrap();
        for (a, b) in original.iter().zip(unpacked.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_zero_vector_returns_zero() {
        let z = vec![0.0, 0.0, 0.0];
        let v = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&z, &v), 0.0);
    }

    #[test]
    fn cosine_mismatched_length_returns_zero() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn unpack_invalid_length_returns_none() {
        assert!(unpack_embedding(&[1, 2, 3]).is_none());
    }

    #[test]
    fn max_similarity_picks_best_match() {
        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            vec![0.0, 1.0, 0.0], // orthogonal
            vec![0.5, 0.5, 0.0], // 45-degree
            vec![0.9, 0.1, 0.0], // close to query
        ];
        let best = max_similarity(&query, &candidates);
        // Closest is candidate[2], cosine ~0.994
        assert!(best > 0.99);
    }
}
