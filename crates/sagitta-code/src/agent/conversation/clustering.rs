// Semantic conversation clustering implementation
// TODO: Implement actual clustering algorithms

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use qdrant_client::{Qdrant, qdrant::{PointStruct, Filter, Condition, SearchPoints}};
use sagitta_search::EmbeddingPool;
use crate::agent::conversation::types::ProjectType;
use crate::agent::conversation::cluster_namer::{ClusterNamer, ClusterNamerConfig};
use tokio::sync::RwLock;

use super::types::{Conversation, ConversationSummary};

// Phase 3: Add embedding cache type
type EmbeddingCache = Arc<RwLock<HashMap<String, Vec<f32>>>>;

/// Conversation clustering manager for semantic grouping
pub struct ConversationClusteringManager {
    /// Qdrant client for semantic analysis
    qdrant_client: Arc<Qdrant>,
    
    /// Embedding handler for generating vectors
    embedding_handler: EmbeddingPool,
    
    /// Collection name for clustering
    collection_name: String,
    
    /// Clustering parameters
    config: ClusteringConfig,
    
    /// Cluster namer for generating descriptive names
    cluster_namer: Option<ClusterNamer>,
    
    // Phase 3 optimizations
    /// Embedding cache to avoid O(n²) embedding generation
    embedding_cache: EmbeddingCache,
}

/// Configuration for conversation clustering
#[derive(Debug, Clone)]
pub struct ClusteringConfig {
    /// Minimum similarity threshold for clustering (0.0-1.0)
    pub similarity_threshold: f32,
    
    /// Maximum number of conversations per cluster
    pub max_cluster_size: usize,
    
    /// Minimum number of conversations to form a cluster
    pub min_cluster_size: usize,
    
    /// Whether to use temporal proximity in clustering
    pub use_temporal_proximity: bool,
    
    /// Maximum time difference for temporal clustering (in hours)
    pub max_temporal_distance_hours: u64,
    
    // Phase 3 optimizations
    /// Smart threshold: only cluster if >= this many conversations
    pub smart_clustering_threshold: usize,
    
    /// Enable embedding caching to avoid O(n²) embedding calls
    pub enable_embedding_cache: bool,
    
    /// Use local similarity computation instead of Qdrant searches
    pub use_local_similarity: bool,
    
    /// Run clustering asynchronously in background
    pub async_clustering: bool,
    
    /// Cache size for embeddings (number of conversation embeddings to cache)
    pub embedding_cache_size: usize,
}

impl Default for ClusteringConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.7,
            max_cluster_size: 20,
            min_cluster_size: 2,
            use_temporal_proximity: true,
            max_temporal_distance_hours: 24 * 7, // 1 week
            smart_clustering_threshold: 5,
            enable_embedding_cache: true,
            use_local_similarity: false,
            async_clustering: false,
            embedding_cache_size: 100,
        }
    }
}

/// A cluster of related conversations
#[derive(Debug, Clone)]
pub struct ConversationCluster {
    /// Unique identifier for the cluster
    pub id: Uuid,
    
    /// Cluster title (derived from common themes)
    pub title: String,
    
    /// Conversation IDs in this cluster
    pub conversation_ids: Vec<Uuid>,
    
    /// Cluster centroid (average embedding)
    pub centroid: Vec<f32>,
    
    /// Average similarity score within cluster
    pub cohesion_score: f32,
    
    /// Common tags across conversations
    pub common_tags: Vec<String>,
    
    /// Dominant project type in cluster
    pub dominant_project_type: Option<ProjectType>,
    
    /// Time range of conversations in cluster
    pub time_range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
}

/// Clustering result containing all discovered clusters
#[derive(Debug, Clone)]
pub struct ClusteringResult {
    /// Discovered clusters
    pub clusters: Vec<ConversationCluster>,
    
    /// Conversations that couldn't be clustered
    pub outliers: Vec<Uuid>,
    
    /// Overall clustering quality metrics
    pub metrics: ClusteringMetrics,
}

/// Metrics for evaluating clustering quality
#[derive(Debug, Clone)]
pub struct ClusteringMetrics {
    /// Number of clusters formed
    pub cluster_count: usize,
    
    /// Number of outlier conversations
    pub outlier_count: usize,
    
