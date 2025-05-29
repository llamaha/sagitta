pub mod detector;
pub mod merkle;
pub mod resync;
pub mod diff;

// Re-export commonly used types
pub use merkle::{MerkleManager, HashDiff, calculate_file_hash, calculate_merkle_root, compare_file_hashes};
pub use detector::*;
pub use diff::*; 