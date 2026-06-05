use sha2::{Digest, Sha256};

pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn compute_hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        let hash1 = compute_hash("hello world");
        let hash2 = compute_hash("hello world");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_hash_different_content() {
        let hash1 = compute_hash("hello");
        let hash2 = compute_hash("world");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_hash_bytes() {
        let data = b"test data";
        let hash = compute_hash_bytes(data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }
}