    /// Average cluster cohesion score
    pub average_cohesion: f32,
    
    /// Silhouette score (measure of clustering quality)
    pub silhouette_score: f32,
}

impl ConversationClusteringManager {
    /// Create a new clustering manager
    pub async fn new(
        qdrant_client: Arc<Qdrant>,
        embedding_handler: EmbeddingPool,
        collection_name: String,
        config: ClusteringConfig,
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
            collection_name,
            config,
            cluster_namer: None,
            embedding_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Create clustering manager with default config
    pub async fn with_default_config(
        qdrant_client: Arc<Qdrant>,
        embedding_handler: EmbeddingPool,
        collection_name: String,
    ) -> Result<Self> {
        Self::new(qdrant_client, embedding_handler, collection_name, ClusteringConfig::default()).await
    }
    
    /// Set the cluster namer for generating descriptive names
    pub fn with_cluster_namer(mut self, cluster_namer: ClusterNamer) -> Self {
        self.cluster_namer = Some(cluster_namer);
        self
    }
    
    /// Set the cluster namer with LLM client
    pub fn with_llm_cluster_namer(mut self, llm_client: Arc<dyn crate::llm::client::LlmClient>) -> Self {
        let cluster_namer = ClusterNamer::new(ClusterNamerConfig::default())
            .with_llm_client(llm_client);
        self.cluster_namer = Some(cluster_namer);
        self
    }
    
    /// Extract clustering features from a conversation
    fn extract_clustering_features(conversation: &Conversation) -> String {
        let mut features = Vec::new();
        
        // Add title
        features.push(conversation.title.clone());
        
        // Add key message content (first few messages)
        for message in conversation.messages.iter().take(3) {
            features.push(message.content.clone());
        }
        
        // Add tags
        features.extend(conversation.tags.clone());
        
        // Add project context
        if let Some(ref project_context) = conversation.project_context {
            features.push(format!("Project: {}", project_context.name));
            features.push(format!("Type: {:?}", project_context.project_type));
        }
        
        features.join(" ")
    }
    
    /// Index conversations for clustering
    pub async fn index_conversations(&self, conversations: &[Conversation]) -> Result<()> {
        // Clear existing collection
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
        
        // Index each conversation
        for conversation in conversations {
            let features = Self::extract_clustering_features(conversation);
            let embedding = sagitta_search::embed_single_text_with_pool(&self.embedding_handler, &features).await?;
            
            let point = PointStruct::new(
                qdrant_client::qdrant::PointId::from(conversation.id.to_string()),
                qdrant_client::qdrant::NamedVectors::default()
                    .add_vector("dense", embedding),
                qdrant_client::Payload::from(self.create_metadata(conversation))
            );
            
            use qdrant_client::qdrant::UpsertPoints;
            
            let upsert_points = UpsertPoints {
                collection_name: self.collection_name.clone(),
                points: vec![point],
                ..Default::default()
            };
            
            self.qdrant_client.upsert_points(upsert_points).await?;
        }
        
        Ok(())
    }
    
    /// Create metadata for a conversation point
    fn create_metadata(&self, conversation: &Conversation) -> HashMap<String, qdrant_client::qdrant::Value> {
        let mut metadata = HashMap::new();
        
        metadata.insert("title".to_string(), conversation.title.clone().into());
        metadata.insert("created_at".to_string(), conversation.created_at.to_rfc3339().into());
        metadata.insert("last_active".to_string(), conversation.last_active.to_rfc3339().into());
        metadata.insert("message_count".to_string(), (conversation.messages.len() as i64).into());
        
        if let Some(workspace_id) = conversation.workspace_id {
            metadata.insert("workspace_id".to_string(), workspace_id.to_string().into());
        }
        
        if let Some(ref project_context) = conversation.project_context {
            metadata.insert("project_type".to_string(), format!("{:?}", project_context.project_type).into());
        }
        
        // Add tags
        for (i, tag) in conversation.tags.iter().enumerate() {
            metadata.insert(format!("tag_{}", i), tag.clone().into());
        }
        
        metadata
    }
    
    /// Perform clustering on indexed conversations
    pub async fn cluster_conversations(&self, conversations: &[ConversationSummary]) -> Result<ClusteringResult> {
        // Phase 3: Smart threshold check - skip clustering if too few conversations
        if conversations.len() < self.config.smart_clustering_threshold {
            log::debug!("Skipping clustering: {} conversations < threshold of {}", 
                conversations.len(), self.config.smart_clustering_threshold);
            return Ok(ClusteringResult {
                clusters: Vec::new(),
                outliers: conversations.iter().map(|c| c.id).collect(),
                metrics: ClusteringMetrics {
                    cluster_count: 0,
                    outlier_count: conversations.len(),
                    average_cohesion: 0.0,
                    silhouette_score: 0.0,
                },
            });
        }

        // Phase 3: Use async clustering if enabled
        if self.config.async_clustering {
            self.cluster_conversations_async(conversations).await
        } else {
            self.cluster_conversations_sync(conversations).await
        }
    }

    /// Phase 3: Asynchronous clustering that can run in background
    pub async fn cluster_conversations_async(&self, conversations: &[ConversationSummary]) -> Result<ClusteringResult> {
        // Instead of spawning a task, just call the optimized method directly
        // The actual async benefit comes from the non-blocking nature of the embedding operations
        self.cluster_conversations_optimized(conversations).await
    }

    /// Phase 3: Synchronous clustering (existing implementation)
    async fn cluster_conversations_sync(&self, conversations: &[ConversationSummary]) -> Result<ClusteringResult> {
        self.cluster_conversations_optimized(conversations).await
    }

    /// Phase 3: Optimized clustering with caching and local similarity
    async fn cluster_conversations_optimized(&self, conversations: &[ConversationSummary]) -> Result<ClusteringResult> {
        if conversations.is_empty() {
            return Ok(ClusteringResult {
                clusters: Vec::new(),
                outliers: Vec::new(),
                metrics: ClusteringMetrics {
                    cluster_count: 0,
                    outlier_count: 0,
                    average_cohesion: 0.0,
                    silhouette_score: 0.0,
                },
            });
        }
        
        // Phase 3: Build optimized similarity matrix with caching
        let similarity_matrix = if self.config.use_local_similarity {
            self.build_similarity_matrix_local(conversations).await?
        } else {
            self.build_similarity_matrix_cached(conversations).await?
        };
        
        // Apply clustering algorithm
        let clusters = self.apply_clustering_algorithm(&similarity_matrix, conversations).await?;
        
        // Identify outliers
        let clustered_ids: std::collections::HashSet<Uuid> = clusters
            .iter()
            .flat_map(|c| c.conversation_ids.iter())
            .copied()
            .collect();
        
        let outliers: Vec<Uuid> = conversations
            .iter()
            .map(|c| c.id)
            .filter(|id| !clustered_ids.contains(id))
            .collect();
        
        // Calculate metrics
        let metrics = self.calculate_clustering_metrics(&clusters, &outliers, &similarity_matrix);
        
        Ok(ClusteringResult {
            clusters,
            outliers,
            metrics,
        })
    }
    
    /// Build similarity matrix between conversations
    async fn build_similarity_matrix(&self, conversations: &[ConversationSummary]) -> Result<Vec<Vec<f32>>> {
        let n = conversations.len();
        let mut matrix = vec![vec![0.0; n]; n];
        
        for i in 0..n {
            for j in i..n {
                if i == j {
                    matrix[i][j] = 1.0;
                } else {
                    let similarity = self.calculate_conversation_similarity(
                        &conversations[i],
                        &conversations[j],
                    ).await?;
                    matrix[i][j] = similarity;
                    matrix[j][i] = similarity;
                }
            }
        }
        
        Ok(matrix)
    }
    
    /// Phase 3: Build similarity matrix with embedding caching
    async fn build_similarity_matrix_cached(&self, conversations: &[ConversationSummary]) -> Result<Vec<Vec<f32>>> {
        let n = conversations.len();
        let mut matrix = vec![vec![0.0; n]; n];
        
        // Pre-generate and cache all embeddings
        for conversation in conversations {
            let _ = self.get_or_generate_embedding(&conversation.title).await?;
        }
        
        for i in 0..n {
            for j in i..n {
                if i == j {
                    matrix[i][j] = 1.0;
                } else {
                    let similarity = self.calculate_conversation_similarity_cached(
                        &conversations[i],
                        &conversations[j],
                    ).await?;
                    matrix[i][j] = similarity;
                    matrix[j][i] = similarity;
                }
            }
        }
        
        Ok(matrix)
    }

    /// Phase 3: Build similarity matrix using local computation (fastest)
    async fn build_similarity_matrix_local(&self, conversations: &[ConversationSummary]) -> Result<Vec<Vec<f32>>> {
        let n = conversations.len();
        let mut matrix = vec![vec![0.0; n]; n];
        
        // Pre-generate all embeddings in parallel
        let mut embeddings = Vec::with_capacity(n);
        for conversation in conversations {
            let embedding = self.get_or_generate_embedding(&conversation.title).await?;
            embeddings.push(embedding);
        }
        
        // Compute all similarities locally using cosine similarity
        for i in 0..n {
            for j in i..n {
                if i == j {
                    matrix[i][j] = 1.0;
                } else {
                    let similarity = self.calculate_local_similarity(
                        &embeddings[i], 
                        &embeddings[j],
                        &conversations[i],
                        &conversations[j],
                    );
                    matrix[i][j] = similarity;
                    matrix[j][i] = similarity;
                }
            }
        }
        
        Ok(matrix)
    }

    /// Phase 3: Get embedding from cache or generate and cache it
    async fn get_or_generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // Check cache first
        {
            let cache = self.embedding_cache.read().await;
            if let Some(embedding) = cache.get(text) {
                return Ok(embedding.clone());
            }
        }
        
        // Generate new embedding
        let embedding = sagitta_search::embed_single_text_with_pool(&self.embedding_handler, text).await?;
        
        // Cache it (with size limit)
        {
            let mut cache = self.embedding_cache.write().await;
            if cache.len() >= self.config.embedding_cache_size {
                // Simple LRU: remove oldest (in practice, remove arbitrary entry)
                if let Some(key) = cache.keys().next().cloned() {
                    cache.remove(&key);
                }
            }
            cache.insert(text.to_string(), embedding.clone());
        }
        
        Ok(embedding)
    }

