use std::sync::Arc;
use tonic::{Request, Response, Status};
use qdrant_client::Qdrant;
use crate::config::AppConfig;
use crate::server::error::{Result, ServerError};
use crate::server::auth::{ApiKeyAuthenticator, authenticate_request};
use chrono::Utc;
use qdrant_client::qdrant::{Distance, CreateCollectionBuilder, VectorParamsBuilder};
use std::fmt;

// Import the generated gRPC code conditionally
#[cfg(feature = "server")]
use vectordb_proto::vectordb::{
    Empty, ServerInfo, ModelInfo, StatusResponse, CreateCollectionRequest,
    CollectionRequest, ListCollectionsResponse, QueryRequest, QueryResponse,
    SearchResult, IndexFilesRequest, IndexResponse, AddRepositoryRequest,
    ListRepositoriesResponse, RepositoryRequest, RemoveRepositoryRequest,
    SyncRepositoryRequest, UseBranchRequest, RepositoryInfo
};

#[cfg(feature = "server")]
use vectordb_proto::vector_db_service_server::VectorDbService;
use crate::cli;

#[cfg(feature = "server")]
use std::time::Instant;

// Import the right types for filters
#[cfg(feature = "server")]
use qdrant_client::qdrant::{Filter, Condition, FieldCondition, SearchPoints, WithPayloadSelector};
#[cfg(feature = "server")]
use qdrant_client::qdrant::with_payload_selector::SelectorOptions;
#[cfg(feature = "server")]
use qdrant_client::qdrant::r#match::MatchValue;
#[cfg(feature = "server")]
use qdrant_client::qdrant::condition::ConditionOneOf;

// Service implementation
pub struct VectorDBServiceImpl {
    config: Arc<AppConfig>,
    client: Arc<Qdrant>,
    authenticator: Option<ApiKeyAuthenticator>,
    pub(crate) version: String,
    pub(crate) build_date: String,
}

// Manual implementation of Debug since Qdrant doesn't implement it
impl fmt::Debug for VectorDBServiceImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VectorDBServiceImpl")
            .field("config", &self.config)
            .field("client", &"<Qdrant client>")
            .field("authenticator", &self.authenticator)
            .field("version", &self.version)
            .field("build_date", &self.build_date)
            .finish()
    }
}

impl VectorDBServiceImpl {
    /// Create a new service implementation
    pub fn new(config: Arc<AppConfig>, client: Arc<Qdrant>) -> Self {
        // TODO: Initialize authenticator from server config when available
        Self {
            config,
            client,
            authenticator: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_date: env!("CARGO_PKG_VERSION").to_string(), // This should be build date but for now we'll use version
        }
    }
    
    /// Set the authenticator for this service
    pub fn with_authenticator(mut self, authenticator: ApiKeyAuthenticator) -> Self {
        self.authenticator = Some(authenticator);
        self
    }
    
    /// Authenticate a request
    fn authenticate<T>(&self, request: &Request<T>) -> Result<()> {
        if let Some(auth) = &self.authenticator {
            auth.authenticate(request)?;
        }
        Ok(())
    }
}

// Conditional implementation of the gRPC service
#[cfg(feature = "server")]
#[tonic::async_trait]
impl VectorDbService for VectorDBServiceImpl {
    async fn get_server_info(
        &self,
        request: Request<Empty>,
    ) -> std::result::Result<Response<ServerInfo>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Get model information
        let model_info = ModelInfo {
            model_path: self.config.onnx_model_path.clone().unwrap_or_default(),
            tokenizer_path: self.config.onnx_tokenizer_path.clone().unwrap_or_default(),
            vector_dimension: 384, // Default dimension (should be dynamically detected)
            model_type: "onnx".to_string(),
        };
        
        // Create response
        let response = ServerInfo {
            version: self.version.clone(),
            build_date: self.build_date.clone(),
            is_healthy: true,
            model_info: Some(model_info),
        };
        
