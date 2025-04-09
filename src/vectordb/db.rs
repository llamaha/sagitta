use crate::vectordb::cache::{CacheCheckResult, EmbeddingCache};
use crate::vectordb::embedding::{EmbeddingModel, EmbeddingModelType, EMBEDDING_DIM};
use crate::vectordb::error::{Result, VectorDBError};
use crate::vectordb::hnsw::{HNSWConfig, HNSWIndex, HNSWStats};
use crate::vectordb::onnx::ONNX_EMBEDDING_DIM;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, warn};
use num_cpus;
use rayon::iter::ParallelIterator;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

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

#[derive(Serialize, Deserialize)]
struct DBFile {
    embeddings: HashMap<String, Vec<f32>>,
    hnsw_config: Option<HNSWConfig>,
    feedback: Option<FeedbackData>,
    embedding_model_type: Option<EmbeddingModelType>,
    onnx_model_path: Option<String>,
    onnx_tokenizer_path: Option<String>,
}

pub struct VectorDB {
    pub embeddings: HashMap<String, Vec<f32>>,
    db_path: String,
    cache: EmbeddingCache,
    pub hnsw_index: Option<HNSWIndex>,
    feedback: FeedbackData,
    embedding_model_type: EmbeddingModelType,
    onnx_model_path: Option<PathBuf>,
    onnx_tokenizer_path: Option<PathBuf>,
}

impl Clone for VectorDB {
    fn clone(&self) -> Self {
        Self {
            embeddings: self.embeddings.clone(),
            db_path: self.db_path.clone(),
            cache: self.cache.clone(),
            hnsw_index: self.hnsw_index.clone(),
            feedback: self.feedback.clone(),
            embedding_model_type: self.embedding_model_type.clone(),
            onnx_model_path: self.onnx_model_path.clone(),
            onnx_tokenizer_path: self.onnx_tokenizer_path.clone(),
        }
    }
}

impl VectorDB {
    pub fn new(db_path: String) -> Result<Self> {
        debug!("Creating VectorDB with database path: {}", db_path);

        let (
            embeddings,
            hnsw_config,
            feedback,
            embedding_model_type,
            onnx_model_path,
            onnx_tokenizer_path,
        ) = if Path::new(&db_path).exists() {
            debug!("Database file exists, attempting to load");
            match fs::read_to_string(&db_path) {
                Ok(contents) => {
                    debug!("Database file read successfully, parsing JSON");
                    match serde_json::from_str::<DBFile>(&contents) {
                        Ok(db_file) => {
                            debug!(
                                "Database parsed successfully: {} embeddings",
                                db_file.embeddings.len()
                            );
                            (
                                db_file.embeddings,
                                db_file.hnsw_config,
                                db_file.feedback.unwrap_or_default(),
                                db_file.embedding_model_type.unwrap_or_default(),
                                db_file.onnx_model_path.map(PathBuf::from),
                                db_file.onnx_tokenizer_path.map(PathBuf::from),
                            )
                        }
                        Err(e) => {
                            error!("Database file appears to be corrupted: {}", e);
                            eprintln!("Warning: Database file appears to be corrupted: {}", e);
                            eprintln!("Creating a new empty database.");
                            let _ = fs::remove_file(&db_path);
                            debug!("Creating a new empty database");
                            (
                                HashMap::new(),
                                Some(HNSWConfig::default()),
                                FeedbackData::default(),
                                EmbeddingModelType::Fast,
                                None,
                                None,
                            )
                        }
                    }
                }
                Err(e) => {
                    error!("Couldn't read database file: {}", e);
                    eprintln!("Warning: Couldn't read database file: {}", e);
                    eprintln!("Creating a new empty database.");
                    debug!("Creating a new empty database");
                    (
                        HashMap::new(),
                        Some(HNSWConfig::default()),
                        FeedbackData::default(),
                        EmbeddingModelType::Fast,
                        None,
                        None,
                    )
                }
            }
        } else {
            debug!("Database file doesn't exist, creating new database");
            (
                HashMap::new(),
                Some(HNSWConfig::default()),
                FeedbackData::default(),
                EmbeddingModelType::Fast,
                None,
                None,
            )
        };

        let cache_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache.json")
            .to_string_lossy()
            .to_string();
        debug!("Creating embedding cache at: {}", cache_path);

        let mut cache = match EmbeddingCache::new(cache_path.clone()) {
            Ok(cache) => {
                debug!("Cache loaded successfully");
                cache
            }
            Err(e) => {
                error!("Couldn't load cache: {}", e);
                eprintln!("Warning: Couldn't load cache: {}", e);
                eprintln!("Creating a new empty cache.");
                let _ = fs::remove_file(&cache_path);
                debug!("Creating a new empty cache");
                EmbeddingCache::new(cache_path)?
            }
        };

        debug!("Setting cache model type to: {:?}", embedding_model_type);
        cache.set_model_type(embedding_model_type.clone());

        let hnsw_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("hnsw_index.json");
        debug!("Looking for HNSW index at: {}", hnsw_path.display());

        let hnsw_index = if hnsw_path.exists() {
            debug!("HNSW index file exists, attempting to load");
            match HNSWIndex::load_from_file(&hnsw_path) {
                Ok(index) => {
                    debug!("HNSW index loaded successfully");
                    Some(index)
                }
                Err(e) => {
                    error!("Couldn't load HNSW index: {}", e);
                    eprintln!("Warning: Couldn't load HNSW index: {}", e);
                    eprintln!("Creating a new index or rebuilding from embeddings.");
                    let _ = fs::remove_file(&hnsw_path);
                    debug!("Rebuilding HNSW index from embeddings");
                    hnsw_config.map(|config| {
                        let mut index = HNSWIndex::new(config);
                        for (_, embedding) in &embeddings {
                            let _ = index.insert(embedding.clone());
                        }
                        debug!("HNSW index rebuilt with {} embeddings", embeddings.len());
                        index
                    })
                }
            }
        } else {
            debug!("No HNSW index file found, creating new index");
            let config = hnsw_config.unwrap_or_else(HNSWConfig::default);
            let mut index = HNSWIndex::new(config);
            for (_, embedding) in &embeddings {
                let _ = index.insert(embedding.clone());
            }
            debug!(
                "New HNSW index created with {} embeddings",
                embeddings.len()
            );
            Some(index)
        };

        debug!("VectorDB initialization complete");
        Ok(Self {
            embeddings,
            db_path,
            cache,
            hnsw_index,
            feedback,
            embedding_model_type,
            onnx_model_path,
            onnx_tokenizer_path,
        })
    }

