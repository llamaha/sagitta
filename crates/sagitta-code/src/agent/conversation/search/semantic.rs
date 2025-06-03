// Semantic conversation search implementation using Qdrant directly
// TODO: Implement actual semantic search

use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use qdrant_client::{Qdrant, qdrant::{PointStruct, Filter, Condition, ScoredPoint, SearchPoints, PointId, Value as QdrantValue, point_id::PointIdOptions}};
use sagitta_search::{EmbeddingHandler, config::AppConfig};

use super::ConversationSearchEngine;
use crate::agent::conversation::types::{
    Conversation, ConversationQuery, ConversationSearchResult, ConversationSummary
};

/// Semantic conversation search engine using Qdrant directly
pub struct SemanticConversationSearchEngine {
    /// Qdrant client for vector operations
    qdrant_client: Arc<Qdrant>,
    
    /// Embedding handler for generating vectors
    embedding_handler: EmbeddingHandler,
    
    /// Conversation summaries for quick access
    conversations: Arc<RwLock<HashMap<Uuid, ConversationSummary>>>,
    
    /// Collection name for conversations
    collection_name: String,
}

impl SemanticConversationSearchEngine {
    /// Create a new semantic search engine
    pub async fn new(
        qdrant_client: Arc<Qdrant>,
        embedding_handler: EmbeddingHandler,
        collection_name: String,
    ) -> Result<Self> {
        // Ensure collection exists
        if !qdrant_client.collection_exists(&collection_name).await? {
            use qdrant_client::qdrant::{CreateCollection, VectorParams, VectorsConfig, Distance};
            
            let create_collection = CreateCollection {
                collection_name: collection_name.clone(),
                vectors_config: Some(VectorsConfig {
                    config: Some(qdrant_client::qdrant::vectors_config::Config::ParamsMap(
                        qdrant_client::qdrant::VectorParamsMap {
                            map: std::collections::HashMap::from([
                                ("dense".to_string(), VectorParams {
                                    size: 384, // Using 384-dim embeddings
                                    distance: Distance::Cosine as i32,
                                    hnsw_config: None,
                                    quantization_config: None,
                                    on_disk: None,
                                    datatype: None,
                                    multivector_config: None,
                                })
                            ])
                        }
                    )),
                }),
                shard_number: None,
                sharding_method: None,
                replication_factor: None,
                write_consistency_factor: None,
                on_disk_payload: None,
                hnsw_config: None,
                wal_config: None,
                optimizers_config: None,
                init_from_collection: None,
                quantization_config: None,
                sparse_vectors_config: None,
                timeout: None,
                strict_mode_config: None,
            };
            
            qdrant_client.create_collection(create_collection).await?;
        }
        
        Ok(Self {
            qdrant_client,
            embedding_handler,
            conversations: Arc::new(RwLock::new(HashMap::new())),
            collection_name,
        })
    }
    
    /// Extract semantic content from a conversation
    fn extract_semantic_content(conversation: &Conversation) -> Vec<String> {
        let mut content_chunks = Vec::new();
        
        // Add title as a chunk
        content_chunks.push(format!("Title: {}", conversation.title));
        
        // Add each message as a chunk
        for (i, message) in conversation.messages.iter().enumerate() {
            content_chunks.push(format!("Message {}: {}", i + 1, message.content));
        }
        
        // Add branch content
        for branch in &conversation.branches {
            content_chunks.push(format!("Branch: {}", branch.title));
            if let Some(description) = &branch.description {
                content_chunks.push(format!("Branch Description: {}", description));
            }
            for (i, message) in branch.messages.iter().enumerate() {
                content_chunks.push(format!("Branch Message {}: {}", i + 1, message.content));
            }
        }
        
        // Add checkpoint information
        for checkpoint in &conversation.checkpoints {
            content_chunks.push(format!("Checkpoint: {}", checkpoint.title));
            if let Some(description) = &checkpoint.description {
                content_chunks.push(format!("Checkpoint Description: {}", description));
            }
        }
        
        // Add tags as semantic context
        if !conversation.tags.is_empty() {
            content_chunks.push(format!("Tags: {}", conversation.tags.join(", ")));
        }
        
        // Add project context if available
        if let Some(ref project_context) = conversation.project_context {
            content_chunks.push(format!("Project: {} ({})", project_context.name, format!("{:?}", project_context.project_type)));
        }
        
        content_chunks
    }
    