        Ok(Response::new(response))
    }
    
    async fn create_collection(
        &self,
        request: Request<CreateCollectionRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let req = request.into_inner();
        let collection_name = req.name;
        let vector_size = req.vector_size as u64;
        
        // Parse distance metric
        let distance = match req.distance.as_str() {
            "cosine" => Distance::Cosine,
            "euclidean" => Distance::Euclid,
            "dot" => Distance::Dot,
            _ => {
                return Ok(Response::new(StatusResponse {
                    success: false,
                    message: format!("Invalid distance metric: {}", req.distance),
                }));
            }
        };
        
        // Create the collection in Qdrant using Builder pattern
        let create_request = CreateCollectionBuilder::new(&collection_name)
            .vectors_config(VectorParamsBuilder::new(vector_size, distance));
            
        let create_result = self.client
            .create_collection(create_request)
            .await;
            
        // Handle the result
        match create_result {
            Ok(_) => {
                Ok(Response::new(StatusResponse {
                    success: true,
                    message: format!("Collection '{}' created successfully", collection_name),
                }))
            }
            Err(e) => {
                // Check if collection already exists
                if e.to_string().contains("already exists") {
                    Ok(Response::new(StatusResponse {
                        success: false,
                        message: format!("Collection '{}' already exists", collection_name),
                    }))
                } else {
                    Err(Status::internal(format!("Failed to create collection: {}", e)))
                }
            }
        }
    }
    
    async fn list_collections(
        &self,
        request: Request<Empty>,
    ) -> std::result::Result<Response<ListCollectionsResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Get collections from Qdrant
        let collections_result = self.client.list_collections().await;
        
        match collections_result {
            Ok(response) => {
                let collection_names = response
                    .collections
                    .into_iter()
                    .map(|c| c.name)
                    .collect();
                
                Ok(Response::new(ListCollectionsResponse {
                    collections: collection_names,
                }))
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to list collections: {}", e)))
            }
        }
    }
    
    async fn delete_collection(
        &self,
        request: Request<CollectionRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract the collection name
        let collection_name = request.into_inner().name;
        
        // Delete the collection
        let delete_result = self.client.delete_collection(collection_name.clone()).await;
        
        match delete_result {
            Ok(_) => {
                Ok(Response::new(StatusResponse {
                    success: true,
                    message: format!("Collection '{}' deleted successfully", collection_name),
                }))
            }
            Err(e) => {
                if e.to_string().contains("not found") || e.to_string().contains("doesn't exist") {
                    Ok(Response::new(StatusResponse {
                        success: false,
                        message: format!("Collection '{}' does not exist", collection_name),
                    }))
                } else {
                    Err(Status::internal(format!("Failed to delete collection: {}", e)))
                }
            }
        }
    }
    
    async fn clear_collection(
        &self,
        request: Request<CollectionRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract the collection name
        let collection_name = request.into_inner().name;
        
        // First check if the collection exists
        let collections_result = self.client.list_collections().await;
        
        match collections_result {
            Ok(response) => {
                let collections = response.collections.into_iter().map(|c| c.name).collect::<Vec<String>>();
                
                if !collections.contains(&collection_name) {
                    return Ok(Response::new(StatusResponse {
                        success: false,
                        message: format!("Collection '{}' does not exist", collection_name),
                    }));
                }
            }
            Err(e) => {
                return Err(Status::internal(format!("Failed to list collections: {}", e)));
            }
        }
        
        // Delete the collection
        let delete_result = self.client.delete_collection(collection_name.clone()).await;
        
        match delete_result {
            Ok(_) => {
                // Create an empty collection again with the same name/settings using Builder pattern
                // We assume the vector size is 384 which is our default
                let create_request = CreateCollectionBuilder::new(&collection_name)
                    .vectors_config(VectorParamsBuilder::new(384, Distance::Cosine));
                    
                let create_result = self.client
                    .create_collection(create_request)
                    .await;
                    
                match create_result {
                    Ok(_) => {
                        Ok(Response::new(StatusResponse {
                            success: true,
                            message: format!("Collection '{}' cleared successfully", collection_name),
                        }))
                    }
                    Err(e) => {
                        Err(Status::internal(format!("Failed to recreate collection after clearing: {}", e)))
                    }
                }
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to clear collection: {}", e)))
            }
        }
    }
    
    async fn index_files(
        &self,
        request: Request<IndexFilesRequest>,
    ) -> std::result::Result<Response<IndexResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let req = request.into_inner();
        let paths = req.paths;
        let extensions = req.extensions;
        let collection_name = req.collection_name;
        
        // First check if the collection exists
        let collections_result = self.client.list_collections().await;
        
        let collection_exists = match collections_result {
            Ok(response) => {
                let collections = response.collections.into_iter().map(|c| c.name).collect::<Vec<String>>();
                collections.contains(&collection_name)
            }
            Err(e) => {
                return Err(Status::internal(format!("Failed to list collections: {}", e)));
            }
        };
        
        // If collection doesn't exist, create it
        if !collection_exists {
            let create_request = CreateCollectionBuilder::new(&collection_name)
                .vectors_config(VectorParamsBuilder::new(384, Distance::Cosine));
                
            let create_result = self.client
                .create_collection(create_request)
                .await;
                
            if let Err(e) = create_result {
                return Err(Status::internal(format!("Failed to create collection: {}", e)));
            }
        }
        
        // Use the CLI's simple index logic to perform the indexing
        // Since this is already implemented in the CLI, we'll call into that directly
        // We're adapting it to work within the server context
        
        #[cfg(feature = "onnx")]
        let index_result = {
            use crate::vectordb::embedding_logic::EmbeddingHandler;
            use crate::vectordb::embedding::EmbeddingModelType;
            use std::path::PathBuf;
            use indicatif::ProgressBar;
            use crate::cli::simple::index;
            
            // Create embedding handler
            let handler = EmbeddingHandler::new(
                EmbeddingModelType::Onnx,
                self.config.onnx_model_path.clone().map(PathBuf::from),
                self.config.onnx_tokenizer_path.clone().map(PathBuf::from),
            ).map_err(|e| Status::internal(format!("Failed to create embedding handler: {}", e)))?;
            
            // Convert paths to PathBufs
            let path_bufs = paths.into_iter().map(PathBuf::from).collect::<Vec<PathBuf>>();
            
            // Create a hidden progress bar for server-side operations
            let progress_bar = ProgressBar::hidden();
            
            // Run the indexing operation
            let result = index::index_paths(
                self.client.as_ref(),
                &collection_name,
                &handler,
                &path_bufs,
                &extensions,
                &progress_bar,
            ).await;
            
            match result {
                Ok((indexed_files, indexed_chunks)) => {
                    Ok(Response::new(IndexResponse {
                        success: true,
                        message: format!("Successfully indexed {} files with {} chunks", indexed_files, indexed_chunks),
                        indexed_files: indexed_files as i32,
                        indexed_chunks: indexed_chunks as i32,
                    }))
                }
                Err(e) => {
                    Err(Status::internal(format!("Indexing failed: {}", e)))
                }
            }
        };
        
        #[cfg(not(feature = "onnx"))]
        let index_result = Err(Status::internal("ONNX support not enabled"));
        
        index_result
    }
    
    async fn query_collection(
        &self,
        request: Request<QueryRequest>,
    ) -> std::result::Result<Response<QueryResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let req = request.into_inner();
        let query_text = req.query_text;
        let collection_name = req.collection_name;
        let limit = req.limit as u64;
        let language_filter = req.language;
        let element_type_filter = req.element_type;
        
        // Create embedding for the query
        #[cfg(feature = "onnx")]
        let embedding_result = {
            use crate::vectordb::embedding_logic::EmbeddingHandler;
            use crate::vectordb::embedding::EmbeddingModelType;
            use std::path::PathBuf;
            
            let handler = EmbeddingHandler::new(
                EmbeddingModelType::Onnx,
                self.config.onnx_model_path.clone().map(PathBuf::from),
                self.config.onnx_tokenizer_path.clone().map(PathBuf::from),
            ).map_err(|e| Status::internal(format!("Failed to create embedding handler: {}", e)))?;
            
            // Fix: Use embed method instead of get_embedding
            handler.embed(&[&query_text])
                .map_err(|e| Status::internal(format!("Failed to create query embedding: {}", e)))
                .map(|embeddings| embeddings[0].clone())
        };
        
        #[cfg(not(feature = "onnx"))]
        let embedding_result = Err(Status::internal("ONNX support not enabled"));
        
        let embedding = embedding_result?;
        
        // Start measuring query time
        let start = Instant::now();
        
        // Create a SearchPointsBuilder for the query
        use qdrant_client::qdrant::SearchPointsBuilder;
        
        // Initialize filter
        let mut filter_conditions = Vec::new();
        
        // Add language filter if provided
        if let Some(lang) = language_filter {
            use qdrant_client::qdrant::FieldCondition;
            use qdrant_client::qdrant::Match;
            
            let lang_condition = FieldCondition {
                key: cli::commands::FIELD_LANGUAGE.to_string(),
                r#match: Some(Match {
                    match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(lang)),
                }),
                range: None,
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
                geo_polygon: None,
                datetime_range: None,
            };
            
            use qdrant_client::qdrant::Condition;
            use qdrant_client::qdrant::condition::ConditionOneOf;
            
            filter_conditions.push(Condition {
                condition_one_of: Some(ConditionOneOf::Field(lang_condition)),
            });
        }
        
        // Add element type filter if provided
        if let Some(elem_type) = element_type_filter {
            use qdrant_client::qdrant::FieldCondition;
            use qdrant_client::qdrant::Match;
            
            let type_condition = FieldCondition {
                key: cli::commands::FIELD_ELEMENT_TYPE.to_string(),
                r#match: Some(Match {
                    match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(elem_type)),
                }),
                range: None,
                geo_bounding_box: None,
                geo_radius: None,
                values_count: None,
                geo_polygon: None,
                datetime_range: None,
            };
            
            use qdrant_client::qdrant::Condition;
            use qdrant_client::qdrant::condition::ConditionOneOf;
            
            filter_conditions.push(Condition {
                condition_one_of: Some(ConditionOneOf::Field(type_condition)),
            });
        }
        
        // Create a filter if we have conditions
        let filter = if !filter_conditions.is_empty() {
            use qdrant_client::qdrant::Filter;
            Some(Filter {
                should: Vec::new(),
                must: filter_conditions,
                must_not: Vec::new(),
                min_should: None,
            })
        } else {
            None
        };
        
        // Build the search request
        let search_request = SearchPointsBuilder::new(collection_name, embedding, limit)
            .with_payload(true);
            
        // Add filter if present
        let search_request = if let Some(f) = filter {
            search_request.filter(f)
        } else {
            search_request
        };
        
        // Perform search
        let search_result = self.client.search_points(search_request).await;
        
        // Calculate query time
        let query_time = start.elapsed();
        
        match search_result {
            Ok(search_response) => {
                // Convert points to SearchResults
                let results: Vec<SearchResult> = search_response.result
                    .into_iter()
                    .map(|point| {
                        let payload = point.payload;
                        
                        // Extract fields from payload
                        let file_path = get_string_from_payload(&payload, cli::commands::FIELD_FILE_PATH).unwrap_or_default();
                        let start_line = get_integer_from_payload(&payload, cli::commands::FIELD_START_LINE).unwrap_or(0) as i32;
                        let end_line = get_integer_from_payload(&payload, cli::commands::FIELD_END_LINE).unwrap_or(0) as i32;
                        let language = get_string_from_payload(&payload, cli::commands::FIELD_LANGUAGE).unwrap_or_default();
                        let element_type = get_string_from_payload(&payload, cli::commands::FIELD_ELEMENT_TYPE).unwrap_or_default();
                        let content = get_string_from_payload(&payload, cli::commands::FIELD_CHUNK_CONTENT).unwrap_or_default();
                        let branch = get_string_from_payload(&payload, cli::commands::FIELD_BRANCH);
                        let commit_hash = get_string_from_payload(&payload, cli::commands::FIELD_COMMIT_HASH);
                        
                        SearchResult {
                            file_path,
                            start_line,
                            end_line,
                            language,
                            element_type,
                            content,
                            score: point.score,
                            branch,
                            commit_hash,
                        }
                    })
                    .collect();
                
                Ok(Response::new(QueryResponse {
                    total_results: results.len() as i32,
                    query_time_ms: query_time.as_millis() as f32,
                    results,
                }))
            },
            Err(e) => {
                Err(Status::internal(format!("Search query failed: {}", e)))
            }
        }
    }
    
    async fn add_repository(
        &self,
        request: Request<AddRepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let req = request.into_inner();
        
        // Clone the values we need
        let name_opt = req.name.clone();
        let url = req.url.clone();
        
        // Use the CLI's repo_commands::add_repository function
        let result = {
            use std::path::PathBuf;
            use crate::cli::repo_commands::{add_repository, add::AddRepoArgs};
            
            let local_path = req.local_path.map(PathBuf::from);
            let branch = req.branch;
            let remote = req.remote.unwrap_or_else(|| "origin".to_string());
            let ssh_key_path = req.ssh_key_path.map(PathBuf::from);
            let ssh_passphrase = req.ssh_passphrase;
            
            // Create a mutable copy of the config
            let mut config = self.config.as_ref().clone();
            
            // Create AddRepoArgs
            let args = AddRepoArgs {
                url: url.clone(),
                name: name_opt.clone(),
                local_path: local_path.clone(),
                branch: branch.clone(),
                remote: Some(remote.clone()),
                ssh_key: ssh_key_path.clone(),
                ssh_passphrase: ssh_passphrase.clone(),
            };
            
            // Call the add_repository function
            add_repository(
                args,
                &mut config,
                self.client.clone(),
                None,
            ).await
        };
        
        match result {
            Ok(()) => {
                // Get the repository name from the request
                let repo_name = name_opt.unwrap_or_else(|| {
                    // Extract repo name from URL
                    let url_str = url.trim_end_matches(".git");
                    url_str.split('/').last().unwrap_or("unknown").to_string()
                });
            
                Ok(Response::new(StatusResponse {
                    success: true,
                    message: format!("Repository '{}' added successfully", repo_name),
                }))
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to add repository: {}", e)))
            }
        }
    }
    
    async fn list_repositories(
        &self,
        request: Request<Empty>,
    ) -> std::result::Result<Response<ListRepositoriesResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Use the CLI's repo_commands::list_repositories function to get repositories
        let result = {
            use crate::cli::repo_commands::get_managed_repos;
            
            // Create a copy of the config
            let config = self.config.as_ref().clone();
            
            get_managed_repos(&config)
        };
        
        match result {
            Ok(managed_repos) => {
                let active_repo = managed_repos.active_repository.clone();
                
                // Convert to gRPC repository info
                let repositories = managed_repos.repositories.into_iter()
                    .map(|repo| {
                        // Determine if this is the active repo
                        let is_active = active_repo.as_ref()
                            .map(|active| active == &repo.name)
                            .unwrap_or(false);
                            
                        // Convert tracked branches to strings
                        let tracked_branches = repo.tracked_branches.into_iter().collect();
                            
                        // Get indexed languages (we'll return empty for now as it's
                        // not stored in the config)
                        let indexed_languages = Vec::new();
                        
                        RepositoryInfo {
                            name: repo.name,
                            url: repo.url,
                            local_path: repo.local_path.to_string_lossy().to_string(),
                            default_branch: repo.default_branch.clone(),
                            active_branch: repo.active_branch.clone().unwrap_or_default(),
                            tracked_branches,
                            indexed_languages,
                            is_active,
                        }
                    })
                    .collect();
                    
                Ok(Response::new(ListRepositoriesResponse {
                    repositories,
                    active_repository: active_repo,
                }))
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to list repositories: {}", e)))
            }
        }
    }
    
    async fn use_repository(
        &self,
        request: Request<RepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let repo_name = request.into_inner().name;
        
        // Use the CLI's repo_commands::use_repository function
        let result = {
            use crate::cli::repo_commands::{set_active_repo, r#use::UseRepoArgs};
            
            // Create a mutable copy of the config
            let mut config = self.config.as_ref().clone();
            
            // Create UseRepoArgs
            let args = UseRepoArgs {
                name: repo_name.clone(),
            };
            
            // Call the set_active_repo function
            set_active_repo(args, &mut config, None)
        };
        
        match result {
            Ok(_) => {
                Ok(Response::new(StatusResponse {
                    success: true,
                    message: format!("Repository '{}' is now active", repo_name),
                }))
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to set active repository: {}", e)))
            }
        }
    }
    
    async fn remove_repository(
        &self,
        request: Request<RemoveRepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let req = request.into_inner();
        let repo_name = req.name;
        let skip_confirmation = req.skip_confirmation;
        
        // Use the CLI's repo_commands::remove_repository function
        let result = {
            use crate::cli::repo_commands::{remove_repository, remove::RemoveRepoArgs};
            
            // Create a mutable copy of the config
            let mut config = self.config.as_ref().clone();
            
            // Create RemoveRepoArgs
            let args = RemoveRepoArgs {
                name: repo_name.clone(),
                yes: skip_confirmation,
            };
            
            // Create a client for Qdrant
            let client = self.client.clone();
            
            // Call the remove_repository function
            remove_repository(args, &mut config, client, None).await
        };
        
        match result {
            Ok(_) => {
                Ok(Response::new(StatusResponse {
                    success: true,
                    message: format!("Repository '{}' removed successfully", repo_name),
                }))
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to remove repository: {}", e)))
            }
        }
    }
    
    async fn sync_repository(
        &self,
        request: Request<SyncRepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let req = request.into_inner();
        
        // Clone the name to avoid issues with moved values
        let name_opt = req.name.clone();
        let _extensions = req.extensions;
        let _force = req.force;
        
        // Create a simple message-only response for now
        // Due to Send issues with the git2 library in async contexts,
        // we'll have to implement a more sophisticated solution later
        // that properly isolates the non-Send git operations
        
        let repo_name = name_opt.unwrap_or_else(|| {
            self.config.active_repository.clone().unwrap_or_else(|| "unknown".to_string())
        });
        
        // Create a response indicating we need a better implementation
        Ok(Response::new(StatusResponse {
            success: true,
            message: format!(
                "Repository '{}' sync request received. Please use the CLI for now as server-side git operations need to be refactored.",
                repo_name
            ),
        }))
    }
    
    async fn use_branch(
        &self,
        request: Request<UseBranchRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        // Authenticate the request
        self.authenticate(&request)?;
        
        // Extract request data
        let req = request.into_inner();
        let branch_name = req.branch_name;
        let repository_name = req.repository_name;
        
        // Use the CLI's repo_commands::use_branch function
        let result = {
            use crate::cli::repo_commands::{use_branch, use_branch::UseBranchArgs};
            
            // Create a mutable copy of the config
            let mut config = self.config.as_ref().clone();
            
            // Create UseBranchArgs
            let args = UseBranchArgs {
                name: branch_name.clone(),
            };
            
            // Call the use_branch function
            use_branch(args, &mut config, None).await
        };
        
        match result {
            Ok(()) => {
                // Extract repository name (use provided or active)
                let repo_name = repository_name.unwrap_or_else(|| {
                    self.config.active_repository.clone().unwrap_or_else(|| "unknown".to_string())
                });
                
                Ok(Response::new(StatusResponse {
                    success: true,
                    message: format!("Switched to branch '{}' in repository '{}'", branch_name, repo_name),
                }))
            }
            Err(e) => {
                Err(Status::internal(format!("Failed to switch branch: {}", e)))
            }
        }
    }
}