    pub fn set_onnx_paths(
        &mut self,
        model_path: Option<PathBuf>,
        tokenizer_path: Option<PathBuf>,
    ) -> Result<()> {
        if let Some(model_path) = &model_path {
            if !model_path.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX model file not found: {}",
                    model_path.display()
                )));
            }
        }
        if let Some(tokenizer_path) = &tokenizer_path {
            if !tokenizer_path.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX tokenizer file not found: {}",
                    tokenizer_path.display()
                )));
            }
        }

        if let (Some(model_path_ref), Some(tokenizer_path_ref)) = (&model_path, &tokenizer_path) {
            match EmbeddingModel::new_onnx(model_path_ref, tokenizer_path_ref) {
                Ok(_) => {
                    self.onnx_model_path = model_path;
                    self.onnx_tokenizer_path = tokenizer_path;
                    self.cache.set_model_type(EmbeddingModelType::Onnx);
                    self.cache.invalidate_different_model_types();
                    self.save()?;
                }
                Err(e) => {
                    return Err(VectorDBError::EmbeddingError(format!(
                        "Failed to initialize ONNX model with provided paths: {}",
                        e
                    )));
                }
            }
        } else {
            self.onnx_model_path = model_path;
            self.onnx_tokenizer_path = tokenizer_path;
            self.save()?;
        }
        Ok(())
    }

    pub fn set_embedding_model_type(&mut self, model_type: EmbeddingModelType) -> Result<()> {
        if model_type == EmbeddingModelType::Onnx {
            if self.onnx_model_path.is_none() || self.onnx_tokenizer_path.is_none() {
                return Err(VectorDBError::EmbeddingError(
                    "Cannot set ONNX model type: model or tokenizer paths not set".to_string(),
                ));
            }
            let onnx_model_path = self.onnx_model_path.as_ref().unwrap();
            let onnx_tokenizer_path = self.onnx_tokenizer_path.as_ref().unwrap();
            if !onnx_model_path.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX model file not found: {}",
                    onnx_model_path.display()
                )));
            }
            if !onnx_tokenizer_path.exists() {
                return Err(VectorDBError::EmbeddingError(format!(
                    "ONNX tokenizer file not found: {}",
                    onnx_tokenizer_path.display()
                )));
            }
            match EmbeddingModel::new_onnx(onnx_model_path, onnx_tokenizer_path) {
                Ok(_) => self.embedding_model_type = model_type,
                Err(e) => {
                    return Err(VectorDBError::EmbeddingError(format!(
                        "Failed to initialize ONNX model: {}",
                        e
                    )));
                }
            }
        } else {
            self.embedding_model_type = model_type;
        }
        self.save()?;
        Ok(())
    }

    pub fn embedding_model_type(&self) -> &EmbeddingModelType {
        &self.embedding_model_type
    }

    pub fn create_embedding_model(&self) -> Result<EmbeddingModel> {
        match &self.embedding_model_type {
            EmbeddingModelType::Fast => Ok(EmbeddingModel::new()),
            EmbeddingModelType::Onnx => {
                if let (Some(model_path), Some(tokenizer_path)) =
                    (&self.onnx_model_path, &self.onnx_tokenizer_path)
                {
                    EmbeddingModel::new_onnx(model_path, tokenizer_path)
                        .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))
                } else {
                    Err(VectorDBError::EmbeddingError(
                        "ONNX model paths not set. Environment variables VECTORDB_ONNX_MODEL and VECTORDB_ONNX_TOKENIZER are required".to_string()
                    ))
                }
            }
        }
    }

    pub fn set_hnsw_config(&mut self, config: Option<HNSWConfig>) {
        if let Some(config) = config {
            let mut current_config = config;
            if !self.embeddings.is_empty() {
                let dataset_size = self.embeddings.len();
                let optimal_layers = HNSWConfig::calculate_optimal_layers(dataset_size);
                current_config.num_layers = optimal_layers;
            }
            let mut index = HNSWIndex::new(current_config);
            for (_, embedding) in &self.embeddings {
                let _ = index.insert(embedding.clone());
            }
            self.hnsw_index = Some(index);
        } else {
            self.hnsw_index = None;
        }
    }

    pub fn rebuild_hnsw_index(&mut self) -> Result<()> {
        if self.hnsw_index.is_none() {
            return Ok(());
        }
        let current_config = self.hnsw_index.as_ref().unwrap().get_config();
        let dataset_size = self.embeddings.len();
        let optimal_layers = HNSWConfig::calculate_optimal_layers(dataset_size);
        if current_config.num_layers == optimal_layers {
            return Ok(());
        }
        let mut new_config = current_config.clone();
        new_config.num_layers = optimal_layers;
        if let Some(index) = &self.hnsw_index {
            let new_index = index.rebuild_with_config_parallel(new_config)?;
            self.hnsw_index = Some(new_index);
            self.save()?;
        }
        Ok(())
    }

    pub fn index_file(&mut self, file_path: &Path) -> Result<()> {
        self.index_file_without_save(file_path)?;
        self.save()?;
        Ok(())
    }

    pub fn index_file_without_save(&mut self, file_path: &Path) -> Result<()> {
        let file_path_str = file_path.to_string_lossy().to_string();
        if let Some(cached_embedding) = self.cache.get(&file_path_str) {
            self.embeddings
                .insert(file_path_str.clone(), cached_embedding.to_vec());
            if let Some(index) = &mut self.hnsw_index {
                index.insert(cached_embedding.to_vec())?;
            }
            return Ok(());
        }
        let model = self.create_embedding_model().map_err(|e| {
            if self.embedding_model_type == EmbeddingModelType::Onnx {
                eprintln!("Error creating ONNX embedding model: {}", e);
                if self.onnx_model_path.is_none() || self.onnx_tokenizer_path.is_none() {
                    eprintln!(
                        "ONNX model paths missing - model: {:?}, tokenizer: {:?}",
                        self.onnx_model_path, self.onnx_tokenizer_path
                    );
                } else {
                    eprintln!("ONNX model paths are set but model creation failed");
                }
            }
            VectorDBError::EmbeddingError(e.to_string())
        })?;
        let contents = fs::read_to_string(file_path).map_err(|e| VectorDBError::IOError(e))?;
        let embedding = model
            .embed(&contents)
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
        let file_hash = EmbeddingCache::get_file_hash(file_path)?;
        self.cache
            .insert(file_path_str.clone(), embedding.clone(), file_hash)?;
        self.embeddings.insert(file_path_str, embedding.clone());
        if let Some(index) = &mut self.hnsw_index {
            index.insert(embedding)?;
        }
        Ok(())
    }

    pub fn index_directory(&mut self, dir: &str, file_types: &[String]) -> Result<()> {
        let dir_path = Path::new(dir);
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(VectorDBError::DirectoryNotFound(dir.to_string()));
        }
        debug!("Starting directory scan for files to index in {}", dir);
        let files: Vec<PathBuf> = if file_types.is_empty()
            && self.embedding_model_type == EmbeddingModelType::Fast
        {
            debug!("Using fast model with no file types specified - indexing all non-binary files");
            WalkDir::new(dir_path)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    if !entry.file_type().is_file() { return false; }
                    if let Ok(file) = std::fs::File::open(entry.path()) {
                        let mut buffer = [0u8; 512];
                        let mut reader = std::io::BufReader::new(file);
                        if let Ok(bytes_read) = reader.read(&mut buffer) {
                            if bytes_read > 0 { return !buffer[..bytes_read].contains(&0); }
                            }
                        }
                    true
                })
                .map(|entry| entry.path().to_path_buf())
                .collect()
        } else {
            WalkDir::new(dir_path)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_type().is_file()
                        && match entry.path().extension() {
                            Some(ext) => file_types.contains(&ext.to_string_lossy().to_string()),
                            None => false,
                        }
                })
                .map(|entry| entry.path().to_path_buf())
                .collect()
        };

        let file_count = files.len();
        if file_count == 0 {
            println!("No matching files found in the directory.");
            return Ok(());
        }
        println!("Found {} files to index.", file_count);
        let model = Arc::new(self.create_embedding_model()?);
        let batch_size = if file_count < 1000 { 100 } else if file_count < 10000 { 500 } else { 1000 };
        debug!("Using batch size of {} for {} files", batch_size, file_count);
        self.index_directory_parallel(files, model, batch_size)?;
        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        debug!("Saving VectorDB to {}", self.db_path);
        let path = Path::new(&self.db_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| VectorDBError::DirectoryCreationError {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let db_file = DBFile {
            embeddings: self.embeddings.clone(),
            hnsw_config: self.hnsw_index.as_ref().map(|idx| idx.get_config()),
            feedback: Some(self.feedback.clone()),
            embedding_model_type: Some(self.embedding_model_type.clone()),
            onnx_model_path: self.onnx_model_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            onnx_tokenizer_path: self.onnx_tokenizer_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        };
        let json = serde_json::to_string_pretty(&db_file)?;
        fs::write(&self.db_path, &json).map_err(|e| VectorDBError::FileWriteError {
            path: path.to_path_buf(),
            source: e,
        })?;
        if let Some(index) = &self.hnsw_index {
            let hnsw_path = Path::new(&self.db_path)
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                .join("hnsw_index.json");
            if let Some(parent) = hnsw_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            debug!("Saving HNSW index to {}", hnsw_path.display());
            if let Err(e) = index.save_to_file(&hnsw_path) {
                error!("Failed to save HNSW index: {}", e);
                eprintln!("Warning: Failed to save HNSW index: {}", e);
            }
        }
        self.cache.save()?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.embeddings.clear();
        if let Some(index) = &self.hnsw_index {
            let config = index.get_config();
            self.hnsw_index = Some(HNSWIndex::new(config));
        }
        self.cache.clear()?;
        self.feedback = FeedbackData::default();
        self.save()?;
        Ok(())
    }

    pub fn stats(&self) -> DBStats {
        let embedding_dimension = if !self.embeddings.is_empty() {
            self.embeddings.values().next().unwrap().len()
        } else {
            match &self.embedding_model_type {
                EmbeddingModelType::Fast => EMBEDDING_DIM,
                EmbeddingModelType::Onnx => ONNX_EMBEDDING_DIM,
            }
        };
        DBStats {
            indexed_files: self.embeddings.len(),
            embedding_dimension,
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
            hnsw_stats: self.hnsw_index.as_ref().map(|index| index.stats()),
            embedding_model_type: self.embedding_model_type.clone(),
        }
    }

    pub fn nearest_vectors(&mut self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>> {
        if let Some(ref mut index) = self.hnsw_index {
            let ef = 100;
            let results = index.search(query, k, ef)?;
            let mut nearest = Vec::new();
            for (node_id, dist) in results {
                if let Some(file_path) = self.get_file_path(node_id) {
                    let file_path = file_path.clone();
                    let similarity = 1.0 - dist;
                    nearest.push((file_path, similarity));
                }
            }
            Ok(nearest)
        } else {
            let mut results: Vec<_> = self
                .embeddings
                .iter()
                .map(|(path, embedding)| {
                    let distance = Self::cosine_distance(embedding, query);
                    let similarity = 1.0 - distance;
                    (path.clone(), similarity)
                })
                .collect();
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            results.truncate(k);
            Ok(results)
        }
    }

    pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a > 0.0 && norm_b > 0.0 {
            let similarity = dot_product / (norm_a * norm_b);
            let clamped_similarity = similarity.clamp(-1.0, 1.0);
            1.0 - clamped_similarity
        } else {
            1.0
        }
    }

    pub fn get_file_path(&self, node_id: usize) -> Option<&String> {
        self.embeddings.keys().nth(node_id)
    }

    pub fn filter_by_filepath(&self, query: &str, max_files: usize) -> Vec<String> {
        let query = query.to_lowercase();
        let terms: Vec<&str> = query.split_whitespace().filter(|t| t.len() > 1).collect();
        if terms.is_empty() {
            return self.embeddings.keys().take(max_files).cloned().collect();
        }
        let mut scored_paths: Vec<(String, f32)> = self
            .embeddings
            .keys()
            .map(|path| {
                let path_lower = path.to_lowercase();
                let path_segments: Vec<&str> =
                    path_lower.split(|c| c == '/' || c == '\\').collect();
                let filename = path_segments.last().unwrap_or(&"");
                let filename_no_ext = filename.split('.').next().unwrap_or(filename);
                let mut score = 0.0;
                for term in &terms {
                    if filename_no_ext == *term { score += 10.0; }
                    else if filename_no_ext.contains(term) { score += 5.0; }
                    else if path_lower.contains(term) { score += 2.0; }
                    if filename.ends_with(&format!(".{}", term)) { score += 3.0; }
                }
                let depth_penalty = (path_segments.len() as f32 * 0.1).min(1.0);
                score -= depth_penalty;
                if filename.ends_with(".rs") || filename.ends_with(".rb") || filename.ends_with(".go") { score += 1.0; }
                (path.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();
        scored_paths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_paths.into_iter().take(max_files).map(|(path, _)| path).collect()
    }

    pub fn record_feedback( &mut self, query: &str, file_path: &str, is_relevant: bool ) -> Result<()> {
        let query = query.to_lowercase();
        let query_map = self.feedback.query_feedback.entry(query.clone()).or_insert_with(HashMap::new);
        let entry = query_map.entry(file_path.to_string()).or_insert(FeedbackEntry {
            relevant_count: 0, irrelevant_count: 0, relevance_score: 0.5,
        });
        if is_relevant { entry.relevant_count += 1; } else { entry.irrelevant_count += 1; }
        let total = entry.relevant_count + entry.irrelevant_count;
        if total > 0 { entry.relevance_score = entry.relevant_count as f32 / total as f32; }
        self.save()?;
        Ok(())
    }

    pub fn get_feedback_score(&self, query: &str, file_path: &str) -> Option<f32> {
        let query = query.to_lowercase();
        self.feedback.query_feedback.get(&query).and_then(|file_map| file_map.get(file_path)).map(|entry| entry.relevance_score)
    }

    pub fn get_similar_queries(&self, query: &str, max_queries: usize) -> Vec<String> {
        let query = query.to_lowercase();
        let query_terms: Vec<&str> = query.split_whitespace().collect();
        if query_terms.is_empty() { return Vec::new(); }
        let mut scored_queries: Vec<(String, f32)> = self.feedback.query_feedback.keys()
            .filter(|&existing_query| existing_query != &query)
            .map(|existing_query| {
                let existing_terms: Vec<&str> = existing_query.split_whitespace().collect();
                let intersection: Vec<&&str> = query_terms.iter().filter(|t| existing_terms.contains(t)).collect();
                let union_size = query_terms.len() + existing_terms.len() - intersection.len();
                let similarity = if union_size > 0 { intersection.len() as f32 / union_size as f32 } else { 0.0 };
                (existing_query.clone(), similarity)
            })
            .filter(|(_, score)| *score > 0.2)
            .collect();
        scored_queries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_queries.into_iter().take(max_queries).map(|(query, _)| query).collect()
    }

    pub fn apply_feedback_boost(&self, query: &str, file_scores: &mut HashMap<String, f32>) {
        let query = query.to_lowercase();
        if let Some(feedback_map) = self.feedback.query_feedback.get(&query) {
            for (file_path, entry) in feedback_map {
                if let Some(score) = file_scores.get_mut(file_path) {
                    let confidence = (entry.relevant_count + entry.irrelevant_count) as f32 / 5.0;
                    let confidence_factor = confidence.min(1.0);
                    let feedback_factor = (entry.relevance_score - 0.5) * 2.0;
                    *score += feedback_factor * confidence_factor * 0.2;
                }
            }
        }
        let similar_queries = self.get_similar_queries(&query, 3);
        for similar_query in similar_queries {
            if let Some(feedback_map) = self.feedback.query_feedback.get(&similar_query) {
                for (file_path, entry) in feedback_map {
                    if let Some(score) = file_scores.get_mut(file_path) {
                        let confidence = (entry.relevant_count + entry.irrelevant_count) as f32 / 10.0;
                        let confidence_factor = confidence.min(1.0);
                        let feedback_factor = (entry.relevance_score - 0.5) * 2.0;
                        *score += feedback_factor * confidence_factor * 0.1;
                    }
                }
            }
        }
    }

    pub fn onnx_model_path(&self) -> Option<&PathBuf> {
        self.onnx_model_path.as_ref()
    }

    pub fn onnx_tokenizer_path(&self) -> Option<&PathBuf> {
        self.onnx_tokenizer_path.as_ref()
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

    fn index_directory_parallel( &mut self, files: Vec<PathBuf>, model: Arc<EmbeddingModel>, batch_size: usize ) -> Result<()> {
        let num_threads = num_cpus::get();
        let file_count = files.len();
        info!("Starting parallel indexing for {} files using {} threads.", file_count, num_threads);
        let progress = ProgressBar::new(file_count as u64);
        progress.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files indexed ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"));
        let cache_arc = Arc::new(Mutex::new(self.cache.clone()));
        let mut files_to_embed: Vec<(PathBuf, Option<u64>)> = Vec::with_capacity(file_count);
        let mut files_from_cache = 0;
        let mut cache_errors = 0;
        debug!("Checking cache for {} files...", file_count);
        {
            let mut cache_guard = cache_arc.lock().unwrap();
            for file_path in &files {
                let path_str = file_path.to_string_lossy().to_string();
                match cache_guard.check_cache_and_get_hash(&path_str, file_path) {
                    Ok(CacheCheckResult::Hit(embedding)) => {
                        drop(cache_guard);
                        self.embeddings.insert(path_str, embedding.clone());
                        if let Some(index) = &mut self.hnsw_index {
                            if let Err(e) = index.insert(embedding) {
                                error!("Failed to insert cached embedding into HNSW index for {}: {}", file_path.display(), e);
                            }
                        }
                        files_from_cache += 1;
                        progress.inc(1);
                        cache_guard = cache_arc.lock().unwrap();
                    }
                    Ok(CacheCheckResult::Miss(hash_opt)) => { files_to_embed.push((file_path.clone(), hash_opt)); }
                    Err(e) => {
                        error!("Cache check/hash error for {}: {}. Queuing for embedding.", file_path.display(), e);
                        cache_errors += 1;
                        files_to_embed.push((file_path.clone(), None));
                    }
                }
            }
        }
        if cache_errors > 0 { progress.println(format!("Warning: Encountered {} cache read/hash errors.", cache_errors)); }
        let files_to_embed_count = files_to_embed.len();
        debug!("Found {} files in cache. {} files need embedding.", files_from_cache, files_to_embed_count);
        if files_to_embed_count == 0 {
            progress.finish_with_message(format!("Processed {} files (all from cache).", files_from_cache));
            if let Err(e) = self.save() { error!("Failed to save database after cache check: {}", e); }
            return Ok(());
        }
        let model_clone = Arc::clone(&model);
        let effective_batch_size = std::cmp::min(batch_size, 128);
        let file_chunks: Vec<Vec<(PathBuf, Option<u64>)>> = files_to_embed.chunks(effective_batch_size).map(|chunk| chunk.to_vec()).collect();
        let chunk_count = file_chunks.len();
        info!("Processing {} files needing embedding in {} chunks (batch size ~{}).", files_to_embed_count, chunk_count, effective_batch_size);
        progress.println(format!("Scanning and preparing {} files for embedding...", files_to_embed_count));
        let pool = rayon::ThreadPoolBuilder::new().num_threads(num_threads).build().unwrap();
        let (tx, rx): ( Sender<Vec<(PathBuf, Option<u64>, Result<Vec<f32>>)>>, Receiver<_>, ) = mpsc::channel();
        let progress_clone = progress.clone();
        let processed = Arc::new(AtomicUsize::new(0));
        let processed_clone = processed.clone();
        let should_continue = Arc::new(AtomicBool::new(true));
        let should_continue_clone = should_continue.clone();
        std::thread::spawn(move || {
            let mut last_count = 0;
            let start = std::time::Instant::now();
            progress_clone.set_message("Starting file processing...");
            while should_continue_clone.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let current = processed_clone.load(Ordering::SeqCst);
                if current > last_count {
                    let elapsed = start.elapsed().as_secs_f32();
                    let rate = if elapsed > 0.1 { current as f32 / elapsed } else { 0.0 };
                    progress_clone.set_message(format!("Processing files ({:.1} files/sec)", rate));
                    progress_clone.set_position(current as u64);
                    last_count = current;
                } else if current > 0 { progress_clone.set_message("Processing (waiting for results)..."); }
                 else { progress_clone.set_message("Preparing for embedding..."); }
            }
        });
        pool.install(move || {
            file_chunks.into_par_iter().for_each_with(tx, |tx_clone, chunk_data| {
                    let mut texts_to_embed: Vec<String> = Vec::with_capacity(chunk_data.len());
                let mut paths_and_hashes: Vec<(PathBuf, Option<u64>)> = Vec::with_capacity(chunk_data.len());
                let mut results_for_chunk: Vec<(PathBuf, Option<u64>, Result<Vec<f32>>)> = Vec::with_capacity(chunk_data.len());
                    for (path, hash_opt) in chunk_data {
                        match std::fs::read_to_string(&path) {
                        Ok(content) => { texts_to_embed.push(content); paths_and_hashes.push((path, hash_opt)); }
                        Err(e) => { results_for_chunk.push(( path, hash_opt, Err(VectorDBError::IOError(e)), )); }
                    }
                }
                    if !texts_to_embed.is_empty() {
                    let text_refs: Vec<&str> = texts_to_embed.iter().map(String::as_str).collect();
                        match model_clone.embed_batch(&text_refs) {
                            Ok(embeddings) => {
                                assert_eq!(embeddings.len(), paths_and_hashes.len());
                            for ((path, hash_opt), embedding) in paths_and_hashes.into_iter().zip(embeddings) {
                                    results_for_chunk.push((path, hash_opt, Ok(embedding)));
                                }
                            }
                            Err(e) => {
                                let error_message = format!("Batch embedding failed: {}", e);
                                for (path, hash_opt) in paths_and_hashes {
                                results_for_chunk.push(( path, hash_opt, Err(VectorDBError::EmbeddingError(error_message.clone())), ));
                            }
                        }
                    }
                }
                    if !results_for_chunk.is_empty() {
                        processed.fetch_add(results_for_chunk.len(), Ordering::SeqCst);
                    if tx_clone.send(results_for_chunk).is_err() { warn!("Failed to send chunk results to main thread."); }
                    }
                });
        });
        should_continue.store(false, Ordering::SeqCst);
        let mut successful_embeddings = 0;
        let mut save_counter = 0;
        let start_time = std::time::Instant::now();
        let mut last_report_time = start_time;
        let mut processed_new_files = 0;
        while let Ok(chunk_results) = rx.recv() {
            let mut cache_guard = cache_arc.lock().unwrap();
            for (file_path, hash_opt, result) in chunk_results {
                processed_new_files += 1;
                match result {
                    Ok(embedding) => {
                        let file_path_str = file_path.to_string_lossy().to_string();
                        if let Some(hash) = hash_opt {
                            if let Err(e) = cache_guard.insert_with_hash( file_path_str.clone(), embedding.clone(), hash, ) {
                                error!("Failed to insert into cache for {}: {}", file_path.display(), e);
                            }
                        } else {
                            match EmbeddingCache::get_file_hash(&file_path) {
                                Ok(new_hash) => {
                                    if let Err(e) = cache_guard.insert_with_hash( file_path_str.clone(), embedding.clone(), new_hash, ) {
                                        error!("Failed to insert into cache (retry hash) for {}: {}", file_path.display(), e);
                                    }
                                }
                                Err(e) => { error!("Failed again to get hash for cache insertion for {}: {}", file_path.display(), e); }
                            }
                        }
                        self.embeddings.insert(file_path_str.clone(), embedding.clone());
                        drop(cache_guard);
                        if let Some(index) = &mut self.hnsw_index {
                            if let Err(e) = index.insert(embedding) {
                                error!("Failed to insert into HNSW index for {}: {}", file_path.display(), e);
                                progress.println(format!("Warning: Failed to add {} to HNSW index: {}", file_path_str, e));
                            }
                        }
                        cache_guard = cache_arc.lock().unwrap();
                        successful_embeddings += 1;
                        save_counter += 1;
                    }
                    Err(error) => { progress.println(format!("Error indexing {}: {}", file_path.display(), error)); }
                }
                progress.inc(1);
            }
            drop(cache_guard);
            if save_counter >= effective_batch_size {
                if let Err(e) = self.save() {
                    error!("Failed to save database during batch processing: {}", e);
                    progress.println(format!("Warning: Failed to save database: {}", e));
                } else { debug!("Saved database after processing {} new files", save_counter); }
                save_counter = 0;
            }
            let now = std::time::Instant::now();
            if now.duration_since(last_report_time).as_secs() >= 5 {
                last_report_time = now;
                let elapsed_secs = now.duration_since(start_time).as_secs_f32();
                let rate = if elapsed_secs > 0.1 { processed_new_files as f32 / elapsed_secs } else { 0.0 };
                progress.set_message(format!("Storing embeddings ({:.1} files/sec)", rate));
            }
        }
        if save_counter > 0 {
            if let Err(e) = self.save() {
                error!("Failed to save database at end of indexing: {}", e);
                progress.println(format!("Warning: Failed to save database: {}", e));
            } else { debug!("Final save completed."); }
            }
        let elapsed_total = start_time.elapsed().as_secs_f32();
        let rate_final = if elapsed_total > 0.1 { successful_embeddings as f32 / elapsed_total } else { 0.0 };
        let total_files_in_db = files_from_cache + successful_embeddings;
        if files_from_cache > 0 {
            progress.finish_with_message(format!(
                "Indexed {} files ({} new embeddings, {} from cache) in {:.1}s ({:.1} new/sec)",
                total_files_in_db, successful_embeddings, files_from_cache, elapsed_total, rate_final
            ));
        } else {
            progress.finish_with_message(format!(
                "Indexed {} files ({} new embeddings) in {:.1}s ({:.1} new/sec)",
                total_files_in_db, successful_embeddings, elapsed_total, rate_final
            ));
        }
        Ok(())
    }
}

pub struct DBStats {
    pub indexed_files: usize,
    pub embedding_dimension: usize,
    pub db_path: String,
    pub cached_files: usize,
    pub hnsw_stats: Option<HNSWStats>,
    pub embedding_model_type: EmbeddingModelType,
}

impl Clone for DBStats {
    fn clone(&self) -> Self {
        Self {
            indexed_files: self.indexed_files,
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
    
    use tempfile::tempdir;

    #[test]
    fn test_vectordb() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db").to_string_lossy().to_string();
        let _db = VectorDB::new(db_path)?;
        // Basic test - more tests needed for specific functionality
        Ok(())
    }

    #[test]
    fn test_optimal_layer_count() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_string_lossy().to_string();
        let _db = VectorDB::new(db_path).unwrap();
        let optimal = HNSWConfig::calculate_optimal_layers(1_000);
        assert!(optimal > 0);
        let optimal = HNSWConfig::calculate_optimal_layers(10_000);
        assert!(optimal > 0);
        let optimal = HNSWConfig::calculate_optimal_layers(100_000);
        assert!(optimal > 0);
    }
}