    /// Create Qdrant points for indexing
    async fn create_points(&self, conversation: &Conversation) -> Result<Vec<PointStruct>> {
        let content_chunks = Self::extract_semantic_content(conversation);
        let mut points = Vec::new();
        
        // Generate embeddings for all chunks
        let chunk_texts: Vec<&str> = content_chunks.iter().map(|s| s.as_str()).collect();
        let embeddings = self.embedding_handler.embed(&chunk_texts)?;
        
        for (i, (content, embedding)) in content_chunks.iter().zip(embeddings.iter()).enumerate() {
            let point_id_str = format!("{}_{}", conversation.id, i);
            let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
            
            payload.insert("conversation_id".to_string(), conversation.id.to_string().into());
            payload.insert("chunk_index".to_string(), (i as i64).into());
            payload.insert("title".to_string(), conversation.title.clone().into());
            payload.insert("content".to_string(), content.clone().into());
            
            if let Some(workspace_id) = conversation.workspace_id {
                payload.insert("workspace_id".to_string(), workspace_id.to_string().into());
            }
            
            // Add tags to payload
            for (j, tag) in conversation.tags.iter().enumerate() {
                payload.insert(format!("tag_{}", j), tag.clone().into());
            }
            
            let point = PointStruct::new(
                qdrant_client::qdrant::PointId::from(point_id_str),
                qdrant_client::qdrant::NamedVectors::default()
                    .add_vector("dense", embedding.clone()),
                qdrant_client::Payload::from(payload)
            );
            
            points.push(point);
        }
        
        Ok(points)
    }
    
    /// Apply filters to search results
    fn apply_filters(
        &self,
        results: Vec<ScoredPoint>,
        query: &ConversationQuery,
        conversations: &HashMap<Uuid, ConversationSummary>,
    ) -> Vec<ScoredPoint> {
        results.into_iter().filter(|result| {
            // Extract conversation ID from point payload
            if let Some(conv_id_value) = result.payload.get("conversation_id") {
                if let Some(conv_id_str) = conv_id_value.as_str() {
                    if let Ok(conv_id) = Uuid::parse_str(conv_id_str) {
                        if let Some(conversation) = conversations.get(&conv_id) {
                            // Filter by workspace
                            if let Some(workspace_id) = query.workspace_id {
                                if conversation.workspace_id != Some(workspace_id) {
                                    return false;
                                }
                            }
                            
                            // Filter by status
                            if let Some(ref status) = query.status {
                                if &conversation.status != status {
                                    return false;
                                }
                            }
                            
                            // Filter by date range
                            if let Some((start_date, end_date)) = query.date_range {
                                if conversation.last_active < start_date || conversation.last_active > end_date {
                                    return false;
                                }
                            }
                            
                            // Filter by tags
                            if let Some(tags_to_match) = &query.tags {
                                if !tags_to_match.is_empty() {
                                    if !tags_to_match.iter().any(|t| conversation.tags.contains(t)) {
                                        return false;
                                    }
                                }
                            }
                            
                            return true;
                        }
                    }
                }
            }
            false
        }).collect()
    }
    
    /// Group search results by conversation
    fn group_results_by_conversation(
        &self,
        results: Vec<ScoredPoint>,
        conversations: &HashMap<Uuid, ConversationSummary>,
    ) -> Vec<ConversationSearchResult> {
        let mut conversation_results: HashMap<Uuid, (ConversationSummary, Vec<String>, f32)> = HashMap::new();
        
        for result in results {
            if let Some(conv_id_value) = result.payload.get("conversation_id") {
                if let Some(conv_id_str) = conv_id_value.as_str() {
                    if let Ok(conv_id) = Uuid::parse_str(conv_id_str) {
                        if let Some(conversation) = conversations.get(&conv_id) {
                            let entry = conversation_results.entry(conv_id).or_insert_with(|| {
                                (conversation.clone(), Vec::new(), 0.0)
                            });
                            
                            // Add matching snippet
                            if let Some(content_value) = result.payload.get("content") {
                                if let Some(content) = content_value.as_str() {
                                    entry.1.push(content.to_string());
                                }
                            }
                            
                            // Update max score
                            entry.2 = entry.2.max(result.score);
                        }
                    }
                }
            }
        }
        
        // Convert to search results
        let mut search_results: Vec<ConversationSearchResult> = conversation_results
            .into_iter()
            .map(|(id, (summary, snippets, score))| ConversationSearchResult {
                id,
                title: summary.title.clone(),
                relevance_score: score,
                summary_snippet: snippets.first().cloned().or_else(|| summary.title.chars().take(100).collect::<String>().into()),
                last_active: summary.last_active,
                conversation: None,
                matching_snippets: snippets,
                matching_messages: Vec::new(),
            })
            .collect();
        
        // Sort by relevance score
        search_results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        
        search_results
    }