    /// Phase 3: Calculate similarity using cached embeddings
    async fn calculate_conversation_similarity_cached(
        &self,
        conv1: &ConversationSummary,
        conv2: &ConversationSummary,
    ) -> Result<f32> {
        // Get embeddings from cache (they should already be there)
        let emb1 = self.get_or_generate_embedding(&conv1.title).await?;
        let emb2 = self.get_or_generate_embedding(&conv2.title).await?;
        
        // Calculate cosine similarity locally
        let mut semantic_similarity = cosine_similarity(&emb1, &emb2);
        
        // Apply temporal proximity if enabled
        if self.config.use_temporal_proximity {
            semantic_similarity = self.apply_temporal_weighting(semantic_similarity, conv1, conv2);
        }
        
        // Tag similarity bonus
        semantic_similarity = self.apply_tag_weighting(semantic_similarity, conv1, conv2);
        
        Ok(semantic_similarity.clamp(0.0, 1.0))
    }

    /// Phase 3: Calculate similarity using local computation only
    fn calculate_local_similarity(
        &self,
        emb1: &[f32],
        emb2: &[f32],
        conv1: &ConversationSummary,
        conv2: &ConversationSummary,
    ) -> f32 {
        // Calculate cosine similarity locally
        let mut semantic_similarity = cosine_similarity(emb1, emb2);
        
        // Apply temporal proximity if enabled
        if self.config.use_temporal_proximity {
            semantic_similarity = self.apply_temporal_weighting(semantic_similarity, conv1, conv2);
        }
        
        // Tag similarity bonus
        semantic_similarity = self.apply_tag_weighting(semantic_similarity, conv1, conv2);
        
        semantic_similarity.clamp(0.0, 1.0)
    }

