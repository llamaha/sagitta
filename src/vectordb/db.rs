use crate::vectordb::cache::EmbeddingCache;
use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWConfig, HNSWIndex, HNSWStats};
use crate::vectordb::search::result::SearchResult;
use crate::vectordb::snippet_extractor;
use indicatif::style::TemplateError;
use log::{debug, error, warn};
//use rayon::iter::ParallelIterator;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};
use crate::vectordb::embedding_logic::EmbeddingHandler;
use super::indexing;
use std::fmt;

// Add From implementation here
impl From<TemplateError> for VectorDBError {
    fn from(error: TemplateError) -> Self {
        VectorDBError::GeneralError(format!("Progress bar template error: {}", error))
    }
}

/// Relevance feedback data for a query-file pair
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FeedbackEntry {
    pub relevant_count: usize,
    pub irrelevant_count: usize,
    pub relevance_score: f32,
}

/// Collection of query feedback data
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FeedbackData {
    pub query_feedback: HashMap<String, HashMap<String, FeedbackEntry>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndexedChunk {
    pub id: usize,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    // Embedding vector is stored in HNSW, not duplicated here by default
    pub embedding: Vec<f32>,
    pub last_modified: SystemTime,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DBFile {
    indexed_chunks: Vec<IndexedChunk>,
    hnsw_config: Option<HNSWConfig>,
    feedback: Option<FeedbackData>,
    embedding_model_type: Option<EmbeddingModelType>,
    onnx_model_path: Option<String>,
    onnx_tokenizer_path: Option<String>,
    #[serde(default)]
    indexed_roots: HashMap<String, u64>,
}

/// Configuration for creating or loading a VectorDB instance.
#[derive(Clone, Debug)]
pub struct VectorDBConfig {
    /// Path to the main database file (e.g., "data/db.json").
    /// The cache (`cache.json`) and HNSW index (`hnsw_index.json`) will be stored
    /// in the same directory.
    pub db_path: String,
    /// Path to the ONNX embedding model file (e.g., `model.onnx`).
    pub onnx_model_path: PathBuf,
    /// Path to the ONNX tokenizer configuration file (e.g., `tokenizer.json`).
    pub onnx_tokenizer_path: PathBuf,
}

/// The main vector database struct.
///
/// Handles indexing directories, generating embeddings, storing data,
/// and performing semantic search.
#[derive(Clone)]
pub struct VectorDB {
    /// Stores the metadata and text for each indexed chunk. Embeddings are primarily in HNSW.
    pub(crate) indexed_chunks: Vec<IndexedChunk>,
    /// Path to the primary database file (`db.json`).
    pub(crate) db_path: String,
    /// Manages caching of file hashes and timestamps to speed up re-indexing.
    pub(crate) cache: EmbeddingCache,
    /// The HNSW index for fast approximate nearest neighbor search. Loaded on demand.
    pub(crate) hnsw_index: Option<HNSWIndex>,
    /// Stores user feedback for relevance tuning (future feature).
    pub(crate) feedback: FeedbackData,
    /// Handles loading and using the embedding model.
    pub(crate) embedding_handler: EmbeddingHandler,
    /// Tracks the root directories that have been indexed and their last index timestamp.
    pub(crate) indexed_roots: HashMap<String, u64>,
}

impl fmt::Debug for VectorDB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VectorDB")
         .field("db_path", &self.db_path)
         .field("indexed_chunks_count", &self.indexed_chunks.len())
         .field("cache_len", &self.cache.len())
         .field("has_hnsw_index", &self.hnsw_index.is_some())
         .field("embedding_handler", &self.embedding_handler)
         .field("indexed_roots_count", &self.indexed_roots.len())
         .finish()
    }
}

