// src/vectordb/indexing.rs

// Comment out imports causing errors for now
 // Ensure EmbeddingModel is still needed
// use crate::vectordb::provider::Provider; // Commented out
// Removed unused: use rayon::prelude::*;
use walkdir::DirEntry;

// Define a placeholder struct for IndexedChunk if needed, or comment out usage
// struct IndexedChunk { /* ... fields ... */ }

/* // Comment out the entire index_directory function for now
pub(super) fn index_directory(db: &mut VectorDB, dir_path: &str, file_patterns: &[String]) -> Result<()> {
    // ... function body ...
}
*/

/* // Comment out the entire remove_directory function for now
pub(super) fn remove_directory(db: &mut VectorDB, dir_path: &str) -> Result<()> {
    // ... function body ...
}
*/

// Comment out functions that depend on VectorDB or IndexedChunk

/* // Comment out collect_files
fn collect_files(_db: &VectorDB, canonical_dir_path: &str, file_patterns: &[String]) -> Result<Vec<PathBuf>> {
    // ... function body ...
}
*/

/* // Comment out index_files_parallel
fn index_files_parallel(
    db: &mut VectorDB,
    files_to_index: Vec<PathBuf>,
    embedding_model: Arc<EmbeddingModel>,
    cache: Arc<Mutex<EmbeddingCache>>,
) -> Result<Vec<IndexedChunk>> {
    // ... function body ...
}
*/

/* // Comment out index_single_file
fn index_single_file(
    file_path: &Path,
    embedding_model: &EmbeddingModel,
    cache: &Mutex<EmbeddingCache>,
    // db_state: &HashMap<String, IndexedChunk>, // Removed dependency
    pb: &ProgressBar,
) -> Result<Option<Vec<IndexedChunk>>> {
    // ... function body ...
}
*/

/* // Comment out rebuild_hnsw_index_from_state
fn rebuild_hnsw_index_from_state(db: &mut VectorDB, dimension: usize) -> Result<()> {
    // ... function body ...
}
*/

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with('.'))
         .unwrap_or(false)
}

fn is_target_dir(entry: &DirEntry) -> bool {
    entry.file_name() == "target" && entry.file_type().is_dir()
} 