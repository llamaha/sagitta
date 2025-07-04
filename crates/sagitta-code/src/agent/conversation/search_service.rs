use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use sagitta_search::{EmbeddingPool, embed_single_text_with_pool};
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use sagitta_embed::EmbeddingProcessor;

use super::types::{Conversation, ConversationSummary};

/// Trait for conversation search operations
#[async_trait]
pub trait ConversationSearchService: Send + Sync {
    /// Index a conversation for search
    async fn index_conversation(&self, conversation: &Conversation) -> Result<()>;
    
    /// Remove a conversation from the search index
    async fn remove_conversation(&self, conversation_id: Uuid) -> Result<()>;
    
    /// Search conversations by semantic similarity
    async fn search_conversations(&self, query: &str, limit: usize) -> Result<Vec<ConversationSearchResult>>;
    
    /// Find similar conversations to a given conversation
    async fn find_similar_conversations(&self, conversation_id: Uuid, limit: usize) -> Result<Vec<ConversationSearchResult>>;
    
    /// Get conversation embeddings for clustering
    async fn get_conversation_embeddings(&self, conversation_ids: &[Uuid]) -> Result<Vec<ConversationEmbedding>>;
    
    /// Batch index multiple conversations
    async fn batch_index_conversations(&self, conversations: &[Conversation]) -> Result<()>;
}

/// Search result for conversations
#[derive(Debug, Clone)]
pub struct ConversationSearchResult {
    /// Conversation summary
    pub conversation: ConversationSummary,
    
    /// Similarity score (0.0 to 1.0)
    pub score: f32,
    
    /// Matching snippet if available
    pub snippet: Option<String>,
    
    /// Reason for the match
    pub match_reason: MatchReason,
}

/// Reason why a conversation matched the search
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchReason {
    /// Title similarity
    TitleMatch,
    
    /// Content similarity
    ContentMatch,
    
    /// Tag match
    TagMatch,
    
    /// Project context match
    ProjectMatch,
    
    /// Combined similarity
    SemanticMatch,
}

/// Conversation embedding for clustering
#[derive(Debug, Clone)]
pub struct ConversationEmbedding {
    /// Conversation ID
    pub conversation_id: Uuid,
    
    /// Embedding vector
    pub embedding: Vec<f32>,
    
    /// Metadata for the embedding
    pub metadata: EmbeddingMetadata,
}

/// Metadata associated with conversation embeddings
#[derive(Debug, Clone)]
pub struct EmbeddingMetadata {
    /// Title of the conversation
    pub title: String,
    
    /// Tags associated with the conversation
    pub tags: Vec<String>,
    
    /// Project type if available
    pub project_type: Option<String>,
    
    /// Last activity timestamp
    pub last_active: chrono::DateTime<chrono::Utc>,
    
    /// Message count
    pub message_count: usize,
}

/// Implementation of conversation search service using Qdrant and embeddings
pub struct ConversationSearchServiceImpl {
    /// Qdrant client for vector operations
    qdrant_client: Arc<dyn QdrantClientTrait>,
    
    /// Embedding pool for generating embeddings
    embedding_pool: Arc<EmbeddingPool>,
    
    /// Collection name for conversation embeddings
    collection_name: String,
    
    /// Embedding dimension
    embedding_dimension: usize,
}

impl ConversationSearchServiceImpl {
    /// Create a new conversation search service
    pub async fn new(
        qdrant_client: Arc<dyn QdrantClientTrait>,
        embedding_pool: Arc<EmbeddingPool>,
        collection_name: String,
    ) -> Result<Self> {
        let embedding_dimension = embedding_pool.dimension();
        
        let service = Self {
            qdrant_client,
            embedding_pool,
            collection_name,
            embedding_dimension,
        };
        
        // Ensure collection exists
        service.ensure_collection_exists().await?;
        
        Ok(service)
    }
    
    /// Ensure the conversation collection exists in Qdrant
    async fn ensure_collection_exists(&self) -> Result<()> {
        // Check if collection exists
        if self.qdrant_client.collection_exists(self.collection_name.clone()).await
            .map_err(|e| anyhow::anyhow!("Failed to check collection existence: {}", e))? {
            return Ok(());
        }
        
        // Create collection using the simplified interface
        self.qdrant_client.create_collection(&self.collection_name, self.embedding_dimension as u64).await
            .map_err(|e| anyhow::anyhow!("Failed to create collection: {}", e))?;
        
        Ok(())
    }
    