    fn create_search_results(
        &self,
        search_response: qdrant_client::qdrant::SearchResponse,
        conversations_map: &HashMap<Uuid, ConversationSummary> 
    ) -> Vec<ConversationSearchResult> {
        search_response.result.into_iter().filter_map(|hit| {
            let point_id_union = hit.id.as_ref()?.point_id_options.as_ref()?;
            let conversation_id_str = match point_id_union {
                PointIdOptions::Uuid(s) => s.clone(), 
                PointIdOptions::Num(n) => n.to_string(), 
            };
            let conversation_id = Uuid::parse_str(&conversation_id_str).ok()?;
            let conversation_summary = conversations_map.get(&conversation_id)?;
            
            let mut snippets = Vec::new();
            if let Some(payload_value) = hit.payload.get("content_chunks") { 
                if let Some(qdrant_client::qdrant::value::Kind::ListValue(list_val)) = &payload_value.kind { 
                    for val_container in &list_val.values { 
                        if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &val_container.kind { 
                            snippets.push(s.clone());
                        }
                    }
                }
            }

            Some(ConversationSearchResult {
                id: conversation_id,
                title: conversation_summary.title.clone(),
                relevance_score: hit.score,
                summary_snippet: snippets.first().cloned(),
                last_active: conversation_summary.last_active,
                conversation: None,
                matching_snippets: snippets,
                matching_messages: Vec::new(),
            })
        }).collect()
    }
}

#[async_trait]
impl ConversationSearchEngine for SemanticConversationSearchEngine {
    async fn index_conversation(&self, conversation: &Conversation) -> Result<()> {
        // Update conversation cache
        {
            let mut conversations = self.conversations.write().await;
            conversations.insert(conversation.id, conversation.to_summary());
        }
        
        // Create points for vector indexing
        let points = self.create_points(conversation).await?;
        
        // Index points in Qdrant
        use qdrant_client::qdrant::UpsertPoints;
        
        let upsert_points = UpsertPoints {
            collection_name: self.collection_name.clone(),
            points,
            ..Default::default()
        };
        
        self.qdrant_client.upsert_points(upsert_points).await?;
        
        Ok(())
    }
    