impl VectorDB {
    /// Creates a new VectorDB instance or loads an existing one from the specified `db_path`.
    ///
    /// The `VectorDBConfig` requires paths to the database file, ONNX model, and tokenizer.
    /// If the `db_path` exists, it attempts to load the database, cache, and HNSW index.
    /// If loading fails or files are missing, it initializes an empty database structure.
    ///
    /// # Errors
    ///
    /// Returns `VectorDBError` if:
    /// - The provided ONNX model or tokenizer paths do not exist.
    /// - There are issues reading or deserializing existing database files.
    /// - The embedding model cannot be loaded.
    pub fn new(config: VectorDBConfig) -> Result<Self> {
        debug!("Creating VectorDB with config: {:?}", config);
        let db_path = config.db_path.clone();

        // Validate config paths early
        if !config.onnx_model_path.exists() { return Err(VectorDBError::FileNotFound(config.onnx_model_path.display().to_string())); }
        if !config.onnx_tokenizer_path.exists() { return Err(VectorDBError::FileNotFound(config.onnx_tokenizer_path.display().to_string())); }

        // Load existing DBFile data if it exists
        let (
            indexed_chunks,
            _hnsw_config,
            feedback,
            loaded_model_type, // Still need to load this for cache compatibility check
            _loaded_onnx_model_path, // Not directly used anymore, handler takes from config
            _loaded_onnx_tokenizer_path, // Not directly used anymore
            indexed_roots,
        ) = if Path::new(&db_path).exists() {
            debug!("Database file exists, attempting to load");
            match fs::read_to_string(&db_path) {
                Ok(contents) => {
                    debug!("Database file read successfully, parsing JSON");
                    let db_file: DBFile = serde_json::from_str(&contents)?;
                    // Determine model type: Use saved type, default to Onnx if not present
                    let model_type_from_db = db_file.embedding_model_type.unwrap_or(EmbeddingModelType::Onnx);
                    debug!(
                        "Database parsed successfully: {} indexed chunks, {} indexed roots, model type: {:?}",
                        db_file.indexed_chunks.len(),
                        db_file.indexed_roots.len(),
                        model_type_from_db, // Log the loaded type
                    );
                    (
                        db_file.indexed_chunks,
                        db_file.hnsw_config,
                        db_file.feedback.unwrap_or_default(),
                        model_type_from_db, // Use the loaded type
                        db_file.onnx_model_path.map(PathBuf::from),
                        db_file.onnx_tokenizer_path.map(PathBuf::from),
                        db_file.indexed_roots,
                    )
                }
                Err(e) => {
                    error!("Couldn't read database file: {}", e);
                    eprintln!("Warning: Couldn't read database file: {}", e);
                    eprintln!("Creating a new empty database.");
                    ( Vec::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Onnx, None, None, HashMap::new() )
                }
            }
        } else {
            debug!("Database file doesn't exist, creating new database");
            // Default to Onnx for new DB
            ( Vec::new(), Some(HNSWConfig::default()), FeedbackData::default(), EmbeddingModelType::Onnx, None, None, HashMap::new() )
        };

        // Create EmbeddingHandler using paths from the *config*, not the loaded DB file.
        // The type is also implicitly Onnx because we require ONNX paths in the config.
        let embedding_handler = EmbeddingHandler::new(
            EmbeddingModelType::Onnx, // Type is Onnx due to config requirements
            Some(config.onnx_model_path.clone()),
            Some(config.onnx_tokenizer_path.clone()),
        )?;

        let cache_path = Path::new(&db_path).parent().unwrap_or_else(|| Path::new(".")).join("cache.json").to_string_lossy().to_string();
        debug!("Creating embedding cache at: {}", cache_path);
        let mut cache = EmbeddingCache::new(cache_path)?;
        // Set cache model type based on the *loaded* type from DB file for consistency check
        debug!("Setting cache model type to loaded type: {:?}", loaded_model_type);
        cache.set_model_type(loaded_model_type.clone()); // Use loaded type here

        let hnsw_path = Path::new(&db_path).parent().unwrap_or_else(|| Path::new(".")).join("hnsw_index.json");
        debug!("Looking for HNSW index at: {}", hnsw_path.display());
        let hnsw_index = if hnsw_path.exists() {
             match HNSWIndex::load_from_file(&hnsw_path) {
                 Ok(index) => {
                    // Use the handler's dimension for compatibility check
                    let expected_dim = match embedding_handler.create_embedding_model() {
                        Ok(model) => model.dim(),
                        Err(_) => embedding_handler.embedding_model_type().default_dimension(), // Fallback
                    };
                     // let expected_dim = hnsw_config.map(|c| c.dimension).unwrap_or_else(|| loaded_model_type.default_dimension()); // Old way
                     if index.get_config().dimension == expected_dim {
                         Some(index)
                     } else {
                         warn!("Loaded HNSW index dimension mismatch (loaded {}, expected {}). Discarding index.", index.get_config().dimension, expected_dim);
                         let _ = fs::remove_file(&hnsw_path);
                         None
                     }
                 }
                 Err(e) => {
                     error!("Couldn't load HNSW index: {}. Discarding.", e);
                     let _ = fs::remove_file(&hnsw_path);
                     None
                 }
             }
        } else {
             None
        };

        // Initialize the VectorDB struct
        Ok(Self {
            indexed_chunks,
            db_path,
            cache,
            hnsw_index,
            feedback,
            embedding_handler, // Store the handler instance
            indexed_roots,
        })
    }