    /// Generate embedding for conversation content
    async fn generate_conversation_embedding(&self, conversation: &Conversation) -> Result<Vec<f32>> {
        // Combine title, tags, and first few messages for embedding
        let mut content_parts = vec![conversation.title.clone()];
        
        // Add tags
        content_parts.extend(conversation.tags.clone());
        
        // Add project context
        if let Some(ref project_context) = conversation.project_context {
            content_parts.push(format!("Project: {}", project_context.name));
            content_parts.push(format!("Type: {:?}", project_context.project_type));
        }
        
        // Add first few messages (up to 3)
        for message in conversation.messages.iter().take(3) {
            content_parts.push(message.content.clone());
        }
        
        let combined_content = content_parts.join(" ");
        embed_single_text_with_pool(&self.embedding_pool, &combined_content).await
            .map_err(|e| anyhow::anyhow!("Failed to generate embedding: {}", e))
    }
    
    /// Create metadata for a conversation
    fn create_conversation_metadata(&self, conversation: &Conversation) -> std::collections::HashMap<String, qdrant_client::qdrant::Value> {
        let mut metadata = std::collections::HashMap::new();
        
        metadata.insert("title".to_string(), conversation.title.clone().into());
        metadata.insert("created_at".to_string(), conversation.created_at.to_rfc3339().into());
        metadata.insert("last_active".to_string(), conversation.last_active.to_rfc3339().into());
        metadata.insert("message_count".to_string(), (conversation.messages.len() as i64).into());
        metadata.insert("status".to_string(), format!("{:?}", conversation.status).into());
        
        // Add workspace ID if present
        if let Some(workspace_id) = conversation.workspace_id {
            metadata.insert("workspace_id".to_string(), workspace_id.to_string().into());
        }
        
        // Add project context
        if let Some(ref project_context) = conversation.project_context {
            metadata.insert("project_name".to_string(), project_context.name.clone().into());
            metadata.insert("project_type".to_string(), format!("{:?}", project_context.project_type).into());
        }
        
        // Add tags
        for (i, tag) in conversation.tags.iter().enumerate() {
            metadata.insert(format!("tag_{}", i), tag.clone().into());
        }
        
        // Add branch and checkpoint counts
        metadata.insert("branch_count".to_string(), (conversation.branches.len() as i64).into());
        metadata.insert("checkpoint_count".to_string(), (conversation.checkpoints.len() as i64).into());
        
        metadata
    }
    
    /// Extract point ID as string
    fn extract_point_id(point_id: &qdrant_client::qdrant::PointId) -> Result<String> {
        use qdrant_client::qdrant::point_id::PointIdOptions;
        
        match &point_id.point_id_options {
            Some(PointIdOptions::Uuid(uuid_str)) => Ok(uuid_str.clone()),
            Some(PointIdOptions::Num(num)) => Ok(num.to_string()),
            None => Err(anyhow::anyhow!("Point ID has no options")),
        }
    }
}

#[async_trait]
impl ConversationSearchService for ConversationSearchServiceImpl {
    async fn index_conversation(&self, conversation: &Conversation) -> Result<()> {
        use qdrant_client::qdrant::{PointStruct, UpsertPoints};
        
        // Generate embedding
        let embedding = self.generate_conversation_embedding(conversation).await?;
        
        // Create metadata
        let metadata = self.create_conversation_metadata(conversation);
        
        // Create point
        let point = PointStruct::new(
            qdrant_client::qdrant::PointId::from(conversation.id.to_string()),
            embedding,
            qdrant_client::Payload::from(metadata),
        );
        
        // Upsert point
        let upsert_points = UpsertPoints {
            collection_name: self.collection_name.clone(),
            points: vec![point],
            ..Default::default()
        };
        
        self.qdrant_client.upsert_points(upsert_points).await
            .map_err(|e| anyhow::anyhow!("Failed to upsert points: {}", e))?;
        
        Ok(())
    }
    