    async fn remove_conversation(&self, id: Uuid) -> Result<()> {
        // Remove from conversation cache
        {
            let mut conversations = self.conversations.write().await;
            conversations.remove(&id);
        }
        
        // Remove points from Qdrant
        use qdrant_client::qdrant::{DeletePoints, PointsSelector};
        
        let filter = Filter::must([Condition::matches("conversation_id", id.to_string())]);
        
        let delete_points = DeletePoints {
            collection_name: self.collection_name.clone(),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(filter)
                ),
            }),
            ..Default::default()
        };
        
        self.qdrant_client.delete_points(delete_points).await?;
        
        Ok(())
    }
    
    async fn search(&self, query: &ConversationQuery) -> Result<Vec<ConversationSearchResult>> {
        if query.text.as_ref().map_or(true, |t| t.is_empty()) {
            return Ok(Vec::new());
        }
        let query_text = query.text.as_ref().unwrap();

        let embedding = self.embedding_handler.embed(&[query_text])?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to generate query embedding"))?;

        let mut filter_conditions = Vec::new(); 
        if let Some(status) = &query.status {
            filter_conditions.push(Condition::matches("status", serde_json::to_string(&status)?));
        }
        if let Some(project_type) = &query.project_type {
            filter_conditions.push(Condition::matches("project_type", serde_json::to_string(&project_type)?));
        }
        if let Some(workspace_id) = query.workspace_id {
            filter_conditions.push(Condition::matches("workspace_id", workspace_id.to_string()));
        }

        let qdrant_filter = if filter_conditions.is_empty() { None } else { Some(Filter::must(filter_conditions)) };

        let search_points = SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: embedding,
            limit: query.limit.unwrap_or(10) as u64,
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            score_threshold: None,
            offset: None,
            filter: qdrant_filter,
            params: None,
            vector_name: Some("dense".to_string()),
            read_consistency: None,
            timeout: None,
            shard_key_selector: None,
            sparse_indices: None,
        };

        let search_response = self.qdrant_client.search_points(search_points).await?;

        let relevant_summaries_map: HashMap<Uuid, ConversationSummary> = HashMap::new();
        let mut results = self.create_search_results(search_response, &relevant_summaries_map);

        if let Some((start_date, end_date)) = query.date_range {
            results.retain(|res| {
                if let Some(conv_summary) = relevant_summaries_map.get(&res.id) {
                    conv_summary.last_active >= start_date && conv_summary.last_active <= end_date
                } else {
                    true
                }
            });
        }
        
        if let Some(tags_to_match) = query.tags.as_ref() {
            if !tags_to_match.is_empty() {
                results.retain(|res| {
                    if let Some(conv_summary) = relevant_summaries_map.get(&res.id) {
                        tags_to_match.iter().all(|t| conv_summary.tags.contains(t))
                    } else {
                        false
                    }
                });
            }
        }

        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results)
    }
    
    async fn clear_index(&self) -> Result<()> {
        // Clear conversation cache
        {
            let mut conversations = self.conversations.write().await;
            conversations.clear();
        }
        
        // Clear Qdrant collection
        use qdrant_client::qdrant::{DeletePoints, PointsSelector};
        
        let delete_points = DeletePoints {
            collection_name: self.collection_name.clone(),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Points(
                        qdrant_client::qdrant::PointsIdsList { ids: vec![] }
                    )
                ),
            }),
            ..Default::default()
        };
        
        self.qdrant_client.delete_points(delete_points).await?;
        
        Ok(())
    }
    
    async fn rebuild_index(&self, conversations: &[Conversation]) -> Result<()> {
        // Clear existing index
        self.clear_index().await?;
        
        // Re-index all conversations
        for conversation in conversations {
            self.index_conversation(conversation).await?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::types::{Conversation, ConversationQuery};
    use crate::agent::message::types::AgentMessage;
    use tempfile::TempDir;
    use sagitta_search::config::AppConfig;

    async fn create_test_setup() -> (Arc<Qdrant>, EmbeddingHandler) {
        let temp_dir = TempDir::new().unwrap();
        let qdrant_url = "http://localhost:6334"; // Assume Qdrant is running for tests
        
        let qdrant_client = Arc::new(Qdrant::from_url(qdrant_url).build().unwrap());
        
        // Create a minimal config for embedding handler
        let config = AppConfig::default();
        let embedding_handler = EmbeddingHandler::new(&sagitta_search::app_config_to_embedding_config(&config)).unwrap();
        
        (qdrant_client, embedding_handler)
    }

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_semantic_search_engine_creation() {
        let (qdrant_client, embedding_handler) = create_test_setup().await;
        let engine = SemanticConversationSearchEngine::new(
            qdrant_client,
            embedding_handler,
            "test_conversations".to_string()
        ).await.unwrap();
        
        // Should start with empty index
        let query = ConversationQuery::default();
        let results = engine.search(&query).await.unwrap();
        assert!(results.is_empty());
    }
    
    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_index_and_semantic_search() {
        let (qdrant_client, embedding_handler) = create_test_setup().await;
        let engine = SemanticConversationSearchEngine::new(
            qdrant_client,
            embedding_handler,
            "test_conversations".to_string()
        ).await.unwrap();
        
        let mut conversation = Conversation::new("Rust Programming Help".to_string(), None);
        conversation.add_message(AgentMessage::user("How do I implement async functions in Rust?"));
        conversation.add_message(AgentMessage::assistant("You can use async fn syntax..."));
        conversation.tags.push("rust".to_string());
        conversation.tags.push("async".to_string());
        
        // Index conversation
        engine.index_conversation(&conversation).await.unwrap();
        
        // Search with semantic query
        let mut query = ConversationQuery::default();
        query.text = Some("asynchronous programming".to_string());
        
        let results = engine.search(&query).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].conversation.as_ref().unwrap().id, conversation.id);
        assert!(results[0].relevance_score > 0.0);
    }
} 