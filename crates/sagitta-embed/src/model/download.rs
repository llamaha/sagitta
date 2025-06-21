use anyhow::{Context, Result};
use hf_hub::{api::sync::Api, Repo, RepoType};
use std::path::{Path, PathBuf};
use log::{info, debug};

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

        // Create HuggingFace API
        let api = Api::new()?;
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        // Download model file
        let model_file = model.model_file();
        debug!("Downloading model file: {}", model_file);
        let model_path = repo
            .get(model_file)
            .with_context(|| format!("Failed to download model file: {}", model_file))?;

        // Download tokenizer file
        let tokenizer_file = model.tokenizer_file();
        debug!("Downloading tokenizer file: {}", tokenizer_file);
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

        // For GPU models converted from non-ONNX format, we might need to convert
        if model.is_gpu_model() && !model_path.exists() {
            return Err(anyhow::anyhow!(
                "GPU model requires conversion. Please use the conversion script in scripts/convert_bge_small_gpu_fp16.py"
            ));
        }

        info!("Model downloaded successfully to: {:?}", model_path);

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
}