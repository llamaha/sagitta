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

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with('.'))
         .unwrap_or(false)
}

pub fn is_target_dir(entry: &DirEntry) -> bool {
    entry.file_name() == "target" && entry.file_type().is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use walkdir::DirEntry;
    use std::fs::{self, File};
    use tempfile::tempdir;
    
    // Helper function to create a DirEntry from a path
    fn create_dir_entry(path: &PathBuf, is_dir: bool) -> DirEntry {
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .find(|e| e.path() == path && e.file_type().is_dir() == is_dir)
            .expect("Failed to create DirEntry")
    }
    
    #[test]
    fn test_is_hidden() {
        let temp_dir = tempdir().unwrap();
        
        // Create a hidden directory
        let hidden_dir_path = temp_dir.path().join(".hidden_dir");
        fs::create_dir(&hidden_dir_path).unwrap();
        
        // Create a regular directory
        let regular_dir_path = temp_dir.path().join("regular_dir");
        fs::create_dir(&regular_dir_path).unwrap();
        
        // Create a hidden file
        let hidden_file_path = temp_dir.path().join(".hidden_file");
        File::create(&hidden_file_path).unwrap();
        
        // Create a regular file
        let regular_file_path = temp_dir.path().join("regular_file");
        File::create(&regular_file_path).unwrap();
        
        // Test hidden directory
        let hidden_dir_entry = create_dir_entry(&hidden_dir_path, true);
        assert!(is_hidden(&hidden_dir_entry));
        
        // Test regular directory
        let regular_dir_entry = create_dir_entry(&regular_dir_path, true);
        assert!(!is_hidden(&regular_dir_entry));
        
        // Test hidden file
        let hidden_file_entry = create_dir_entry(&hidden_file_path, false);
        assert!(is_hidden(&hidden_file_entry));
        
        // Test regular file
        let regular_file_entry = create_dir_entry(&regular_file_path, false);
        assert!(!is_hidden(&regular_file_entry));
    }
    
    #[test]
    fn test_is_target_dir() {
        let temp_dir = tempdir().unwrap();
        
        // Create a target directory
        let target_dir_path = temp_dir.path().join("target");
        fs::create_dir(&target_dir_path).unwrap();
        
        // Create a regular directory
        let regular_dir_path = temp_dir.path().join("regular_dir");
        fs::create_dir(&regular_dir_path).unwrap();
        
        // Create a file named "target"
        let target_file_path = temp_dir.path().join("target_file");
        File::create(&target_file_path).unwrap();
        
        // Test target directory
        let target_dir_entry = create_dir_entry(&target_dir_path, true);
        assert!(is_target_dir(&target_dir_entry));
        
        // Test regular directory
        let regular_dir_entry = create_dir_entry(&regular_dir_path, true);
        assert!(!is_target_dir(&regular_dir_entry));
        
        // Test file named "target" (should return false since it's not a directory)
        let target_file_entry = create_dir_entry(&target_file_path, false);
        assert!(!is_target_dir(&target_file_entry));
    }
} 