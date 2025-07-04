use anyhow::{Context, Result};
use hf_hub::{api::sync::Api, Repo, RepoType};
use std::path::{Path, PathBuf};
use std::fs;
use log::{info, debug, warn};

/// Predefined embedding models with their HuggingFace model IDs and file paths
#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddingModel {
    /// BGE Small v1.5 with INT8 quantization (fast)
    BgeSmallEnV15Quantized,
    /// BGE Small v1.5 FP32 (standard precision)
    BgeSmallEnV15Fp32,
    /// Custom model specified by HuggingFace model ID
    Custom(String),
}

impl EmbeddingModel {
    /// Get the HuggingFace model ID for the embedding model
    pub fn model_id(&self) -> &str {
        match self {
            Self::BgeSmallEnV15Quantized => "Qdrant/bge-small-en-v1.5-onnx-Q",
            Self::BgeSmallEnV15Fp32 => "BAAI/bge-small-en-v1.5",
            Self::Custom(id) => id,
        }
    }

    /// Get the ONNX model file name within the repository
    pub fn model_file(&self) -> &str {
        match self {
            Self::BgeSmallEnV15Quantized => "model_optimized.onnx",
            Self::BgeSmallEnV15Fp32 => "onnx/model.onnx",
            Self::Custom(_) => "model.onnx", // Default for custom models
        }
    }

    /// Get the tokenizer file name within the repository
    pub fn tokenizer_file(&self) -> &str {
        match self {
            Self::BgeSmallEnV15Quantized => "tokenizer.json",
            Self::BgeSmallEnV15Fp32 => "tokenizer.json",
            Self::Custom(_) => "tokenizer.json",
        }
    }

    /// Additional files that need to be downloaded
    pub fn additional_files(&self) -> Vec<&str> {
        match self {
            Self::BgeSmallEnV15Quantized => vec!["tokenizer_config.json", "config.json", "special_tokens_map.json"],
            Self::BgeSmallEnV15Fp32 => vec!["tokenizer_config.json", "config.json", "special_tokens_map.json"],
            Self::Custom(_) => vec!["tokenizer_config.json", "config.json", "special_tokens_map.json"],
        }
    }

    /// Returns true if this is a GPU-optimized model
    pub fn is_gpu_model(&self) -> bool {
        matches!(self, Self::BgeSmallEnV15Fp32)
    }

    /// Parse from string representation
    pub fn from_str(s: &str) -> Self {
        match s {
            "bge-small-en-v1.5-q" | "bge-small-fast" => Self::BgeSmallEnV15Quantized,
            "bge-small-en-v1.5-fp16" | "bge-small-fp32" => Self::BgeSmallEnV15Fp32,
            // Don't try to download test-default model
            "test-default" => Self::Custom("test-default".to_string()),
            custom => Self::Custom(custom.to_string()),
        }
    }
}

/// Model downloader that handles automatic downloading from HuggingFace
pub struct ModelDownloader {
    cache_dir: PathBuf,
    show_progress: bool,
}

impl ModelDownloader {
    /// Create a new model downloader with default cache directory
    pub fn new() -> Result<Self> {
        let cache_dir = Self::default_cache_dir()?;
        Ok(Self {
            cache_dir,
            show_progress: true,
        })
    }
    
    /// Download a file directly from HuggingFace using a simple HTTP client
    /// This is a fallback when hf_hub fails
    fn download_file_direct(&self, model_id: &str, filename: &str, cache_path: &Path) -> Result<PathBuf> {
        use ureq;
        
        // Construct the direct download URL
        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            model_id, filename
        );
        
        info!("Attempting direct download from: {}", url);
        