// Helper functions for extracting data from payload
#[cfg(feature = "server")]
fn get_string_from_payload(
    payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>,
    key: &str,
) -> Option<String> {
    payload.get(key).and_then(|value| {
        if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &value.kind {
            Some(s.clone())
        } else {
            None
        }
    })
}

#[cfg(feature = "server")]
fn get_integer_from_payload(
    payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>,
    key: &str,
) -> Option<i64> {
    payload.get(key).and_then(|value| {
        if let Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) = &value.kind {
            Some(*i)
        } else {
            None
        }
    })
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use tonic::Request;
    use std::sync::Arc;
    use crate::config::AppConfig;
    use qdrant_client::Qdrant;
    use vectordb_proto::vectordb::{Empty, CreateCollectionRequest, CollectionRequest};
    
    // Helper function to create a test service
    fn create_test_service() -> VectorDBServiceImpl {
        let config = Arc::new(AppConfig::default());
        let client = Arc::new(
            Qdrant::from_url("http://localhost:6334")
                .build()
                .expect("Failed to create Qdrant client")
        );
        
        VectorDBServiceImpl::new(config, client)
    }
    
    #[tokio::test]
    async fn test_get_server_info() {
        let service = create_test_service();
        let request = Request::new(Empty {});
        
        let response = service.get_server_info(request).await;
        assert!(response.is_ok(), "Failed to get server info: {:?}", response.err());
        
        let server_info = response.unwrap().into_inner();
        assert_eq!(server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(server_info.is_healthy);
        assert!(server_info.model_info.is_some());
    }
    
    #[tokio::test]
    async fn test_collection_management() {
        let service = create_test_service();
        
        // Test collection name
        let collection_name = format!("test_collection_{}", fastrand::u64(..));
        
        // Create a collection
        let create_request = Request::new(CreateCollectionRequest {
            name: collection_name.clone(),
            vector_size: 384,
            distance: "cosine".to_string(),
        });
        
        let create_response = service.create_collection(create_request).await;
        assert!(create_response.is_ok(), "Failed to create collection: {:?}", create_response.err());
        let create_result = create_response.unwrap().into_inner();
        assert!(create_result.success, "Create collection failed: {}", create_result.message);
        
        // List collections
        let list_request = Request::new(Empty {});
        let list_response = service.list_collections(list_request).await;
        assert!(list_response.is_ok(), "Failed to list collections: {:?}", list_response.err());
        let collections = list_response.unwrap().into_inner().collections;
        assert!(collections.contains(&collection_name), "Created collection not found in list");
        
        // Delete the collection
        let delete_request = Request::new(CollectionRequest {
            name: collection_name.clone(),
        });
        
        let delete_response = service.delete_collection(delete_request).await;
        assert!(delete_response.is_ok(), "Failed to delete collection: {:?}", delete_response.err());
        let delete_result = delete_response.unwrap().into_inner();
        assert!(delete_result.success, "Delete collection failed: {}", delete_result.message);
        
        // Verify it's gone
        let list_request = Request::new(Empty {});
        let list_response = service.list_collections(list_request).await;
        assert!(list_response.is_ok(), "Failed to list collections after delete: {:?}", list_response.err());
        let collections_after = list_response.unwrap().into_inner().collections;
        assert!(!collections_after.contains(&collection_name), "Collection still exists after deletion");
    }
    
    // Add test for repository APIs
    #[tokio::test]
    async fn test_repository_management() {
        use std::fs;
        use std::path::Path;
        use tempfile::tempdir;
        
        // Create a temporary directory for testing
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        
        // Create a test repo in the temp directory
        let repo_path = temp_path.join("test_repo");
        fs::create_dir_all(&repo_path).expect("Failed to create test repo dir");
        
        // Initialize an empty git repo
        let repo = git2::Repository::init(&repo_path).expect("Failed to init git repo");
        
        // Create a test file
        let file_path = repo_path.join("test.rs");
        fs::write(&file_path, "fn test() {}\n").expect("Failed to write test file");
        
        // Commit the file
        let mut index = repo.index().expect("Failed to get index");
        index.add_path(Path::new("test.rs")).expect("Failed to add path");
        index.write().expect("Failed to write index");
        
        let oid = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(oid).expect("Failed to find tree");
        
        let signature = git2::Signature::now("Test", "test@example.com").expect("Failed to create signature");
        repo.commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])
            .expect("Failed to commit");
            
        // Create a test branch
        let head = repo.head().expect("Failed to get HEAD");
        let commit = repo.find_commit(head.target().unwrap()).expect("Failed to find commit");
        repo.branch("test-branch", &commit, false).expect("Failed to create branch");
        
        // Now test the repository management APIs
        let service = create_test_service();
        
        // Test add_repository (we'll use the path where we just created the repo)
        let add_request = Request::new(AddRepositoryRequest {
            url: format!("file://{}", repo_path.to_string_lossy()),
            local_path: Some(repo_path.to_string_lossy().to_string()),
            name: Some("test_repo".to_string()),
            branch: Some("main".to_string()),
            remote: Some("origin".to_string()),
            ssh_key_path: None,
            ssh_passphrase: None,
        });
        
        // This test will be more of an integration test that requires actual Git operations,
        // so we'll keep it commented but include it for documentation purposes
        /*
        let add_response = service.add_repository(add_request).await;
        assert!(add_response.is_ok(), "Failed to add repository: {:?}", add_response.err());
        let add_result = add_response.unwrap().into_inner();
        assert!(add_result.success, "Add repository failed: {}", add_result.message);
        
        // Test list_repositories
        let list_request = Request::new(Empty {});
        let list_response = service.list_repositories(list_request).await;
        assert!(list_response.is_ok(), "Failed to list repositories: {:?}", list_response.err());
        let repositories = list_response.unwrap().into_inner().repositories;
        assert!(repositories.iter().any(|r| r.name == "test_repo"), "Added repository not found in list");
        
        // Test use_repository
        let use_request = Request::new(RepositoryRequest {
            name: "test_repo".to_string(),
        });
        
        let use_response = service.use_repository(use_request).await;
        assert!(use_response.is_ok(), "Failed to use repository: {:?}", use_response.err());
        let use_result = use_response.unwrap().into_inner();
        assert!(use_result.success, "Use repository failed: {}", use_result.message);
        
        // Test use_branch
        let branch_request = Request::new(UseBranchRequest {
            branch_name: "test-branch".to_string(),
            repository_name: Some("test_repo".to_string()),
        });
        
        let branch_response = service.use_branch(branch_request).await;
        assert!(branch_response.is_ok(), "Failed to use branch: {:?}", branch_response.err());
        let branch_result = branch_response.unwrap().into_inner();
        assert!(branch_result.success, "Use branch failed: {}", branch_result.message);
        
        // Test sync_repository
        let sync_request = Request::new(SyncRepositoryRequest {
            name: Some("test_repo".to_string()),
            extensions: vec!["rs".to_string()],
            force: false,
        });
        
        let sync_response = service.sync_repository(sync_request).await;
        assert!(sync_response.is_ok(), "Failed to sync repository: {:?}", sync_response.err());
        let sync_result = sync_response.unwrap().into_inner();
        assert!(sync_result.success, "Sync repository failed: {}", sync_result.message);
        
        // Test remove_repository
        let remove_request = Request::new(RemoveRepositoryRequest {
            name: "test_repo".to_string(),
            skip_confirmation: true,
        });
        
        let remove_response = service.remove_repository(remove_request).await;
        assert!(remove_response.is_ok(), "Failed to remove repository: {:?}", remove_response.err());
        let remove_result = remove_response.unwrap().into_inner();
        assert!(remove_result.success, "Remove repository failed: {}", remove_result.message);
        
        // Verify it's gone
        let list_request = Request::new(Empty {});
        let list_response = service.list_repositories(list_request).await;
        assert!(list_response.is_ok(), "Failed to list repositories after removal: {:?}", list_response.err());
        let repositories_after = list_response.unwrap().into_inner().repositories;
        assert!(!repositories_after.iter().any(|r| r.name == "test_repo"), "Repository still exists after removal");
        */
    }
} 