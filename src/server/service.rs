#[cfg(feature = "server")]
use std::sync::Arc;
#[cfg(feature = "server")]
use tonic::{Request, Response, Status};
#[cfg(feature = "server")]
use qdrant_client::Qdrant;
#[cfg(feature = "server")]
use crate::config::{AppConfig, save_config, get_repo_base_path};
#[cfg(feature = "server")]
use crate::server::auth::{ApiKeyAuthenticator};
#[cfg(feature = "server")]
use qdrant_client::qdrant::{Distance, CreateCollectionBuilder, VectorParamsBuilder, SearchPointsBuilder};
#[cfg(feature = "server")]
use crate::cli::repo_commands::helpers;
#[cfg(feature = "server")]
use anyhow::{anyhow, Context};
#[cfg(feature = "server")]
use std::path::{PathBuf};
#[cfg(feature = "server")]
use std::fs;
#[cfg(feature = "server")]
use arc_swap::ArcSwap;
#[cfg(feature = "server")]
use tracing::{error, info, warn};
#[cfg(feature = "server")]
use crate::vectordb::embedding_logic::EmbeddingHandler;
#[cfg(feature = "server")]
use std::time::Instant;
#[cfg(feature = "server")]
use vectordb_proto::vectordb::*;
#[cfg(feature = "server")]
use vectordb_proto::vector_db_service_server::VectorDbService;
#[cfg(feature = "server")]
use std::fmt;
#[cfg(feature = "server")]
use qdrant_client::qdrant::{Match};

#[cfg(feature = "server")]
pub struct VectorDBServiceImpl {
    client: Arc<Qdrant>,
    config: Arc<ArcSwap<AppConfig>>,
    authenticator: Option<ApiKeyAuthenticator>,
}

impl fmt::Debug for VectorDBServiceImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VectorDBServiceImpl")
            .field("config", &self.config.load())
            .field("client", &"<Arc<Qdrant> client>")
            .field("authenticator", &self.authenticator)
            .finish()
    }
}

#[cfg(feature = "server")]
impl VectorDBServiceImpl {
    pub fn new(client: Arc<Qdrant>, initial_config: Arc<AppConfig>, api_key: Option<String>) -> Result<Self, anyhow::Error> {
        let authenticator = match api_key {
             Some(key_path_str) => {
                 let key_path = PathBuf::from(key_path_str);
                 Some(ApiKeyAuthenticator::new(Some(&key_path), true)?)
             }
             None => None,
        };

        Ok(Self {
            client,
            config: Arc::new(ArcSwap::from(initial_config)),
            authenticator,
        })
    }