        // Create cache directory if it doesn't exist
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create cache directory: {:?}", parent))?;
        }
        
        // Download the file
        let response = ureq::get(&url)
            .call()
            .map_err(|e| anyhow::anyhow!("Failed to download {}: {}", filename, e))?;
        
        // Write to cache file
        let mut file = fs::File::create(cache_path)
            .with_context(|| format!("Failed to create file: {:?}", cache_path))?;
            
        let mut reader = response.into_reader();
        std::io::copy(&mut reader, &mut file)
            .with_context(|| format!("Failed to write file: {:?}", cache_path))?;
            
        info!("Successfully downloaded {} to {:?}", filename, cache_path);
        Ok(cache_path.to_path_buf())
    }

    /// Create a new model downloader with custom cache directory
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            show_progress: true,
        }
    }

    /// Set whether to show download progress
    pub fn show_progress(mut self, show: bool) -> Self {
        self.show_progress = show;
        self
    }

    /// Get the default cache directory (~/.cache/huggingface/hub/)
    fn default_cache_dir() -> Result<PathBuf> {
        Ok(dirs::cache_dir()
            .context("Could not find cache directory")?
            .join("huggingface")
            .join("hub"))
    }

    /// Download a model and return paths to the model and tokenizer
    pub fn download_model(&self, model: &EmbeddingModel) -> Result<ModelPaths> {
        let model_id = model.model_id();
        info!("Downloading model: {}", model_id);

        // Log cache directory
        debug!("Using cache directory: {:?}", self.cache_dir);
        
        // Create model-specific cache directory
        let model_cache_dir = self.cache_dir
            .join("models--")
            .join(model_id.replace('/', "--"));
        
        // Determine file paths in cache
        let model_file = model.model_file();
        let tokenizer_file = model.tokenizer_file();
        
        let cached_model_path = model_cache_dir.join("snapshots").join("main").join(model_file);
        let cached_tokenizer_path = model_cache_dir.join("snapshots").join("main").join(tokenizer_file);
        
        // Check if files already exist in cache
        if cached_model_path.exists() && cached_tokenizer_path.exists() {
            info!("Model files found in cache, using cached version");
            return Ok(ModelPaths {
                model_path: cached_model_path,
                tokenizer_path: cached_tokenizer_path,
                additional_files: vec![],
            });
        }
        
        // Try to download using hf_hub first, but catch common errors and use fallback
        let (model_path, tokenizer_path) = match self.try_hf_hub_download(model) {
            Ok(paths) => {
                info!("Successfully downloaded using hf_hub");
                (paths.model_path, paths.tokenizer_path)
            }
            Err(e) => {
                // Log the error for debugging
                warn!("hf_hub download failed: {}", e);
                warn!("Error details: {:?}", e);
                
                // Use direct download as fallback for any hf_hub errors
                // The hf_hub library has various issues that can cause downloads to fail
                info!("Using direct download fallback due to hf_hub error");
                
                // Ensure cache directories exist
                if let Some(parent) = cached_model_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                
                // Use direct download as fallback
                let model_path = self.download_file_direct(model_id, model_file, &cached_model_path)?;
                let tokenizer_path = self.download_file_direct(model_id, tokenizer_file, &cached_tokenizer_path)?;
                
                // Download additional files
                for file in model.additional_files() {
                    let cache_path = model_cache_dir.join("snapshots").join("main").join(file);
                    match self.download_file_direct(model_id, file, &cache_path) {
                        Ok(_) => debug!("Downloaded additional file: {}", file),
                        Err(e) => debug!("Failed to download optional file {}: {}", file, e),
                    }
                }
                
                (model_path, tokenizer_path)
            }
        };

        Ok(ModelPaths {
            model_path,
            tokenizer_path,
            additional_files: vec![],
        })
    }
    
    /// Try to download using hf_hub (may fail with URL parsing errors)
    fn try_hf_hub_download(&self, model: &EmbeddingModel) -> Result<ModelPaths> {
        let model_id = model.model_id();
        
        // Create HuggingFace API
        let api = Api::new()
            .with_context(|| "Failed to create HuggingFace API client")?;
        
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        // Download model file
        let model_file = model.model_file();
        debug!("Downloading model file via hf_hub: {}", model_file);
        let model_path = repo
            .get(model_file)
            .with_context(|| format!("Failed to download model file: {}", model_file))?;

        // Download tokenizer file
        let tokenizer_file = model.tokenizer_file();
        debug!("Downloading tokenizer file via hf_hub: {}", tokenizer_file);
        let tokenizer_path = repo
            .get(tokenizer_file)
            .with_context(|| format!("Failed to download tokenizer file: {}", tokenizer_file))?;

        // Download additional files
        let mut additional_paths = Vec::new();
        for file in model.additional_files() {
            debug!("Downloading additional file: {}", file);
            match repo.get(file) {
                Ok(path) => additional_paths.push(path),
                Err(e) => {
                    debug!("Optional file {} not found: {}", file, e);
                    // Continue - some files might be optional
                }
            }
        }

        Ok(ModelPaths {
            model_path,
            tokenizer_path,
            additional_files: additional_paths,
        })
    }

    /// Check if a model is already cached
    pub fn is_cached(&self, model: &EmbeddingModel) -> bool {
        match self.download_model(model) {
            Ok(paths) => paths.model_path.exists() && paths.tokenizer_path.exists(),
            Err(_) => false,
        }
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

/// Paths to downloaded model files
#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub additional_files: Vec<PathBuf>,
}