    pub fn set_onnx_paths(
        &mut self,
        model_path: Option<PathBuf>,
        tokenizer_path: Option<PathBuf>,
    ) -> Result<()> {
        // Delegate path setting to the handler
        self.embedding_handler.set_onnx_paths(model_path, tokenizer_path)?;

        // Update cache based on the handler's current type
        let current_model_type = self.embedding_handler.embedding_model_type();
        self.cache.set_model_type(current_model_type);
        self.cache.invalidate_different_model_types();

        // Save the updated paths by saving the whole DB state
        // (save() will now read paths from the handler)
        self.save()?;

        Ok(())
    }

    // ---- Indexing Related Methods (Delegated to indexing module) ----

    /// Indexes the files within a specified directory path.
    ///
    /// Recursively scans the directory, identifies supported file types (or uses provided patterns),
    /// extracts text snippets, generates embeddings, and updates the database and HNSW index.
    /// Uses the cache to avoid re-processing unchanged files.
    ///
    /// # Arguments
    ///
    /// * `dir_path` - The path to the directory to index.
    /// * `file_patterns` - Optional list of glob patterns to filter files (e.g., `["*.rs", "*.md"]`).
    ///                    If empty or not provided, uses default supported types.
    ///
    /// # Errors
    ///
    /// Returns `VectorDBError` if the directory path is invalid, file reading fails,
    /// embedding generation fails, or saving the database fails.
    pub fn index_directory(&mut self, dir_path: &str, file_patterns: &[String]) -> Result<()> {
        // Restore delegation call
        indexing::index_directory(self, dir_path, file_patterns)
    }

    /// Removes an indexed directory and its associated data (chunks, cache entries)
    /// from the database.
    ///
    /// Note: This currently removes chunk metadata but may require rebuilding the HNSW index
    /// for the removal to be fully reflected in search results immediately (depending on HNSW implementation).
    /// Consider re-indexing relevant directories or the entire dataset after significant removals.
    ///
    /// # Arguments
    ///
    /// * `dir_path` - The exact path of the directory previously added via `index_directory`.
    ///
    /// # Errors
    ///
    /// Returns `VectorDBError` if saving the database after removal fails.
    pub fn remove_directory(&mut self, dir_path: &str) -> Result<()> {
        // Restore delegation call
        indexing::remove_directory(self, dir_path)
    }

    /// Updates the timestamp for an indexed root directory.
    /// (Internal method, called by indexing functions)
    pub(crate) fn update_indexed_root_timestamp_internal(&mut self, path_str: String, timestamp: u64) {
        // The actual update happens here, called from indexing module
        self.indexed_roots.insert(path_str, timestamp);
    }

    // ---- End Indexing Related Methods ----

    // ---- Other Methods ----

