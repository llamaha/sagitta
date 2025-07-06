//! Unit tests for embedding pool with Default provider

#[cfg(test)]
mod tests {
    use crate::processor::{EmbeddingPool, ProcessedChunk, ChunkMetadata, ProcessingConfig, EmbeddingProcessor};
    use crate::config::EmbeddingConfig;
    use crate::model::EmbeddingModelType;
    use std::path::PathBuf;

    fn create_test_chunk(content: &str, id: &str) -> ProcessedChunk {
        ProcessedChunk {
            content: content.to_string(),
            metadata: ChunkMetadata {
                file_path: PathBuf::from("test.rs"),
                start_line: 1,
                end_line: 10,
                language: "rust".to_string(),
                file_extension: "rs".to_string(),
                element_type: "function".to_string(),
                context: None,
            },
            id: id.to_string(),
        }
    }

    #[tokio::test]
    async fn test_embedding_pool_with_default_provider() {
        // Create config with Default model type
        let embedding_config = EmbeddingConfig {
            model_type: EmbeddingModelType::Default,
            expected_dimension: Some(384),
            ..Default::default()
        };
        
        let processing_config = ProcessingConfig {
            max_embedding_sessions: 2,
            ..Default::default()
        };

        // Create pool
        let pool = EmbeddingPool::new(processing_config, embedding_config).unwrap();
        assert_eq!(pool.dimension(), 384);
        
        // Test processing chunks
        let chunks = vec![
            create_test_chunk("Hello, world!", "chunk_1"),
            create_test_chunk("Test content", "chunk_2"),
        ];
        
        let result = pool.process_chunks(chunks).await.unwrap();
        assert_eq!(result.len(), 2);
        
        // Verify embeddings have correct dimension
        for embedded_chunk in &result {
            assert_eq!(embedded_chunk.embedding.len(), 384);
            // Verify embeddings are normalized (Default provider normalizes)
            let norm: f32 = embedded_chunk.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.001, "Embedding should be normalized");
        }
    }

    #[tokio::test]
    async fn test_embedding_pool_parallel_processing() {
        let embedding_config = EmbeddingConfig {
            model_type: EmbeddingModelType::Default,
            expected_dimension: Some(384),
            ..Default::default()
        };
        
        let processing_config = ProcessingConfig {
            max_embedding_sessions: 4,
            embedding_batch_size: 5,
            ..Default::default()
        };

        let pool = EmbeddingPool::new(processing_config, embedding_config).unwrap();
        
        // Create many chunks to test parallel processing
        let chunks: Vec<ProcessedChunk> = (0..20)
            .map(|i| create_test_chunk(&format!("Test content {i}"), &format!("chunk_{i}")))
            .collect();
        
        let result = pool.process_chunks(chunks).await.unwrap();
        assert_eq!(result.len(), 20);
        
        // Verify all chunks were processed
        for (i, embedded_chunk) in result.iter().enumerate() {
            assert_eq!(embedded_chunk.chunk.id, format!("chunk_{i}"));
            assert_eq!(embedded_chunk.embedding.len(), 384);
        }
    }

    #[tokio::test]
    async fn test_embedding_pool_empty_input() {
        let embedding_config = EmbeddingConfig::default();
        let pool = EmbeddingPool::with_embedding_config(embedding_config).unwrap();
        
        let result = pool.process_chunks(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_embed_texts_async() {
        let embedding_config = EmbeddingConfig::default();
        let pool = EmbeddingPool::with_embedding_config(embedding_config).unwrap();
        
        let texts = vec!["Hello", "World", "Test"];
        let embeddings = pool.embed_texts_async(&texts).await.unwrap();
        
        assert_eq!(embeddings.len(), 3);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), 384);
            // Check normalization
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 0.01, "Norm {norm} should be close to 1.0");
        }
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let embedding_config = EmbeddingConfig::default();
        let processing_config = ProcessingConfig {
            max_embedding_sessions: 4,
            ..Default::default()
        };

        let pool = EmbeddingPool::new(processing_config, embedding_config).unwrap();
        let stats = pool.pool_stats().await;
        
        assert_eq!(stats.max_providers, 4);
        assert_eq!(stats.available_permits, 4);
        assert!(!stats.is_at_capacity());
        assert_eq!(stats.utilization(), 0.0);
    }

    #[tokio::test]
    async fn test_deterministic_embeddings() {
        let embedding_config = EmbeddingConfig::default();
        let pool = EmbeddingPool::with_embedding_config(embedding_config).unwrap();
        
        let texts = vec!["test text"];
        
        // Generate embeddings twice
        let embeddings1 = pool.embed_texts_async(&texts).await.unwrap();
        let embeddings2 = pool.embed_texts_async(&texts).await.unwrap();
        
        // Should produce same embedding for same text
        assert_eq!(embeddings1[0], embeddings2[0]);
    }

    #[tokio::test]
    async fn test_different_texts_different_embeddings() {
        let embedding_config = EmbeddingConfig::default();
        let pool = EmbeddingPool::with_embedding_config(embedding_config).unwrap();
        
        let texts = vec!["hello", "world", "hello"];
        let embeddings = pool.embed_texts_async(&texts).await.unwrap();
        
        // Same text should produce same embedding
        assert_eq!(embeddings[0], embeddings[2]);
        
        // Different texts should produce different embeddings
        assert_ne!(embeddings[0], embeddings[1]);
    }
}