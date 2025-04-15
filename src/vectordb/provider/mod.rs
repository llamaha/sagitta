#![allow(unused_variables)]
#![allow(dead_code)]

use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::error::Result;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::PathBuf;

// === New EmbeddingProvider Trait ===
pub trait EmbeddingProvider: Send + Sync + Debug {
    /// Get the embedding dimension of the model.
    fn dimension(&self) -> usize;

    /// Get the type of the model.
    fn model_type(&self) -> EmbeddingModelType;

    /// Embed a batch of texts.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    // Optionally, a method to embed a single text?
    // fn embed(&self, text: &str) -> Result<Vec<f32>> {
    //     let embeddings = self.embed_batch(&[text])?;
    //     embeddings.into_iter().next().ok_or_else(|| {
    //         crate::vectordb::error::VectorDBError::EmbeddingError("Embedding failed for single text".to_string())
    //     })
    // }
}

// === Provider Modules (Keep existing structure) ===
pub mod onnx;

// === Config Structs (Keep existing structure) ===
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingModelConfig {
    pub model_type: EmbeddingModelType,
    pub onnx_model_path: Option<PathBuf>,
    pub onnx_tokenizer_path: Option<PathBuf>,
}

impl EmbeddingModelConfig {
    pub fn new(model_type: EmbeddingModelType) -> Self {
        Self {
            model_type,
            onnx_model_path: None,
            onnx_tokenizer_path: None,
        }
    }
} 