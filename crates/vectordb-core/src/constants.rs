// Constants shared within vectordb_core, potentially moved from vectordb_cli

// Fields used in Qdrant payloads and filters
pub const FIELD_FILE_PATH: &str = "file_path";
pub const FIELD_START_LINE: &str = "start_line";
pub const FIELD_END_LINE: &str = "end_line";
pub const FIELD_LANGUAGE: &str = "language";
pub const FIELD_CHUNK_CONTENT: &str = "chunk_content";
pub const FIELD_ELEMENT_TYPE: &str = "element_type";
pub const FIELD_FILE_EXTENSION: &str = "file_extension";
pub const FIELD_BRANCH: &str = "branch";
pub const FIELD_COMMIT_HASH: &str = "commit_hash";

// Performance-related constants are now configurable via config.toml
// See crate::config::PerformanceConfig for the default values and configuration options

// Other constants
pub const BATCH_SIZE: usize = 256; // Batch size for Qdrant upserts
pub const INTERNAL_EMBED_BATCH_SIZE: usize = 512; // Increased batch size again (was 256)
pub const COLLECTION_NAME_PREFIX: &str = "repo_"; // Example value, adjust if needed
pub const MAX_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024; // 5 MB default 
pub const DEFAULT_VECTOR_DIMENSION: u64 = 384; 