    fn authenticate<T>(&self, request: &Request<T>) -> Result<(), Status> {
        if let Some(auth) = &self.authenticator {
            auth.authenticate(request).map_err(|e| Status::unauthenticated(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(feature = "server")]
#[tonic::async_trait]
impl VectorDbService for VectorDBServiceImpl {
    async fn get_server_info(
        &self,
        request: Request<Empty>,
    ) -> std::result::Result<Response<ServerInfo>, Status> {
        self.authenticate(&request)?;
        let config = self.config.load();
        let build_date = env!("CARGO_PKG_VERSION").to_string();

        let active_repo_name = config.active_repository.clone();
        let _active_repo_info = active_repo_name.as_ref().and_then(|name| {
            config.repositories.iter().find(|r| &r.name == name)
        }).map(|r| RepositoryInfo {
             name: r.name.clone(),
             url: r.url.clone(),
             local_path: r.local_path.to_string_lossy().into_owned(),
             default_branch: r.default_branch.clone(),
             active_branch: r.active_branch.clone().unwrap_or_default(),
             tracked_branches: r.tracked_branches.clone(),
             indexed_languages: r.indexed_languages.clone().unwrap_or_default(),
             is_active: true,
        });

        Ok(Response::new(ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_date,
            is_healthy: true,
            model_info: None,
        }))
    }
    
    async fn create_collection(
        &self,
        request: Request<CreateCollectionRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        self.authenticate(&request)?;
        let req = request.into_inner();
        let collection_name = req.name;
        let vector_size = req.vector_size as u64;

        let distance = match req.distance.as_str() {
            "cosine" => Distance::Cosine,
            "euclid" => Distance::Euclid,
            "dot" => Distance::Dot,
            _ => return Err(Status::invalid_argument(format!("Invalid distance metric: {}", req.distance))),
        };

        let create_request = CreateCollectionBuilder::new(&collection_name)
            .vectors_config(VectorParamsBuilder::new(vector_size, distance));

        self.client.create_collection(create_request.build()).await
            .map(|_| Response::new(StatusResponse {
                success: true,
                message: format!("Collection '{}' created successfully", collection_name),
            }))
            .map_err(|e| {
                if e.to_string().contains("already exists") {
                    Status::already_exists(format!("Collection '{}' already exists", collection_name))
                } else {
                    error!("Failed to create collection '{}': {}", collection_name, e);
                    Status::internal(format!("Failed to create collection: {}", e))
                }
            })
    }
    
    async fn list_collections(
        &self,
        request: Request<Empty>,
    ) -> std::result::Result<Response<ListCollectionsResponse>, Status> {
        self.authenticate(&request)?;
        let collections_response = self.client.list_collections().await
            .map_err(|e| {
                error!("Failed to list collections: {}", e);
                Status::internal(format!("Failed to list collections: {}", e))
            })?;
        Ok(Response::new(ListCollectionsResponse {
            collections: collections_response.collections.into_iter().map(|c| c.name).collect(),
        }))
    }
    
    async fn clear_collection(
        &self,
        _request: Request<CollectionRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
         self.authenticate(&_request)?;
         warn!("clear_collection RPC endpoint is not fully implemented yet.");
         Ok(Response::new(StatusResponse {
             success: false,
             message: "clear_collection not implemented".to_string(),
         }))
    }
    
    async fn delete_collection(
        &self,
        request: Request<CollectionRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        self.authenticate(&request)?;
        let collection_name = request.into_inner().name;

        self.client.delete_collection(collection_name.clone()).await
            .map(|_| Response::new(StatusResponse {
                success: true,
                message: format!("Collection '{}' deleted successfully", collection_name),
            }))
            .map_err(|e| {
                if e.to_string().contains("not found") || e.to_string().contains("doesn't exist") {
                   Status::not_found(format!("Collection '{}' does not exist", collection_name))
                } else {
                    error!("Failed to delete collection '{}': {}", collection_name, e);
                    Status::internal(format!("Failed to delete collection: {}", e))
                }
            })
    }
    
    async fn index_files(
        &self,
        request: Request<IndexFilesRequest>,
    ) -> std::result::Result<Response<IndexResponse>, Status> {
        self.authenticate(&request)?;
        let req = request.into_inner();
        let config_arc = self.config.load();
        let collection_name = req.collection_name;
        let paths = req.paths;
        let file_paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();

        let repo_name = config_arc.active_repository.as_ref()
                .ok_or_else(|| Status::failed_precondition("No active repository set for index_files request"))?.clone();

        let repo_config = config_arc.repositories.iter().find(|r| r.name == repo_name)
            .ok_or_else(|| Status::not_found(format!("Active repository '{}' config not found", repo_name)))?.clone();

        let active_branch = repo_config.active_branch.as_deref().unwrap_or(&repo_config.default_branch);
        let commit_hash = "HEAD";

        let index_start_time = Instant::now();
        let mut errors = Vec::new();

        let result = crate::cli::repo_commands::helpers::index_files(
            self.client.as_ref(),
            &crate::cli::commands::CliArgs::default(),
            config_arc.as_ref(),
            &repo_config.local_path,
            &file_paths,
            &collection_name,
            active_branch,
            commit_hash,
        ).await;

        let indexing_duration = index_start_time.elapsed();
        if result.is_err() {
             errors.push(format!("{}", result.err().unwrap()));
        }

        info!(
            "Indexing completed for collection '{}': requested {} files in {:.2?}. Errors: {}",
            collection_name, file_paths.len(), indexing_duration, errors.len()
        );

        Ok(Response::new(IndexResponse {
            success: errors.is_empty(),
            message: if errors.is_empty() {
                format!("Successfully processed indexing request for {} files in {:.2?}",
                       file_paths.len(), indexing_duration)
            } else {
                format!("Indexing request failed: {}", errors.join("; "))
            },
            indexed_files: if errors.is_empty() { file_paths.len() as i32 } else { 0 },
            indexed_chunks: 0,
        }))
    }
    
    async fn query_collection(
        &self,
        request: Request<QueryRequest>,
    ) -> std::result::Result<Response<QueryResponse>, Status> {
        self.authenticate(&request)?;
        let req = request.into_inner();
        let config = self.config.load();
        let collection_name = req.collection_name;
        let query_text = req.query_text;
        let limit = if req.limit > 0 { req.limit as u64 } else { 10 };

        let embedding_handler = EmbeddingHandler::new(config.as_ref())
            .map_err(|e| Status::internal(format!("Failed to create embedding handler: {}", e)))?;

        let query_embedding = embedding_handler.embed(&[&query_text])
            .map_err(|e| Status::internal(format!("Failed to generate query embedding: {}", e)))?
            .into_iter().next()
            .ok_or_else(|| Status::internal("Embedding generation yielded no result"))?;

        let mut filter_conditions = Vec::new();
        use qdrant_client::qdrant::{Condition, Filter, condition::ConditionOneOf, r#match::MatchValue, FieldCondition};

        if let Some(lang) = req.language {
            filter_conditions.push(Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "language".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Keyword(lang)),
                    }),
                    range: None,
                    geo_bounding_box: None,
                    geo_radius: None,
                    values_count: None,
                    geo_polygon: None,
                    datetime_range: None,
                })),
            });
        }
        if let Some(elem_type) = req.element_type {
            filter_conditions.push(Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "element_type".to_string(),
                    r#match: Some(Match {
                        match_value: Some(MatchValue::Keyword(elem_type)),
                    }),
                    range: None,
                    geo_bounding_box: None,
                    geo_radius: None,
                    values_count: None,
                    geo_polygon: None,
                    datetime_range: None,
                })),
            });
        }

        let search_filter = Filter {
             must: filter_conditions,
             should: vec![],
             must_not: vec![],
             min_should: None,
        };

        let search_request_builder = SearchPointsBuilder::new(collection_name.clone(), query_embedding, limit)
            .with_payload(true);

        let search_request = search_request_builder;
        
        let search_request = if !search_filter.must.is_empty() { 
             search_request.filter(search_filter) 
        } else { 
             search_request 
        };

        let search_start_time = Instant::now();
        let search_response = self.client.search_points(search_request).await
            .map_err(|e| {
                error!("Search points failed in collection '{}': {}", collection_name, e);
                Status::internal("Failed to execute search query")
            })?;
        let search_duration = search_start_time.elapsed();
        info!("Qdrant search completed in {:.2?}", search_duration);

        let result_count = search_response.result.len();
        let proto_results: Vec<vectordb_proto::vectordb::SearchResult> = search_response.result.into_iter().map(|scored_point| {
            let payload = scored_point.payload;
            vectordb_proto::vectordb::SearchResult {
                score: scored_point.score,
                file_path: get_string_from_payload(&payload, "file_path").unwrap_or_default(),
                start_line: get_integer_from_payload(&payload, "start_line").unwrap_or(0) as i32,
                end_line: get_integer_from_payload(&payload, "end_line").unwrap_or(0) as i32,
                content: get_string_from_payload(&payload, "content").unwrap_or_default(),
                language: get_string_from_payload(&payload, "language").unwrap_or_default(),
                branch: Some(get_string_from_payload(&payload, "branch").unwrap_or_default()),
                commit_hash: Some(get_string_from_payload(&payload, "commit_hash").unwrap_or_default()),
                element_type: get_string_from_payload(&payload, "element_type").unwrap_or_default(),
            }
        }).collect();

        Ok(Response::new(QueryResponse { 
            results: proto_results, 
            total_results: result_count as i32, 
            query_time_ms: search_duration.as_millis() as f32 
        }))
    }
    
    async fn add_repository(
        &self,
        request: Request<AddRepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        self.authenticate(&request)?;
        let req = request.into_inner();
        let url = req.url;
        let name_opt = req.name;
        let local_path = req.local_path;

        // At least one of URL or local_path must be provided
        if url.is_empty() && local_path.is_none() {
            return Err(Status::invalid_argument("Either URL or local path must be provided"));
        }

        // If local_path is provided but doesn't exist and URL is empty, return error
        if url.is_empty() && local_path.is_some() {
            let path = PathBuf::from(local_path.as_ref().unwrap());
            if !path.exists() {
                return Err(Status::invalid_argument(
                    "Local path doesn't exist and no URL provided to clone from"
                ));
            }
        }

        let result = async {
            let current_config_arc = self.config.load();
            let mut config = (**current_config_arc).clone();

            // Determine repository name
            let repo_name_str = match name_opt.as_deref() {
                Some(name) => name.to_string(),
                None => {
                    if !url.is_empty() {
                        // If URL is provided, derive name from URL
                        PathBuf::from(&url).file_stem().and_then(|s| s.to_str())
                            .map(|s| s.trim_end_matches(".git").to_string())
                            .ok_or_else(|| anyhow!("Could not derive repository name from URL"))?
                    } else {
                        // If URL is not provided, derive name from local path directory name
                        let path = PathBuf::from(local_path.as_ref().unwrap());
                        path.file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                            .ok_or_else(|| anyhow!("Could not derive repository name from local path"))?
                    }
                }
            };

            if config.repositories.iter().any(|r| r.name == repo_name_str) {
                return Err(anyhow!("Repository '{}' already exists.", repo_name_str));
            }

            let repo_base_path = get_repo_base_path(Some(&config))?;
            fs::create_dir_all(&repo_base_path).context("Failed to create base repo dir")?;
            let embedding_dim = helpers::DEFAULT_VECTOR_DIMENSION;
            
            let new_repo_config = helpers::prepare_repository(
                &url, Some(&repo_name_str),
                local_path.as_ref().map(PathBuf::from).as_ref(),
                req.branch.as_deref(), req.remote.as_deref(),
                req.ssh_key_path.as_ref().map(PathBuf::from).as_ref(),
                req.ssh_passphrase.as_deref(),
                &repo_base_path, Arc::clone(&self.client), embedding_dim
            ).await?;
            
            let final_repo_name = new_repo_config.name.clone();
            config.repositories.push(new_repo_config);
            config.active_repository = Some(final_repo_name.clone());
            save_config(&config, None).context("Failed to save config")?;
            self.config.store(Arc::new(config));
            info!("Added repository '{}' and set as active.", final_repo_name);
            Ok::<_, anyhow::Error>(final_repo_name)
        }.await;

        match result {
            Ok(repo_name) => Ok(Response::new(StatusResponse {
                success: true, message: format!("Repository '{}' added successfully", repo_name),
            })),
            Err(e) => {
                if e.to_string().contains("already exists") {
                     Err(Status::already_exists(e.to_string()))
                } else {
                     error!("Failed to add repository: {}", e);
                     Err(Status::internal(format!("Failed to add repository: {}", e)))
                }
            }
        }
    }
    
    async fn list_repositories(
        &self,
        request: Request<Empty>,
    ) -> std::result::Result<Response<ListRepositoriesResponse>, Status> {
        self.authenticate(&request)?;
        let config = self.config.load();
        let active_repo_name = config.active_repository.clone();

        let repos = config.repositories.iter().map(|r| RepositoryInfo {
             name: r.name.clone(),
             url: r.url.clone(),
             local_path: r.local_path.to_string_lossy().into_owned(),
             default_branch: r.default_branch.clone(),
             active_branch: r.active_branch.clone().unwrap_or_default(),
             tracked_branches: r.tracked_branches.clone(),
             indexed_languages: r.indexed_languages.clone().unwrap_or_default(),
             is_active: active_repo_name.as_ref() == Some(&r.name),
        }).collect();

        Ok(Response::new(ListRepositoriesResponse {
            repositories: repos,
            active_repository: active_repo_name,
        }))
    }
    
    async fn use_repository(
        &self,
        request: Request<RepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        self.authenticate(&request)?;
        let repo_name = request.into_inner().name;
        if repo_name.is_empty() {
            return Err(Status::invalid_argument("Repository name cannot be empty"));
        }

        let result: std::result::Result<_, anyhow::Error> = {
            let current_config_arc = self.config.load();
            let mut config = (**current_config_arc).clone();
            if config.repositories.iter().any(|r| r.name == repo_name) {
                config.active_repository = Some(repo_name.clone());
                let save_result = save_config(&config, None).context("Failed to save config");
                match save_result {
                    Ok(_) => {
                        self.config.store(Arc::new(config));
                        info!("Set active repository to '{}'", repo_name);
                        Ok(())
                    }
                    Err(e) => {
                        Err(e)
                    }
                }
            } else {
                Err(anyhow!("Repository '{}' not found.", repo_name))
            }
        };

        match result {
            Ok(_) => Ok(Response::new(StatusResponse {
                success: true, message: format!("Repository '{}' is now active", repo_name),
            })),
            Err(e) => {
                 if e.to_string().contains("not found") {
                     Err(Status::not_found(format!("Repository '{}' not found", repo_name)))
                 } else {
                     error!("Failed to set active repository to '{}': {}", repo_name, e);
                     Err(Status::internal(format!("Failed to set active repository: {}", e)))
                 }
            }
        }
    }
    
    async fn remove_repository(
        &self,
        request: Request<RemoveRepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        self.authenticate(&request)?;
        let req = request.into_inner();
        let repo_name = req.name;
        if repo_name.is_empty() {
            return Err(Status::invalid_argument("Repository name cannot be empty"));
        }

        let result = async {
            let current_config_arc = self.config.load();
            let mut config = (**current_config_arc).clone();
            let repo_config_index = config.repositories.iter().position(|r| r.name == repo_name)
                .ok_or_else(|| anyhow!("Repository '{}' not found.", repo_name))?;
            let repo_config_to_delete = config.repositories[repo_config_index].clone();
            helpers::delete_repository_data(&repo_config_to_delete, Arc::clone(&self.client)).await?;
            config.repositories.remove(repo_config_index);
            if config.active_repository.as_deref() == Some(&repo_name) {
                config.active_repository = config.repositories.first().map(|r| r.name.clone());
                 info!("Reset active repository after removing '{}'", repo_name);
            }
            save_config(&config, None).context("Failed to save config")?;
            self.config.store(Arc::new(config));
            info!("Removed repository '{}'", repo_name);
            Ok::<_, anyhow::Error>(())
        }.await;

        match result {
            Ok(_) => Ok(Response::new(StatusResponse {
                success: true, message: format!("Repository '{}' removed successfully", repo_name),
            })),
             Err(e) => {
                 if e.to_string().contains("not found") {
                     Err(Status::not_found(format!("Repository '{}' not found", repo_name)))
                 } else {
                    error!("Failed to remove repository '{}': {}", repo_name, e);
                    Err(Status::internal(format!("Failed to remove repository: {}", e)))
                 }
            }
        }
    }
    
    async fn sync_repository(
        &self,
        request: Request<SyncRepositoryRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        self.authenticate(&request)?;
        let req = request.into_inner();
        let config_arc = self.config.load();

        let repo_name = match req.name {
            Some(name) => name,
            None => config_arc.active_repository.as_ref()
                .ok_or_else(|| Status::failed_precondition("No repository name provided and no active repository set"))?.clone(),
        };
        let repo_config = config_arc.repositories.iter().find(|r| r.name == repo_name)
            .ok_or_else(|| Status::not_found(format!("Repository '{}' not found", repo_name)))?.clone();

        let options = crate::git::SyncOptions {
            force: req.force,
            extensions: if req.extensions.is_empty() { None } else { Some(req.extensions) }
        };
        info!("Starting sync for repository '{}' with options: {:?}", repo_name, options);

        let sync_result_res = crate::git::sync_repository(
            Arc::clone(&self.client),
            repo_config.clone(), options,
            &crate::cli::commands::CliArgs::default(),
            &config_arc,
        ).await;

        let final_result = async {
            let sync_result = sync_result_res.map_err(|e|
                anyhow!("Failed to sync repository '{}': {}", repo_name, e)
            )?;
            if sync_result.success && !sync_result.indexed_languages.is_empty() {
                let current_config_arc_post_sync = self.config.load();
                let mut config_to_update = (**current_config_arc_post_sync).clone();
                if let Some(idx) = config_to_update.repositories.iter().position(|r| r.name == repo_name) {
                    info!(
                        "Updating indexed languages for repo '{}' to: {:?}",
                        repo_name, &sync_result.indexed_languages
                    );
                    config_to_update.repositories[idx].indexed_languages = Some(sync_result.indexed_languages.clone());
                    save_config(&config_to_update, None).context("Failed to save config after sync")?;
                    self.config.store(Arc::new(config_to_update));
                } else {
                     warn!("Repository '{}' not found in config after successful sync, cannot update indexed languages.", repo_name);
                }
            }
            Ok::<_, anyhow::Error>(sync_result)
        }.await;

        match final_result {
            Ok(result) => {
                info!("Sync successful for repository '{}': {}", repo_name, result.message);
                Ok(Response::new(StatusResponse {
                     success: result.success, message: result.message,
                }))
            },
            Err(e) => {
                error!("Error during sync or config update for '{}': {}", repo_name, e);
                Err(Status::internal(e.to_string()))
            }
        }
    }
    
    async fn use_branch(
        &self,
        request: Request<UseBranchRequest>,
    ) -> std::result::Result<Response<StatusResponse>, Status> {
        self.authenticate(&request)?;
        let req = request.into_inner();
        let branch_name = req.branch_name;
        let repository_name_opt = req.repository_name;
        if branch_name.is_empty() {
            return Err(Status::invalid_argument("Branch name cannot be empty"));
        }

        let result = async {
             let current_config_arc = self.config.load();
             let mut config = (**current_config_arc).clone();
             let repo_name = repository_name_opt.clone().or_else(|| config.active_repository.clone())
                 .ok_or_else(|| anyhow!("No repository name provided and no active repository set"))?;
             let repo_config_index = config.repositories.iter().position(|r| r.name == repo_name)
                 .ok_or_else(|| anyhow!("Repository '{}' configuration not found.", repo_name))?;
             let repo_config_clone = config.repositories[repo_config_index].clone();
             let branch_name_clone = branch_name.clone();
             tokio::task::spawn_blocking(move || {
                 helpers::switch_repository_branch(&repo_config_clone, &branch_name_clone)
             }).await??;
             let repo_config_mut = &mut config.repositories[repo_config_index];
             repo_config_mut.active_branch = Some(branch_name.to_string());
             if !repo_config_mut.tracked_branches.contains(&branch_name) {
                 repo_config_mut.tracked_branches.push(branch_name.to_string());
             }
             save_config(&config, None).context("Failed to save config")?;
             self.config.store(Arc::new(config));
             info!("Switched repository '{}' to branch '{}'", repo_name, branch_name);
             Ok::<_, anyhow::Error>(repo_name)
        }.await;

        match result {
            Ok(repo_name) => Ok(Response::new(StatusResponse {
                success: true, message: format!("Switched to branch '{}' in repository '{}'", branch_name, repo_name),
            })),
             Err(e) => {
                 if e.to_string().contains("not found") {
                     Err(Status::not_found(e.to_string()))
                 } else {
                     error!("Failed to switch branch to '{}' in repo '{}': {}", branch_name, repository_name_opt.unwrap_or_default(), e);
                     Err(Status::internal(format!("Failed to switch branch: {}", e)))
                 }
            }
        }
    }
}

// Helper functions to extract values from Qdrant payload
#[cfg(feature = "server")]
fn get_string_from_payload(payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>, key: &str) -> Option<String> {
    payload.get(key).and_then(|v| v.kind.as_ref()).and_then(|k| {
        if let qdrant_client::qdrant::value::Kind::StringValue(s) = k {
            Some(s.clone())
        } else {
            None
        }
    })
}

#[cfg(feature = "server")]
fn get_integer_from_payload(payload: &std::collections::HashMap<String, qdrant_client::qdrant::Value>, key: &str) -> Option<i64> {
    payload.get(key).and_then(|v| v.kind.as_ref()).and_then(|k| {
        if let qdrant_client::qdrant::value::Kind::IntegerValue(i) = k {
            Some(*i)
        } else {
            None
        }
    })
} 