    /// Saves the current state of the database (indexed chunks, HNSW config, model info, roots)
    /// to the `db_path` file. Also saves the cache and HNSW index to their respective files
    /// in the same directory.
    ///
    /// This is often called automatically by methods like `index_directory`, but can be called
    /// manually if needed.
    ///
    /// # Errors
    ///
    /// Returns `VectorDBError` if serialization or file writing fails for the database,
    /// cache, or HNSW index.
    pub fn save(&mut self) -> Result<()> {
        debug!("Saving VectorDB to {}", self.db_path);
        let start = Instant::now();

        // Save HNSW index if it exists
        if let Some(hnsw_index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            debug!("Saving HNSW index to {}", hnsw_path.display());
            if let Err(e) = hnsw_index.save_to_file(&hnsw_path) {
                error!("Failed to save HNSW index: {}", e);
                eprintln!("Warning: Failed to save HNSW index: {}", e);
            } else {
                debug!("HNSW index saved successfully.");
            }
        } else {
            debug!("No HNSW index found, skipping save.");
        }

        // Create DBFile
        let db_file = DBFile {
            indexed_chunks: self.indexed_chunks.clone(),
            hnsw_config: self.hnsw_index.as_ref().map(|idx| idx.get_config().clone()),
            feedback: Some(self.feedback.clone()),
            embedding_model_type: Some(self.embedding_handler.embedding_model_type()),
            onnx_model_path: self.embedding_handler.onnx_model_path().map(|p| p.to_string_lossy().to_string()),
            onnx_tokenizer_path: self.embedding_handler.onnx_tokenizer_path().map(|p| p.to_string_lossy().to_string()),
            indexed_roots: self.indexed_roots.clone(),
        };

        let contents = serde_json::to_string_pretty(&db_file)?;
        fs::write(&self.db_path, contents)?;
        debug!("Saved database file successfully to {}", self.db_path);

        self.cache.save()?;
        debug!("Saved cache successfully.");

        debug!("VectorDB saved in {:.2?}", start.elapsed());
        Ok(())
    }