    async fn remove_conversation(&self, conversation_id: Uuid) -> Result<()> {
        use qdrant_client::qdrant::{DeletePoints, PointsSelector};
        
        let delete_points = DeletePoints {
            collection_name: self.collection_name.clone(),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Points(
                        qdrant_client::qdrant::PointsIdsList {
                            ids: vec![qdrant_client::qdrant::PointId::from(conversation_id.to_string())],
                        }
                    )
                ),
            }),
            ..Default::default()
        };
        
        self.qdrant_client.delete_points(delete_points).await
            .map_err(|e| anyhow::anyhow!("Failed to delete points: {}", e))?;
        
        Ok(())
    }
    
    async fn search_conversations(&self, query: &str, limit: usize) -> Result<Vec<ConversationSearchResult>> {
        use qdrant_client::qdrant::SearchPoints;
        
        // Generate query embedding
        let query_embedding = embed_single_text_with_pool(&self.embedding_pool, query).await
            .map_err(|e| anyhow::anyhow!("Failed to generate query embedding: {}", e))?;
        
        // Search
        let search_points = SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: query_embedding,
            limit: limit as u64,
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            score_threshold: Some(0.5), // Minimum similarity threshold
            offset: None,
            filter: None,
            params: None,
            vector_name: None,
            read_consistency: None,
            timeout: None,
            shard_key_selector: None,
            sparse_indices: None,
        };
        
        let search_response = self.qdrant_client.search_points(search_points).await
            .map_err(|e| anyhow::anyhow!("Failed to search points: {}", e))?;
        
        // Convert results
        let mut results = Vec::new();
        for scored_point in search_response.result {
            let payload = scored_point.payload;
            
            // Extract conversation data from payload
            let title = payload.get("title")
                .and_then(|v| v.as_str())
                .map_or("Unknown".to_string(), |s| s.to_string());
            
            let conversation_id = if let Some(point_id) = scored_point.id {
                let id_str = Self::extract_point_id(&point_id)?;
                Uuid::parse_str(&id_str)
                    .map_err(|e| anyhow::anyhow!("Invalid conversation ID: {}", e))?
            } else {
                continue;
            };
            
            // Create a minimal conversation summary
            let conversation = ConversationSummary {
                id: conversation_id,
                title,
                created_at: chrono::Utc::now(), // Would need to parse from payload
                last_active: chrono::Utc::now(), // Would need to parse from payload
                message_count: payload.get("message_count")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(0) as usize,
                status: crate::agent::state::types::ConversationStatus::Active, // Would need to parse
                tags: Vec::new(), // Would need to extract from payload
                workspace_id: None, // Would need to parse from payload
                project_name: payload.get("project_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                has_branches: payload.get("branch_count")
                    .and_then(|v| v.as_integer())
                    .map(|c| c > 0)
                    .unwrap_or(false),
                has_checkpoints: payload.get("checkpoint_count")
                    .and_then(|v| v.as_integer())
                    .map(|c| c > 0)
                    .unwrap_or(false),
            };
            
            results.push(ConversationSearchResult {
                conversation,
                score: scored_point.score,
                snippet: None, // Could extract from content
                match_reason: MatchReason::SemanticMatch,
            });
        }
        
        Ok(results)
    }
    
    async fn find_similar_conversations(&self, conversation_id: Uuid, limit: usize) -> Result<Vec<ConversationSearchResult>> {
        // For now, return empty results as this requires more complex implementation
        Ok(Vec::new())
    }
    
    async fn get_conversation_embeddings(&self, conversation_ids: &[Uuid]) -> Result<Vec<ConversationEmbedding>> {
        // For now, return empty results as this requires more complex implementation
        Ok(Vec::new())
    }
    
    async fn batch_index_conversations(&self, conversations: &[Conversation]) -> Result<()> {
        use qdrant_client::qdrant::{PointStruct, UpsertPoints};
        
        let mut points = Vec::new();
        
        for conversation in conversations {
            // Generate embedding
            let embedding = self.generate_conversation_embedding(conversation).await?;
            
            // Create metadata
            let metadata = self.create_conversation_metadata(conversation);
            
            // Create point
            let point = PointStruct::new(
                qdrant_client::qdrant::PointId::from(conversation.id.to_string()),
                embedding,
                qdrant_client::Payload::from(metadata),
            );
            
            points.push(point);
        }
        
        // Batch upsert
        let upsert_points = UpsertPoints {
            collection_name: self.collection_name.clone(),
            points,
            ..Default::default()
        };
        
        self.qdrant_client.upsert_points(upsert_points).await
            .map_err(|e| anyhow::anyhow!("Failed to batch upsert points: {}", e))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::ProjectType;
    use sagitta_embed::EmbeddingConfig;
    use std::sync::Arc;
    
    // Create a simple mock for testing
    struct MockQdrantClient;
    
    #[async_trait]
    impl QdrantClientTrait for MockQdrantClient {
        async fn health_check(&self) -> sagitta_search::Result<qdrant_client::qdrant::HealthCheckReply> {
            Ok(qdrant_client::qdrant::HealthCheckReply::default())
        }
        
        async fn delete_collection(&self, _collection_name: String) -> sagitta_search::Result<bool> {
            Ok(true)
        }
        
        async fn search_points(&self, _request: qdrant_client::qdrant::SearchPoints) -> sagitta_search::Result<qdrant_client::qdrant::SearchResponse> {
            Ok(qdrant_client::qdrant::SearchResponse::default())
        }
        
        async fn get_collection_info(&self, _collection_name: String) -> sagitta_search::Result<qdrant_client::qdrant::CollectionInfo> {
            Ok(qdrant_client::qdrant::CollectionInfo::default())
        }
        
        async fn count(&self, _request: qdrant_client::qdrant::CountPoints) -> sagitta_search::Result<qdrant_client::qdrant::CountResponse> {
            Ok(qdrant_client::qdrant::CountResponse::default())
        }
        
        async fn collection_exists(&self, _collection_name: String) -> sagitta_search::Result<bool> {
            Ok(true)
        }
        
        async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &qdrant_client::qdrant::PointsSelector) -> sagitta_search::Result<()> {
            Ok(())
        }
        
        async fn scroll(&self, _request: qdrant_client::qdrant::ScrollPoints) -> sagitta_search::Result<qdrant_client::qdrant::ScrollResponse> {
            Ok(qdrant_client::qdrant::ScrollResponse::default())
        }
        
        async fn upsert_points(&self, _request: qdrant_client::qdrant::UpsertPoints) -> sagitta_search::Result<qdrant_client::qdrant::PointsOperationResponse> {
            Ok(qdrant_client::qdrant::PointsOperationResponse::default())
        }
        
        async fn create_collection(&self, _collection_name: &str, _vector_dimension: u64) -> sagitta_search::Result<bool> {
            Ok(true)
        }
        
        async fn create_collection_detailed(&self, _request: qdrant_client::qdrant::CreateCollection) -> sagitta_search::Result<bool> {
            Ok(true)
        }
        
        async fn delete_points(&self, _request: qdrant_client::qdrant::DeletePoints) -> sagitta_search::Result<qdrant_client::qdrant::PointsOperationResponse> {
            Ok(qdrant_client::qdrant::PointsOperationResponse::default())
        }
        
        async fn query_points(&self, _request: qdrant_client::qdrant::QueryPoints) -> sagitta_search::Result<qdrant_client::qdrant::QueryResponse> {
            Ok(qdrant_client::qdrant::QueryResponse::default())
        }
        
        async fn query(&self, _request: qdrant_client::qdrant::QueryPoints) -> sagitta_search::Result<qdrant_client::qdrant::QueryResponse> {
            Ok(qdrant_client::qdrant::QueryResponse::default())
        }
        
        async fn list_collections(&self) -> sagitta_search::Result<Vec<String>> {
            Ok(vec!["test_conversations".to_string()])
        }
    }
    
    async fn create_test_search_service() -> ConversationSearchServiceImpl {
        let mock_qdrant = Arc::new(MockQdrantClient);
        
        // Create a minimal embedding pool for testing
        let embedding_config = EmbeddingConfig::default();
        let embedding_pool = Arc::new(EmbeddingPool::with_configured_sessions(embedding_config).unwrap());
        
        ConversationSearchServiceImpl::new(
            mock_qdrant,
            embedding_pool,
            "test_conversations".to_string(),
        ).await.unwrap()
    }
    
    #[tokio::test]
    async fn test_search_service_creation() {
        let _service = create_test_search_service().await;
        // If we get here without panicking, the service was created successfully
    }
    
    #[tokio::test]
    #[ignore = "Requires ONNX model configuration"]
    async fn test_embedding_generation() {
        let service = create_test_search_service().await;
        
        let conversation = Conversation::new("Test Conversation".to_string(), None);
        let embedding = service.generate_conversation_embedding(&conversation).await.unwrap();
        
        assert_eq!(embedding.len(), service.embedding_dimension);
        assert!(embedding.iter().any(|&x| x != 0.0)); // Should have non-zero values
    }
    
    #[tokio::test]
    async fn test_metadata_creation() {
        let service = create_test_search_service().await;
        
        let mut conversation = Conversation::new("Test Conversation".to_string(), None);
        conversation.tags.push("test".to_string());
        conversation.tags.push("example".to_string());
        
        let metadata = service.create_conversation_metadata(&conversation);
        
        assert_eq!(metadata.get("title").unwrap().as_str().unwrap(), "Test Conversation");
        assert_eq!(metadata.get("message_count").unwrap().as_integer().unwrap(), 0);
        assert_eq!(metadata.get("tag_0").unwrap().as_str().unwrap(), "test");
        assert_eq!(metadata.get("tag_1").unwrap().as_str().unwrap(), "example");
    }
} 