    /// Phase 3: Apply temporal weighting to similarity score
    fn apply_temporal_weighting(&self, base_similarity: f32, conv1: &ConversationSummary, conv2: &ConversationSummary) -> f32 {
        let time_diff = (conv1.last_active - conv2.last_active).num_hours().abs() as u64;
        let max_diff = self.config.max_temporal_distance_hours;
        
        if time_diff <= max_diff {
            let temporal_factor = 1.0 - (time_diff as f32 / max_diff as f32);
            base_similarity * 0.7 + temporal_factor * 0.3
        } else {
            base_similarity * 0.5 // Reduce similarity for distant conversations
        }
    }

    /// Phase 3: Apply tag weighting to similarity score
    fn apply_tag_weighting(&self, base_similarity: f32, conv1: &ConversationSummary, conv2: &ConversationSummary) -> f32 {
        let common_tags = conv1.tags.iter()
            .filter(|tag| conv2.tags.contains(tag))
            .count();
        
        if common_tags > 0 {
            let tag_similarity = common_tags as f32 / (conv1.tags.len() + conv2.tags.len()) as f32;
            base_similarity * 0.8 + tag_similarity * 0.2
        } else {
            base_similarity
        }
    }
    
    /// Calculate similarity between two conversations
    async fn calculate_conversation_similarity(
        &self,
        conv1: &ConversationSummary,
        conv2: &ConversationSummary,
    ) -> Result<f32> {
        // Generate embedding for conv1 title
        let conv1_embedding = sagitta_search::embed_single_text_with_pool(&self.embedding_handler, &conv1.title).await?;
        
        // Search for conv2 using conv1's embedding
        let search_points = SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: conv1_embedding,
            limit: 1,
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            score_threshold: None,
            offset: None,
            filter: Some(Filter::must([Condition::matches("title", conv2.title.clone())])),
            params: None,
            vector_name: Some("dense".to_string()), // Specify the named vector
            read_consistency: None,
            timeout: None,
            shard_key_selector: None,
            sparse_indices: None,
        };
        