    /// Clears all indexed data from the database.
    ///
    /// Removes all indexed chunks, clears the cache, deletes the HNSW index file,
    /// and resets the feedback data and indexed roots map. Essentially resets the database
    /// to an empty state.
    ///
    /// # Errors
    ///
    /// Returns `VectorDBError` if deleting the HNSW index file or saving the
    /// (now empty) database file fails.
    pub fn clear(&mut self) -> Result<()> {
        debug!("Clearing VectorDB data");
        self.indexed_chunks.clear();
        self.hnsw_index = None;
        self.feedback = FeedbackData::default();
        self.indexed_roots.clear();

        // Also clear the physical files
        let _ = fs::remove_file(&self.db_path);
        let hnsw_path = Path::new(&self.db_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
        let _ = fs::remove_file(hnsw_path);
        self.cache.clear()?; // Clear cache content and file

        debug!("VectorDB cleared");
        Ok(())
    }

    /// Returns statistics about the current state of the database.
    pub fn stats(&self) -> DBStats {
        // Calculate unique files from indexed_chunks
        let unique_files = self.indexed_chunks.iter()
            .map(|chunk| &chunk.file_path)
            .collect::<HashSet<_>>()
            .len();

        // Get dimension and type from handler
        let embedding_dimension = match self.embedding_handler.create_embedding_model() {
            Ok(model) => model.dim(),
            Err(_) => self.embedding_handler.embedding_model_type().default_dimension(), // Fallback
        };
        let embedding_model_type = self.embedding_handler.embedding_model_type();

        DBStats {
            indexed_chunks: self.indexed_chunks.len(),
            unique_files,
            embedding_dimension,
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
            hnsw_stats: self.hnsw_index.as_ref().map(|idx| idx.stats()),
            embedding_model_type,
        }
    }

    pub fn hnsw_index(&self) -> Option<&HNSWIndex> {
        if let Some(index) = &self.hnsw_index {
            debug!( "HNSW index accessed: {} nodes, {} layers", index.stats().total_nodes, index.stats().layers );
            Some(index)
        } else {
            debug!("HNSW index requested but not available");
            None
        }
    }

    pub fn get_supported_file_types() -> Vec<String> {
        vec![
            "rs".to_string(), "rb".to_string(), "go".to_string(), "js".to_string(), "ts".to_string(),
            "md".to_string(), "yaml".to_string(), "yml".to_string(), "toml".to_string(), "xml".to_string(),
        ]
    }

    // Keep helper for search
    pub fn get_file_path(&self, node_id: usize) -> Option<String> {
        // The HNSW node ID now corresponds directly to the index in indexed_chunks
        self.indexed_chunks.get(node_id).map(|chunk| chunk.file_path.clone())
    }

    // Keep getter for cache
    pub fn cache(&self) -> &EmbeddingCache {
        &self.cache
    }

    // Keep getter for the embedding handler
    pub fn embedding_handler(&self) -> &EmbeddingHandler {
        &self.embedding_handler
    }

    /// Returns a reference to the map of indexed root directories and their last update timestamps.
    ///
    /// The key is the absolute path string of the indexed directory, and the value is the
    /// Unix timestamp (seconds since epoch) of the last successful indexing operation
    /// affecting that root.
    pub fn indexed_roots(&self) -> &HashMap<String, u64> {
        &self.indexed_roots
    }

    /// Performs a semantic search query against the indexed data.
    ///
    /// Embeds the query using the configured ONNX model and searches the HNSW index
    /// for the `limit` most semantically similar text chunks. Optionally filters results
    /// by file type extensions.
    ///
    /// # Arguments
    ///
    /// * `query` - The natural language search query.
    /// * `limit` - The maximum number of search results to return.
    /// * `file_types` - Optional list of file extensions (e.g., `vec!["rs".to_string(), "md".to_string()]`)
    ///                  to filter results. Only chunks from files matching these extensions will be returned.
    ///
    /// # Errors
    ///
    /// Returns `VectorDBError` if:
    /// - The embedding model fails to embed the query.
    /// - The HNSW index is not loaded or built.
    /// - An error occurs during the HNSW search.
    pub fn search(&self, query: &str, limit: usize, file_types: Option<Vec<String>>) -> Result<Vec<SearchResult>> {
        debug!("Starting search for query: '{}', limit: {}, file_types: {:?}", query, limit, file_types);
        let start_time = Instant::now();

        let model = self.embedding_handler.create_embedding_model()?;
        let query_dim = model.dim();

        let hnsw_index = self.hnsw_index.as_ref().ok_or(VectorDBError::IndexNotFound)?;
        let index_dim = hnsw_index.get_config().dimension;

        let query_embedding = model.embed(query)?;

        if index_dim != query_dim {
            return Err(VectorDBError::DimensionMismatch { expected: index_dim, found: query_dim });
        }

        let ef_construction = self.hnsw_index.as_ref().map(|idx| idx.get_config().ef_construction).unwrap_or_else(|| HNSWConfig::default().ef_construction);
        let ef_search = ef_construction.max(limit * 2);
        let hnsw_results = hnsw_index.search_parallel(&query_embedding, limit * 5, ef_search)?;

        let file_type_set: Option<HashSet<String>> = file_types.map(|ft| ft.into_iter().collect());
        let mut search_results: Vec<SearchResult> = Vec::with_capacity(hnsw_results.len());

        for (node_id, distance) in hnsw_results {
            if let Some(chunk) = self.indexed_chunks.get(node_id) {
                let file_path = &chunk.file_path;
                if let Some(ref ft_set) = file_type_set {
                    let extension_os = Path::new(file_path).extension().and_then(|os| os.to_str());
                    if let Some(extension) = extension_os {
                        if !ft_set.contains(&extension.to_lowercase()) { continue; }
                    } else { continue; }
                }
                let snippet_text = match snippet_extractor::extract_snippet(file_path, chunk.start_line, chunk.end_line) {
                    Ok(snippet) => snippet,
                    Err(e) => { warn!("Snippet extraction failed: {}", e); chunk.text.clone() }
                };
                let score = 1.0 - (distance / 2.0).max(0.0).min(1.0);
                search_results.push(SearchResult {
                    file_path: file_path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    text: snippet_text,
                    score,
                });
            } else { error!("Invalid HNSW node ID: {}", node_id); }
        }

        search_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        search_results.truncate(limit);

        let duration = start_time.elapsed();
        debug!("Search completed in {:.2?}", duration);
        Ok(search_results)
    }
}

/// Contains statistics about the VectorDB instance.
#[derive(Serialize, Deserialize, Debug)]
pub struct DBStats {
    /// Total number of text chunks indexed across all files.
    pub indexed_chunks: usize,
    /// Number of unique files contributing to the indexed chunks.
    pub unique_files: usize,
    /// The dimensionality of the embeddings used by the model.
    pub embedding_dimension: usize,
    /// Path to the main database file.
    pub db_path: String,
    /// Number of files currently tracked in the cache.
    pub cached_files: usize,
    /// Statistics about the HNSW index, if loaded.
    pub hnsw_stats: Option<HNSWStats>,
    /// The type of embedding model configured.
    pub embedding_model_type: EmbeddingModelType,
}

impl Clone for DBStats {
    fn clone(&self) -> Self {
        Self {
            indexed_chunks: self.indexed_chunks,
            unique_files: self.unique_files,
            embedding_dimension: self.embedding_dimension,
            db_path: self.db_path.clone(),
            cached_files: self.cached_files,
            hnsw_stats: self.hnsw_stats.clone(),
            embedding_model_type: self.embedding_model_type.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectordb::error::Result;
    use tempfile::tempdir;
    use std::fs;
    use std::path::PathBuf;

    // Helper function to set up a test database environment
    fn setup_db_test_env() -> (tempfile::TempDir, String) { 
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db.json");
        let db_path_str = db_path.to_str().unwrap().to_string();

        // Ensure the directory exists
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        // Don't create/index db here, let tests do it.
        (temp_dir, db_path_str)
    }

    // Helper to create default config for tests
    fn default_test_config(db_path: String) -> VectorDBConfig {
        // Use the actual default paths
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json"); 
        VectorDBConfig {
            db_path,
            onnx_model_path: model_path,
            onnx_tokenizer_path: tokenizer_path,
        }
    }

    // Helper function to check if default ONNX files exist
    fn check_default_onnx_files() -> bool {
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
        if !model_path.exists() || !tokenizer_path.exists() {
            warn!("Default ONNX files not found in ./onnx/. Ignoring test.");
            true // Return true to ignore
        } else {
            false // Return false to run test
        }
    }

    #[test]
    fn test_vectordb_new_empty() -> Result<()> {
        if check_default_onnx_files() { return Ok(()); }
        let (_temp_dir, db_path_str) = setup_db_test_env();
        let db_result = VectorDB::new(default_test_config(db_path_str));
        assert!(db_result.is_err(), "Should fail if ONNX files are missing");
        // Expect FileNotFound error now
        assert!(matches!(db_result, Err(VectorDBError::FileNotFound(_))), "Expected FileNotFound, got {:?}", db_result);
        Ok(())
    }

    #[test]
    fn test_vectordb_save_load() -> Result<()> {
        if check_default_onnx_files() {
            println!("Skipping test_vectordb_save_load due to missing ONNX files.");
            return Ok(());
        }
        let (_temp_dir, db_path_str) = setup_db_test_env();
        let mut db1 = VectorDB::new(default_test_config(db_path_str.clone()))?;
        db1.indexed_chunks.push(IndexedChunk {
            id: 0,
            file_path: "test/file1.txt".to_string(),
            start_line: 1,
            end_line: 10,
            text: "chunk 1".to_string(),
            embedding: vec![0.1; 384],
            last_modified: SystemTime::now(),
            metadata: None,
        });
        db1.indexed_roots.insert("test".to_string(), SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());

        // Manually create and insert into HNSW for test purposes
        let dim = db1.indexed_chunks[0].embedding.len();
        db1.hnsw_index = Some(HNSWIndex::new(HNSWConfig::new(dim)));
        let _internal_hnsw_id = db1.hnsw_index.as_mut().unwrap().insert(db1.indexed_chunks[0].embedding.clone())?;

        assert!(db1.embedding_handler.onnx_model_path().is_some());
        assert!(db1.embedding_handler.onnx_tokenizer_path().is_some());

        db1.save()?;
        let db2 = VectorDB::new(default_test_config(db_path_str))?;

        assert_eq!(db2.indexed_chunks.len(), 1, "Loaded DB should have 1 chunk");
        assert_eq!(db2.indexed_chunks[0].id, 0);
        assert_eq!(db2.indexed_chunks[0].file_path, "test/file1.txt");
        assert_eq!(db2.indexed_chunks[0].embedding.len(), 384);
        assert_eq!(db2.indexed_roots.len(), 1, "Loaded DB should have 1 indexed root");
        assert!(db2.indexed_roots.contains_key("test"));
        assert!(db2.embedding_handler.onnx_model_path().is_some());
        assert!(db2.embedding_handler.onnx_tokenizer_path().is_some());
        assert_eq!(db2.embedding_handler.embedding_model_type(), EmbeddingModelType::Onnx);
        assert!(db2.hnsw_index.is_some(), "Loaded DB should have HNSW index");
        assert_eq!(db2.hnsw_index.as_ref().unwrap().get_config().dimension, dim);
        assert_eq!(db2.hnsw_index.as_ref().unwrap().len(), 1);
        Ok(())
    }

    #[test]
    fn test_vectordb_clear() -> Result<()> {
        if check_default_onnx_files() {
            println!("Skipping test_vectordb_clear due to missing ONNX files.");
            return Ok(());
        }
        let (_temp_dir, db_path_str) = setup_db_test_env();
        let config = default_test_config(db_path_str.clone());
        let mut db = VectorDB::new(config.clone())?;
        db.indexed_chunks.push(IndexedChunk {
            id: 0,
            file_path: "dummy".to_string(),
            start_line: 1,
            end_line: 1,
            text: "t".to_string(),
            embedding: vec![0.0; 384],
            last_modified: SystemTime::now(),
            metadata: None,
        });
        db.indexed_roots.insert("root".to_string(), SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
        let dim = db.indexed_chunks[0].embedding.len();
        db.hnsw_index = Some(HNSWIndex::new(HNSWConfig::new(dim)));
        let _ = db.hnsw_index.as_mut().unwrap().insert(db.indexed_chunks[0].embedding.clone())?;
        db.cache.insert_file_hash("dummy".to_string(), 123)?;

        assert!(!db.indexed_chunks.is_empty());
        assert!(db.hnsw_index.is_some());
        assert!(!db.indexed_roots.is_empty());
        assert!(db.cache.len() > 0);

        db.clear()?;
        assert!(db.indexed_chunks.is_empty(), "DB chunks should be empty after clear");
        assert!(db.indexed_roots.is_empty(), "DB indexed roots should be empty after clear");
        assert!(db.hnsw_index.is_none(), "HNSW index should be None after clear");
        assert!(db.cache.len() == 0);

        let db_path = Path::new(&db_path_str);
        assert!(!db_path.exists(), "DB file should be removed after clear");
        let hnsw_path = db_path.parent().unwrap().join("hnsw_index.json");
        assert!(!hnsw_path.exists(), "HNSW file should be removed after clear");
        let cache_path = db_path.parent().unwrap().join("cache.json");
        assert!(!cache_path.exists(), "Cache file should not exist after clear");

        let db_reloaded = VectorDB::new(config)?;
        assert!(db_reloaded.indexed_chunks.is_empty(), "Reloaded DB chunks should be empty after clear");
        assert!(db_reloaded.indexed_roots.is_empty(), "Reloaded DB indexed roots should be empty after clear");
        assert!(db_reloaded.hnsw_index.is_none());
        Ok(())
    }

    #[test]
    #[ignore]
    fn test_vectordb_stats() -> Result<()> {
        Ok(())
    }

    #[test]
    fn test_vectordb_set_onnx_paths_valid() -> Result<()> {
        if check_default_onnx_files() { return Ok(()); }
        let (_temp_dir, db_path) = setup_db_test_env();
        let mut db = VectorDB::new(default_test_config(db_path))?;
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        let tok_path = PathBuf::from("onnx/minilm_tokenizer.json");

        // Get initial paths from handler
        let initial_model_path = db.embedding_handler.onnx_model_path().cloned();
        let initial_tok_path = db.embedding_handler.onnx_tokenizer_path().cloned();

        // Set the same paths again (should succeed)
        let result = db.set_onnx_paths(Some(model_path.clone()), Some(tok_path.clone()));
        assert!(result.is_ok());

        // Check paths in handler are still the same
        assert_eq!(db.embedding_handler.onnx_model_path(), Some(&model_path));
        assert_eq!(db.embedding_handler.onnx_tokenizer_path(), Some(&tok_path));

        // Optionally, set back to original if they were different, or test setting None
        let result_none = db.set_onnx_paths(None, None);
        assert!(result_none.is_ok()); // Setting None is allowed
        assert!(db.embedding_handler.onnx_model_path().is_none());
        assert!(db.embedding_handler.onnx_tokenizer_path().is_none());

        // Set back to originals if needed for other tests potentially
        let _ = db.set_onnx_paths(initial_model_path, initial_tok_path);

        Ok(())
    }

    #[test]
    fn test_vectordb_set_onnx_paths_invalid() -> Result<()> {
        if check_default_onnx_files() { return Ok(()); }
        let (_temp_dir, db_path) = setup_db_test_env();
        let mut db = VectorDB::new(default_test_config(db_path))?;
        let non_existent_path = _temp_dir.path().join("non_existent.onnx");

        // Test invalid model path
        let result_model = db.set_onnx_paths(Some(non_existent_path.clone()), db.embedding_handler.onnx_tokenizer_path().cloned());
        assert!(matches!(result_model, Err(VectorDBError::EmbeddingError(_))), "Setting non-existent model path should fail");
        assert!(result_model.unwrap_err().to_string().contains("model file not found"));

        // Test invalid tokenizer path
         let result_tok = db.set_onnx_paths(db.embedding_handler.onnx_model_path().cloned(), Some(non_existent_path.clone()));
         assert!(matches!(result_tok, Err(VectorDBError::EmbeddingError(_))), "Setting non-existent tokenizer path should fail");
         assert!(result_tok.unwrap_err().to_string().contains("tokenizer file not found"));

        Ok(())
    }

    #[test]
    fn test_create_embedding_model_via_handler() -> Result<()> {
        if check_default_onnx_files() { return Ok(()); }
        let (_temp_dir, db_path) = setup_db_test_env();
        let db = VectorDB::new(default_test_config(db_path))?;

        // Create model via handler
        let model_result = db.embedding_handler.create_embedding_model();
        assert!(model_result.is_ok());
        let model = model_result.unwrap();
        assert!(model.dim() > 0); // Check if dimension is reasonable

        Ok(())
    }

    #[test]
    fn test_create_embedding_model_fails_if_paths_none() -> Result<()> {
        if check_default_onnx_files() { return Ok(()); }
        let (_temp_dir, db_path) = setup_db_test_env();
        let mut db = VectorDB::new(default_test_config(db_path))?;

        // Set paths to None
        db.set_onnx_paths(None, None)?;

        // Attempt to create model
        let model_result = db.embedding_handler.create_embedding_model();
        assert!(model_result.is_err());
        assert!(matches!(model_result, Err(VectorDBError::EmbeddingError(_))));
        assert!(model_result.unwrap_err().to_string().contains("paths not set"));

        Ok(())
    }
}