impl ModelPaths {
    /// Get the directory containing the tokenizer files
    pub fn tokenizer_dir(&self) -> Result<PathBuf> {
        self.tokenizer_path
            .parent()
            .map(|p| p.to_path_buf())
            .context("Could not get tokenizer directory")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_model_from_str() {
        assert_eq!(
            EmbeddingModel::from_str("bge-small-fast"),
            EmbeddingModel::BgeSmallEnV15Quantized
        );
        assert_eq!(
            EmbeddingModel::from_str("bge-small-fp32"),
            EmbeddingModel::BgeSmallEnV15Fp32
        );
        assert_eq!(
            EmbeddingModel::from_str("custom/model-id"),
            EmbeddingModel::Custom("custom/model-id".to_string())
        );
    }

    #[test]
    fn test_model_properties() {
        let cpu_model = EmbeddingModel::BgeSmallEnV15Quantized;
        assert_eq!(cpu_model.model_id(), "Qdrant/bge-small-en-v1.5-onnx-Q");
        assert_eq!(cpu_model.model_file(), "model_optimized.onnx");
        assert!(!cpu_model.is_gpu_model());

        let gpu_model = EmbeddingModel::BgeSmallEnV15Fp32;
        assert_eq!(gpu_model.model_id(), "BAAI/bge-small-en-v1.5");
        assert_eq!(gpu_model.model_file(), "onnx/model.onnx");
        assert!(gpu_model.is_gpu_model());
    }
    
    #[test]
    #[ignore = "Requires internet connection and may be slow"]
    fn test_download_bge_small_fast_model() {
        // Initialize logger for test
        let _ = env_logger::builder()
            .is_test(true)
            .try_init();
            
        // This test verifies that the model download works correctly
        let model = EmbeddingModel::BgeSmallEnV15Quantized;
        let downloader = ModelDownloader::new().expect("Failed to create downloader");
        
        println!("Testing download for model: {:?}", model);
        println!("Model ID: {}", model.model_id());
        println!("Model file: {}", model.model_file());
        println!("Tokenizer file: {}", model.tokenizer_file());
        
        // Try to download the model
        match downloader.download_model(&model) {
            Ok(paths) => {
                // Verify that all expected files exist
                assert!(paths.model_path.exists(), "Model file should exist");
                assert!(paths.tokenizer_path.exists(), "Tokenizer file should exist");
                
                // Check file names
                assert!(paths.model_path.to_string_lossy().contains("model_optimized.onnx"));
                assert!(paths.tokenizer_path.to_string_lossy().contains("tokenizer.json"));
                
                println!("Model downloaded successfully!");
                println!("Model path: {:?}", paths.model_path);
                println!("Tokenizer path: {:?}", paths.tokenizer_path);
            }
            Err(e) => {
                // If download fails, provide helpful information
                eprintln!("Model download failed: {}", e);
                eprintln!("Full error: {:?}", e);
                eprintln!("Error string: {}", e.to_string());
                eprintln!("\nThis might be due to:");
                eprintln!("1. Network connectivity issues");
                eprintln!("2. SSL/TLS certificate problems (try: export HF_HUB_DISABLE_SSL_VERIFY=1)");
                eprintln!("3. HuggingFace API rate limits");
                eprintln!("4. Corporate proxy/firewall blocking the download");
                eprintln!("\nFor debugging, you can:");
                eprintln!("- Set RUST_LOG=debug for more verbose output");
                eprintln!("- Try downloading manually from https://huggingface.co/{}", model.model_id());
                
                // Don't fail the test hard to allow CI to pass
                // but indicate the issue
                panic!("Download test failed - see error message above for troubleshooting");
            }
        }
    }
    
    #[test] 
    fn test_cache_directory_creation() {
        let downloader = ModelDownloader::new();
        assert!(downloader.is_ok(), "Should be able to create downloader");
        
        let downloader = downloader.unwrap();
        assert!(downloader.cache_dir.exists() || downloader.cache_dir.parent().unwrap().exists(),
                "Cache directory or its parent should exist");
    }
    
    #[test]
    #[ignore = "Requires internet connection"]
    fn test_direct_download_fallback() {
        // Initialize logger
        let _ = env_logger::builder()
            .is_test(true)
            .try_init();
            
        let downloader = ModelDownloader::new().expect("Failed to create downloader");
        let model_id = "Qdrant/bge-small-en-v1.5-onnx-Q";
        let filename = "tokenizer.json";
        
        // Create a temporary path for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let cache_path = temp_dir.path().join(filename);
        
        println!("Testing direct download of {} from {}", filename, model_id);
        
        match downloader.download_file_direct(model_id, filename, &cache_path) {
            Ok(path) => {
                println!("Direct download succeeded: {:?}", path);
                assert!(path.exists(), "Downloaded file should exist");
                
                // Check file size to ensure it's not empty
                let metadata = std::fs::metadata(&path).unwrap();
                assert!(metadata.len() > 0, "Downloaded file should not be empty");
                println!("File size: {} bytes", metadata.len());
            }
            Err(e) => {
                eprintln!("Direct download failed: {}", e);
                panic!("Direct download should work");
            }
        }
    }
}