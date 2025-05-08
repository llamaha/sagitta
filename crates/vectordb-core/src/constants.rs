// Constants shared within vectordb_core, potentially moved from vectordb_cli

// Fields used in Qdrant payloads and filters
/// The field name used for storing the relative file path in Qdrant payloads.
pub const FIELD_FILE_PATH: &str = "file_path";
/// The field name used for storing the starting line number of a code chunk.
pub const FIELD_START_LINE: &str = "start_line";
/// The field name used for storing the ending line number of a code chunk.
pub const FIELD_END_LINE: &str = "end_line";
/// The field name used for storing the programming language of a code chunk.
pub const FIELD_LANGUAGE: &str = "language";
/// The field name used for storing the raw content of a code chunk.
pub const FIELD_CHUNK_CONTENT: &str = "chunk_content";
/// The field name used for storing the type of code element (e.g., function, struct).
pub const FIELD_ELEMENT_TYPE: &str = "element_type";
/// The field name used for storing the file extension.
pub const FIELD_FILE_EXTENSION: &str = "file_extension";
/// The field name used for storing the Git branch name.
pub const FIELD_BRANCH: &str = "branch";
/// The field name used for storing the Git commit hash.
pub const FIELD_COMMIT_HASH: &str = "commit_hash";

// Performance-related constants are now configurable via config.toml
// See crate::config::PerformanceConfig for the default values and configuration options

// Other constants
/// Default batch size for Qdrant upsert operations.
pub const BATCH_SIZE: usize = 256; // Batch size for Qdrant upserts
/// Batch size specifically for internal embedding generation.
pub const INTERNAL_EMBED_BATCH_SIZE: usize = 512; // Increased batch size again (was 256)
/// Default prefix for Qdrant collection names associated with repositories.
pub const COLLECTION_NAME_PREFIX: &str = "repo_"; // Example value, adjust if needed
/// Default maximum file size (in bytes) to process during indexing.
pub const MAX_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024; // 5 MB default 
/// Default dimension assumed for vector embeddings if not otherwise specified.
pub const DEFAULT_VECTOR_DIMENSION: u64 = 384; 