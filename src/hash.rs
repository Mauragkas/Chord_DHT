use crate::M;
use sha2::{Digest, Sha256};

pub fn hash(input: &str) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let hash = hasher.finalize();
    let hash_value = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    (hash_value % 2_u64.pow(*M as u32) as u64) as u32
}