        let search_response = self.qdrant_client.search_points(search_points).await?;
        
        let mut semantic_similarity = if let Some(result) = search_response.result.first() {
            result.score
        } else {
            0.0
        };
        
        // Apply temporal proximity if enabled
        if self.config.use_temporal_proximity {
            let time_diff = (conv1.last_active - conv2.last_active).num_hours().abs() as u64;
            let max_diff = self.config.max_temporal_distance_hours;
            
            if time_diff <= max_diff {
                let temporal_factor = 1.0 - (time_diff as f32 / max_diff as f32);
                semantic_similarity = semantic_similarity * 0.7 + temporal_factor * 0.3;
            } else {
                semantic_similarity *= 0.5; // Reduce similarity for distant conversations
            }
        }
        
        // Tag similarity bonus
        let common_tags = conv1.tags.iter()
            .filter(|tag| conv2.tags.contains(tag))
            .count();
        
        if common_tags > 0 {
            let tag_similarity = common_tags as f32 / (conv1.tags.len() + conv2.tags.len()) as f32;
            semantic_similarity = semantic_similarity * 0.8 + tag_similarity * 0.2;
        }
        
        Ok(semantic_similarity.clamp(0.0, 1.0))
    }
    
    /// Apply clustering algorithm (hierarchical clustering)
    async fn apply_clustering_algorithm(
        &self,
        similarity_matrix: &[Vec<f32>],
        conversations: &[ConversationSummary],
    ) -> Result<Vec<ConversationCluster>> {
        let mut clusters = Vec::new();
        let mut assigned = vec![false; conversations.len()];
        
        for i in 0..conversations.len() {
            if assigned[i] {
                continue;
            }
            
            let mut cluster_members = vec![i];
            assigned[i] = true;
            
            // Find similar conversations
            for j in (i + 1)..conversations.len() {
                if assigned[j] {
                    continue;
                }
                
                if similarity_matrix[i][j] >= self.config.similarity_threshold {
                    cluster_members.push(j);
                    assigned[j] = true;
                    
                    if cluster_members.len() >= self.config.max_cluster_size {
                        break;
                    }
                }
            }
            
            // Only create cluster if it meets minimum size requirement
            if cluster_members.len() >= self.config.min_cluster_size {
                let cluster = self.create_cluster(cluster_members, conversations, similarity_matrix).await?;
                clusters.push(cluster);
            } else {
                // Mark as unassigned (will become outliers)
                for &member in &cluster_members {
                    assigned[member] = false;
                }
            }
        }
        
        Ok(clusters)
    }
    
    /// Create a cluster from member indices
    async fn create_cluster(
        &self,
        member_indices: Vec<usize>,
        conversations: &[ConversationSummary],
        similarity_matrix: &[Vec<f32>],
    ) -> Result<ConversationCluster> {
        let conversation_ids: Vec<Uuid> = member_indices
            .iter()
            .map(|&i| conversations[i].id)
            .collect();
        
        // Calculate cohesion score (average pairwise similarity)
        let mut total_similarity = 0.0;
        let mut pair_count = 0;
        
        for i in 0..member_indices.len() {
            for j in (i + 1)..member_indices.len() {
                total_similarity += similarity_matrix[member_indices[i]][member_indices[j]];
                pair_count += 1;
            }
        }
        
        let cohesion_score = if pair_count > 0 {
            total_similarity / pair_count as f32
        } else {
            1.0
        };
        
        // Extract common tags
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for &i in &member_indices {
            for tag in &conversations[i].tags {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
        
        let common_tags: Vec<String> = tag_counts
            .into_iter()
            .filter(|(_, count)| *count >= member_indices.len() / 2) // At least half the conversations
            .map(|(tag, _)| tag)
            .collect();
        
        // Determine dominant project type
        let mut project_type_counts: HashMap<Option<ProjectType>, usize> = HashMap::new();
        for &i in &member_indices {
            if let Some(ref project_name) = conversations[i].project_name {
                // Simple heuristic to determine project type from name
                let project_type = self.infer_project_type_from_name(project_name);
                *project_type_counts.entry(Some(project_type)).or_insert(0) += 1;
            } else {
                *project_type_counts.entry(None).or_insert(0) += 1;
            }
        }
        
        let dominant_project_type = project_type_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .and_then(|(project_type, _)| project_type);
        
        // Calculate time range
        let mut min_time = conversations[member_indices[0]].created_at;
        let mut max_time = conversations[member_indices[0]].last_active;
        
        for &i in &member_indices {
            min_time = min_time.min(conversations[i].created_at);
            max_time = max_time.max(conversations[i].last_active);
        }
        
        // Create initial cluster with placeholder title
        let mut cluster = ConversationCluster {
            id: Uuid::new_v4(),
            title: "Placeholder".to_string(), // Will be replaced
            conversation_ids,
            centroid: Vec::new(), // TODO: Calculate actual centroid
            cohesion_score,
            common_tags,
            dominant_project_type,
            time_range: (min_time, max_time),
        };
        
        // Generate cluster title using ClusterNamer if available
        let title = if let Some(ref cluster_namer) = self.cluster_namer {
            match cluster_namer.generate_cluster_name(&cluster, conversations).await {
                Ok(generated_title) => generated_title,
                Err(e) => {
                    eprintln!("Failed to generate cluster name: {}", e);
                    self.generate_cluster_title(&member_indices, conversations, &cluster.common_tags)
                }
            }
        } else {
            // Fall back to the original method
            self.generate_cluster_title(&member_indices, conversations, &cluster.common_tags)
        };
        
        cluster.title = title;
        Ok(cluster)
    }
    
    /// Infer project type from project name (simple heuristic)
    fn infer_project_type_from_name(&self, project_name: &str) -> ProjectType {
        let name_lower = project_name.to_lowercase();
        
        if name_lower.contains("rust") || name_lower.contains("cargo") || name_lower.ends_with(".rs") {
            ProjectType::Rust
        } else if name_lower.contains("python") || name_lower.contains("py") || name_lower.ends_with(".py") {
            ProjectType::Python
        } else if name_lower.contains("javascript") || name_lower.contains("js") || name_lower.contains("node") || name_lower.ends_with(".js") || name_lower.ends_with(".jsx") {
            ProjectType::JavaScript
        } else if name_lower.contains("typescript") || name_lower.contains("ts") || name_lower.ends_with(".ts") || name_lower.ends_with(".tsx") {
            ProjectType::TypeScript
        } else if name_lower.contains("go") || name_lower.contains("golang") || name_lower.ends_with(".go") {
            ProjectType::Go
        } else if name_lower.contains("ruby") || name_lower.contains("rb") || name_lower.ends_with(".rb") {
            ProjectType::Ruby
        } else if name_lower.contains("markdown") || name_lower.contains("md") || name_lower.ends_with(".md") {
            ProjectType::Markdown
        } else if name_lower.contains("yaml") || name_lower.contains("yml") || name_lower.ends_with(".yaml") || name_lower.ends_with(".yml") {
            ProjectType::Yaml
        } else if name_lower.contains("html") || name_lower.ends_with(".html") {
            ProjectType::Html
        } else {
            ProjectType::Unknown
        }
    }
    
    /// Generate a descriptive title for a cluster
    fn generate_cluster_title(
        &self,
        member_indices: &[usize],
        conversations: &[ConversationSummary],
        common_tags: &[String],
    ) -> String {
        // Use common tags if available
        if !common_tags.is_empty() {
            return format!("{} Conversations", common_tags.join(", "));
        }
        
        // Use common words from titles
        let titles: Vec<&str> = member_indices
            .iter()
            .map(|&i| conversations[i].title.as_str())
            .collect();
        
        let common_words = self.find_common_words(&titles);
        if !common_words.is_empty() {
            return format!("{} Related", common_words.join(" "));
        }
        
        // Fallback to generic title
        format!("Cluster of {} Conversations", member_indices.len())
    }
    
    /// Find common words across titles
    fn find_common_words(&self, titles: &[&str]) -> Vec<String> {
        let mut word_counts: HashMap<String, usize> = HashMap::new();
        
        for title in titles {
            let words: std::collections::HashSet<String> = title
                .to_lowercase()
                .split_whitespace()
                .filter(|word| word.len() > 3) // Only consider longer words
                .map(|word| word.to_string())
                .collect();
            
            for word in words {
                *word_counts.entry(word).or_insert(0) += 1;
            }
        }
        
        word_counts
            .into_iter()
            .filter(|(_, count)| *count >= titles.len() / 2) // At least half the titles
            .map(|(word, _)| word)
            .take(3) // Limit to 3 words
            .collect()
    }
    
    /// Calculate clustering quality metrics
    fn calculate_clustering_metrics(
        &self,
        clusters: &[ConversationCluster],
        outliers: &[Uuid],
        _similarity_matrix: &[Vec<f32>],
    ) -> ClusteringMetrics {
        let cluster_count = clusters.len();
        let outlier_count = outliers.len();
        
        let average_cohesion = if !clusters.is_empty() {
            clusters.iter().map(|c| c.cohesion_score).sum::<f32>() / clusters.len() as f32
        } else {
            0.0
        };
        
        // Simplified silhouette score calculation
        let silhouette_score = if cluster_count > 1 {
            average_cohesion * 0.8 // Simplified approximation
        } else {
            0.0
        };
        
        ClusteringMetrics {
            cluster_count,
            outlier_count,
            average_cohesion,
            silhouette_score,
        }
    }
    
    /// Update clustering configuration
    pub fn update_config(&mut self, config: ClusteringConfig) {
        self.config = config;
    }
    
    /// Get current clustering configuration
    pub fn get_config(&self) -> &ClusteringConfig {
        &self.config
    }
    
    /// Finalize clusters by ensuring all have proper names
    pub async fn finalize_clusters(&self, mut clusters: Vec<ConversationCluster>, conversations: &[ConversationSummary]) -> Result<Vec<ConversationCluster>> {
        if let Some(ref cluster_namer) = self.cluster_namer {
            for cluster in &mut clusters {
                // Only regenerate if the title looks like a placeholder or is generic
                if cluster.title.starts_with("Cluster") || 
                   cluster.title.starts_with("Placeholder") ||
                   cluster.title.contains("Conversations") && cluster.title.len() < 20 {
                    
                    match cluster_namer.generate_cluster_name(cluster, conversations).await {
                        Ok(new_title) => {
                            if !new_title.is_empty() && new_title.len() > 3 {
                                cluster.title = new_title;
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to finalize cluster name for {}: {}", cluster.id, e);
                        }
                    }
                }
            }
        }
        
        Ok(clusters)
    }
}

/// Calculate cosine similarity between two embedding vectors
fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> f32 {
    if vec1.len() != vec2.len() {
        return 0.0;
    }
    
    let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = vec1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm2: f32 = vec2.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm1 == 0.0 || norm2 == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm1 * norm2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::types::ConversationStatus;
    use tempfile::TempDir;

    async fn create_test_setup() -> (Arc<Qdrant>, EmbeddingPool) {
        let config = AppConfig::default();
        let client = Arc::new(Qdrant::from_url("http://localhost:6333").build().unwrap());
        
        let embedding_config = sagitta_search::app_config_to_embedding_config(&config);
        let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config).unwrap();
        
        (client, embedding_pool)
    }

    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_clustering_manager_creation() {
        let (qdrant_client, embedding_handler) = create_test_setup().await;
        let manager = ConversationClusteringManager::with_default_config(
            qdrant_client,
            embedding_handler,
            "test_clustering".to_string(),
        ).await.unwrap();
        
        assert_eq!(manager.config.similarity_threshold, 0.7);
        assert_eq!(manager.config.max_cluster_size, 20);
    }
    
    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_empty_clustering() {
        let (qdrant_client, embedding_handler) = create_test_setup().await;
        let manager = ConversationClusteringManager::with_default_config(
            qdrant_client,
            embedding_handler,
            "test_clustering".to_string(),
        ).await.unwrap();
        
        let result = manager.cluster_conversations(&[]).await.unwrap();
        
        assert_eq!(result.clusters.len(), 0);
        assert_eq!(result.outliers.len(), 0);
        assert_eq!(result.metrics.cluster_count, 0);
    }
    
    #[tokio::test]
    #[ignore] // Requires running Qdrant instance
    async fn test_clustering_config_update() {
        let (qdrant_client, embedding_handler) = create_test_setup().await;
        let mut manager = ConversationClusteringManager::with_default_config(
            qdrant_client,
            embedding_handler,
            "test_clustering".to_string(),
        ).await.unwrap();
        
        let new_config = ClusteringConfig {
            similarity_threshold: 0.8,
            max_cluster_size: 10,
            min_cluster_size: 3,
            use_temporal_proximity: false,
            max_temporal_distance_hours: 48,
            smart_clustering_threshold: 5,
            enable_embedding_cache: true,
            use_local_similarity: false,
            async_clustering: false,
            embedding_cache_size: 100,
        };
        
        manager.update_config(new_config.clone());
        
        assert_eq!(manager.get_config().similarity_threshold, 0.8);
        assert_eq!(manager.get_config().max_cluster_size, 10);
        assert!(!manager.get_config().use_temporal_proximity);
    }
    
    #[test]
    #[ignore] // Requires ONNX model configuration
    fn test_find_common_words() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (qdrant_client, embedding_handler) = rt.block_on(create_test_setup());
        
        let manager_future = ConversationClusteringManager::with_default_config(
            qdrant_client,
            embedding_handler,
            "test".to_string(),
        );
        let manager = rt.block_on(manager_future).unwrap();
        
        let titles = vec![
            "Rust programming help",
            "Rust async programming",
            "Programming in Rust",
        ];
        
        let common_words = manager.find_common_words(&titles);
        assert!(common_words.contains(&"rust".to_string()) || common_words.contains(&"programming".to_string()));
    }
    
    #[test]
    #[ignore] // Requires ONNX model configuration
    fn test_generate_cluster_title() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (qdrant_client, embedding_handler) = rt.block_on(create_test_setup());
        
        let manager_future = ConversationClusteringManager::with_default_config(
            qdrant_client,
            embedding_handler,
            "test".to_string(),
        );
        let manager = rt.block_on(manager_future).unwrap();
        
        let conversations = vec![
            ConversationSummary {
                id: Uuid::new_v4(),
                title: "Rust Help".to_string(),
                created_at: chrono::Utc::now(),
                last_active: chrono::Utc::now(),
                message_count: 5,
                status: ConversationStatus::Active,
                tags: vec!["rust".to_string()],
                workspace_id: None,
                has_branches: false,
                has_checkpoints: false,
                project_name: None,
            },
        ];
        
        let common_tags = vec!["rust".to_string()];
        let title = manager.generate_cluster_title(&[0], &conversations, &common_tags);
        
        assert_eq!(title, "rust Conversations");
    }
} 