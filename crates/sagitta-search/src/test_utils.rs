#![allow(clippy::arc_with_non_send_sync)] 

use crate::qdrant_client_trait::QdrantClientTrait;
use crate::error::{Result, SagittaError};
use async_trait::async_trait;
use qdrant_client::qdrant::{
    CollectionInfo, CountPoints, CountResponse, DeletePoints, HealthCheckReply, PointsOperationResponse, PointsSelector, QueryPoints, 
    QueryResponse, /*ScoredPoint,*/ ScrollPoints, ScrollResponse, SearchPoints, SearchResponse, UpsertPoints, /*PointId*/ CreateCollection
};
use std::sync::{Arc, Mutex};

/// A manual mock implementation of `QdrantClientTrait` for testing purposes.
/// Allows setting expectations and verifying calls for specific methods.
#[derive(Clone, Debug)]
pub struct ManualMockQdrantClient {
    // For query and query_points
    query_called: Arc<Mutex<bool>>,
    query_points_called: Arc<Mutex<bool>>,
    expected_query_response: Arc<Mutex<Option<Result<QueryResponse>>>>,
    
    // For create_collection
    create_collection_called: Arc<Mutex<bool>>,
    create_collection_args: Arc<Mutex<Option<(String, u64)>>>,
    expected_create_collection_response: Arc<Mutex<Option<Result<bool>>>>,

    // For collection_exists
    collection_exists_called: Arc<Mutex<u32>>,
    collection_exists_args: Arc<Mutex<Vec<String>>>,
    expected_collection_exists_responses: Arc<Mutex<Vec<Result<bool>>>>,
    
    // For get_collection_info
    get_collection_info_called: Arc<Mutex<u32>>,
    get_collection_info_args: Arc<Mutex<Vec<String>>>,
    expected_get_collection_info_responses: Arc<Mutex<Vec<Result<CollectionInfo>>>>,
    
    // For delete_collection
    delete_collection_called: Arc<Mutex<u32>>,
    delete_collection_args: Arc<Mutex<Vec<String>>>,
    expected_delete_collection_responses: Arc<Mutex<Vec<Result<bool>>>>,
}

impl ManualMockQdrantClient {
    /// Creates a new instance of the manual mock client.
    pub fn new() -> Self {
        Self {
            query_called: Arc::new(Mutex::new(false)),
            query_points_called: Arc::new(Mutex::new(false)),
            expected_query_response: Arc::new(Mutex::new(None)),
            create_collection_called: Arc::new(Mutex::new(false)),
            create_collection_args: Arc::new(Mutex::new(None)),
            expected_create_collection_response: Arc::new(Mutex::new(None)),
            collection_exists_called: Arc::new(Mutex::new(0)),
            collection_exists_args: Arc::new(Mutex::new(Vec::new())),
            expected_collection_exists_responses: Arc::new(Mutex::new(Vec::new())),
            get_collection_info_called: Arc::new(Mutex::new(0)),
            get_collection_info_args: Arc::new(Mutex::new(Vec::new())),
            expected_get_collection_info_responses: Arc::new(Mutex::new(Vec::new())),
            delete_collection_called: Arc::new(Mutex::new(0)),
            delete_collection_args: Arc::new(Mutex::new(Vec::new())),
            expected_delete_collection_responses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    // --- Methods for query/query_points --- 
    /// Sets the expected response for the next `query` or `query_points` call.
    pub fn expect_query(&self, response: Result<QueryResponse>) {
        *self.expected_query_response.lock().unwrap() = Some(response);
    }

    /// Verifies if the `query` method was called.
    pub fn verify_query_called(&self) -> bool {
        *self.query_called.lock().unwrap()
    }
    
    /// Sets the expected response for the next `query` or `query_points` call. 
    /// (Currently shares expectation with `expect_query`).
    pub fn expect_query_points(&self, response: Result<QueryResponse>) {
        *self.expected_query_response.lock().unwrap() = Some(response); 
    }

    /// Verifies if the `query_points` method was called.
    pub fn verify_query_points_called(&self) -> bool {
        *self.query_points_called.lock().unwrap()
    }

    // --- Methods for create_collection --- 
    /// Sets the expected response for the next `create_collection` call.
    pub fn expect_create_collection(&self, response: Result<bool>) {
        *self.expected_create_collection_response.lock().unwrap() = Some(response);
    }

    /// Verifies if the `create_collection` method was called.
    pub fn verify_create_collection_called(&self) -> bool {
        *self.create_collection_called.lock().unwrap()
    }

    /// Verifies the arguments passed to the last `create_collection` call.
    pub fn verify_create_collection_args(&self, name: &str, dim: u64) -> bool {
        if let Some(args) = &*self.create_collection_args.lock().unwrap() {
            args.0 == name && args.1 == dim
        } else {
            false
        }
    }

    /// Returns the arguments passed to the last `create_collection` call.
    pub fn get_create_collection_args(&self) -> (String, u64) {
        self.create_collection_args.lock().unwrap().clone().unwrap_or(("".to_string(), 0))
    }

    // --- Methods for collection_exists --- 
    /// Adds an expected response to the queue for future `collection_exists` calls.
    pub fn expect_collection_exists(&self, response: Result<bool>) {
        self.expected_collection_exists_responses.lock().unwrap().push(response);
    }

    /// Returns the number of times `collection_exists` was called.
    pub fn verify_collection_exists_called_times(&self) -> u32 {
        *self.collection_exists_called.lock().unwrap()
    }

    /// Returns a clone of the arguments passed to each `collection_exists` call.
    pub fn get_collection_exists_args(&self) -> Vec<String> {
        self.collection_exists_args.lock().unwrap().clone()
    }
    
    // --- Methods for get_collection_info ---
    /// Adds an expected response to the queue for future `get_collection_info` calls.
    pub fn expect_get_collection_info(&self, response: Result<CollectionInfo>) {
        self.expected_get_collection_info_responses.lock().unwrap().push(response);
    }
    
    // --- Methods for delete_collection ---
    /// Adds an expected response to the queue for future `delete_collection` calls.
    pub fn expect_delete_collection(&self, response: Result<bool>) {
        self.expected_delete_collection_responses.lock().unwrap().push(response);
    }
}

#[async_trait]
impl QdrantClientTrait for ManualMockQdrantClient {
    async fn query_points(&self, _request: QueryPoints) -> Result<QueryResponse> {
        *self.query_points_called.lock().unwrap() = true;
        log::debug!("ManualMock: query_points called");
        self.expected_query_response.lock().unwrap().take()
            .unwrap_or_else(|| Err(SagittaError::Other("Mock Error: query_points called without expected response".into())))
    }

    async fn query(&self, _request: QueryPoints) -> Result<QueryResponse> {
        *self.query_called.lock().unwrap() = true;
        log::debug!("ManualMock: query called");
        self.expected_query_response.lock().unwrap().take()
            .unwrap_or_else(|| Err(SagittaError::Other("Mock Error: query called without expected response".into())))
    }

    async fn create_collection(&self, collection_name: &str, vector_dimension: u64) -> Result<bool> {
        *self.create_collection_called.lock().unwrap() = true;
        *self.create_collection_args.lock().unwrap() = Some((collection_name.to_string(), vector_dimension));
        log::debug!("ManualMock: create_collection called with name: {}, dim: {}", collection_name, vector_dimension);
        self.expected_create_collection_response.lock().unwrap().take()
            .unwrap_or_else(|| Err(SagittaError::Other("Mock Error: create_collection called without expected response".into())))
    }

    async fn create_collection_detailed(&self, request: CreateCollection) -> Result<bool> {
        // For the mock, we'll just delegate to the simpler create_collection method
        // Extract collection name and vector size from the request
        let collection_name = &request.collection_name;
        let vector_size = if let Some(vectors_config) = &request.vectors_config {
            if let Some(config) = &vectors_config.config {
                match config {
                    qdrant_client::qdrant::vectors_config::Config::Params(params) => params.size,
                    qdrant_client::qdrant::vectors_config::Config::ParamsMap(_) => 384, // Default fallback
                }
            } else {
                384 // Default fallback
            }
        } else {
            384 // Default fallback
        };
        
        log::debug!("ManualMock: create_collection_detailed called, delegating to create_collection");
        self.create_collection(collection_name, vector_size).await
    }

    async fn collection_exists(&self, collection_name: String) -> Result<bool> {
        *self.collection_exists_called.lock().unwrap() += 1;
        self.collection_exists_args.lock().unwrap().push(collection_name.clone());
        log::debug!("ManualMock: collection_exists called with name: {}", collection_name);
        let mut responses = self.expected_collection_exists_responses.lock().unwrap();
        if responses.is_empty() {
            Err(SagittaError::Other("Mock Error: collection_exists called without expected response(s)".into()))
        } else {
            responses.remove(0) // FIFO for multiple expected calls
        }
    }

    // --- Implement other trait methods with default "unimplemented" behavior --- 
    async fn health_check(&self) -> Result<HealthCheckReply> { unimplemented!("health_check not mocked in ManualMockQdrantClient") }
    async fn delete_collection(&self, collection_name: String) -> Result<bool> {
        *self.delete_collection_called.lock().unwrap() += 1;
        self.delete_collection_args.lock().unwrap().push(collection_name.clone());
        log::debug!("ManualMock: delete_collection called for {}", collection_name);
        
        let mut responses = self.expected_delete_collection_responses.lock().unwrap();
        if !responses.is_empty() {
            responses.remove(0)
        } else {
            Err(SagittaError::Other("Mock Error: delete_collection called without expected response".into()))
        }
    }
    async fn search_points(&self, _request: SearchPoints) -> Result<SearchResponse> { unimplemented!("search_points not mocked in ManualMockQdrantClient") }
    async fn get_collection_info(&self, collection_name: String) -> Result<CollectionInfo> {
        *self.get_collection_info_called.lock().unwrap() += 1;
        self.get_collection_info_args.lock().unwrap().push(collection_name.clone());
        log::debug!("ManualMock: get_collection_info called for {}", collection_name);
        
        let mut responses = self.expected_get_collection_info_responses.lock().unwrap();
        if !responses.is_empty() {
            responses.remove(0)
        } else {
            Err(SagittaError::Other("Mock Error: get_collection_info called without expected response".into()))
        }
    }
    async fn count(&self, _request: CountPoints) -> Result<CountResponse> { unimplemented!("count not mocked in ManualMockQdrantClient") }
    async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &PointsSelector) -> Result<()> { unimplemented!("delete_points_blocking not mocked in ManualMockQdrantClient") }
    async fn scroll(&self, _request: ScrollPoints) -> Result<ScrollResponse> { unimplemented!("scroll not mocked in ManualMockQdrantClient") }
    async fn upsert_points(&self, _request: UpsertPoints) -> Result<PointsOperationResponse> { unimplemented!("upsert_points not mocked in ManualMockQdrantClient") }
    async fn delete_points(&self, _request: DeletePoints) -> Result<PointsOperationResponse> { unimplemented!("delete_points not mocked in ManualMockQdrantClient") }
    async fn list_collections(&self) -> Result<Vec<String>> { unimplemented!("list_collections not mocked in ManualMockQdrantClient") }
} 

#[cfg(test)]
mod enhanced_repository_tests {
    use super::*;
    use crate::{AppConfig, RepositoryConfig};
    use crate::config::{IndexingConfig, PerformanceConfig};
    use crate::{get_enhanced_repository_list, get_enhanced_repository_info, EnhancedRepositoryInfo, FilesystemStatus, SyncState};
    use std::path::PathBuf;
    use std::fs;
    use tempfile::TempDir;
    use tokio;

    pub fn create_test_enhanced_repo_config(name: &str, temp_dir: &TempDir) -> RepositoryConfig {
        let repo_path = temp_dir.path().join(name);
        fs::create_dir_all(&repo_path).unwrap();
        
        // Create some test files
        fs::write(repo_path.join("test.rs"), "fn main() {}\n").unwrap();
        fs::write(repo_path.join("readme.md"), "# Test Repo\n").unwrap();
        fs::write(repo_path.join("config.json"), "{}").unwrap();
        
        // Initialize as git repo and create initial commit
        let repo = git2::Repository::init(&repo_path).unwrap();
        
        // Create an initial commit to avoid unborn branch
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("test.rs")).unwrap();
            index.add_path(std::path::Path::new("readme.md")).unwrap();
            index.add_path(std::path::Path::new("config.json")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        let commit_oid = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        RepositoryConfig {
            name: name.to_string(),
            url: format!("https://github.com/test/{}.git", name),
            local_path: repo_path,
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string(), "dev".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: std::collections::HashMap::from([
                ("main".to_string(), commit_oid.to_string()),
            ]),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: Some(vec!["rust".to_string(), "markdown".to_string()]),
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_get_enhanced_repository_info() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_config = create_test_enhanced_repo_config("test-repo", &temp_dir);
        
        let enhanced_info = get_enhanced_repository_info(&repo_config).await.unwrap();
        
        assert_eq!(enhanced_info.name, "test-repo");
        assert_eq!(enhanced_info.url, "https://github.com/test/test-repo.git");
        assert_eq!(enhanced_info.default_branch, "main");
        assert_eq!(enhanced_info.active_branch, Some("main".to_string()));
        assert_eq!(enhanced_info.tracked_branches, vec!["main", "dev"]);
        
        // Check filesystem status
        assert!(enhanced_info.filesystem_status.exists);
        assert!(enhanced_info.filesystem_status.accessible);
        assert!(enhanced_info.filesystem_status.is_git_repository);
        assert!(enhanced_info.filesystem_status.total_files.unwrap() >= 3);
        
        // Check file extensions
        assert!(!enhanced_info.file_extensions.is_empty());
        let rust_ext = enhanced_info.file_extensions.iter()
            .find(|ext| ext.extension == "rs");
        assert!(rust_ext.is_some());
        assert_eq!(rust_ext.unwrap().count, 1);
        
        // Check sync status
        assert_eq!(enhanced_info.sync_status.state, SyncState::UpToDate); // Commit hash matches
        assert!(enhanced_info.sync_status.last_synced_commits.contains_key("main"));
        
        // Check other fields
        assert_eq!(enhanced_info.indexed_languages, Some(vec!["rust".to_string(), "markdown".to_string()]));
        assert!(!enhanced_info.added_as_local_path);
    }

    #[tokio::test]
    async fn test_get_enhanced_repository_list() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo1 = create_test_enhanced_repo_config("repo1", &temp_dir);
        let repo2 = create_test_enhanced_repo_config("repo2", &temp_dir);
        
        let config = AppConfig {
            repositories: vec![repo1, repo2],
            active_repository: Some("repo1".to_string()),
            ..Default::default()
        };
        
        let enhanced_list = get_enhanced_repository_list(&config).await.unwrap();
        
        assert_eq!(enhanced_list.total_count, 2);
        assert_eq!(enhanced_list.active_repository, Some("repo1".to_string()));
        assert_eq!(enhanced_list.repositories.len(), 2);
        
        // Check summary statistics
        assert_eq!(enhanced_list.summary.existing_count, 2);
        assert!(enhanced_list.summary.total_files >= 6); // 3 files per repo
        assert!(!enhanced_list.summary.common_extensions.is_empty());
        
        // Check that repositories are properly enhanced
        let repo1_info = enhanced_list.repositories.iter()
            .find(|r| r.name == "repo1")
            .unwrap();
        assert!(repo1_info.filesystem_status.exists);
        assert!(!repo1_info.file_extensions.is_empty());
    }

    #[tokio::test]
    async fn test_enhanced_info_with_nonexistent_repo() {
        let nonexistent_path = std::path::PathBuf::from("/nonexistent/path");
        let repo_config = RepositoryConfig {
            name: "nonexistent".to_string(),
            url: "https://github.com/test/nonexistent.git".to_string(),
            local_path: nonexistent_path,
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: std::collections::HashMap::new(),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
        };
        
        let enhanced_info = get_enhanced_repository_info(&repo_config).await.unwrap();
        
        assert_eq!(enhanced_info.name, "nonexistent");
        assert!(!enhanced_info.filesystem_status.exists);
        assert!(!enhanced_info.filesystem_status.accessible);
        assert!(!enhanced_info.filesystem_status.is_git_repository);
        assert_eq!(enhanced_info.filesystem_status.total_files, None);
        assert_eq!(enhanced_info.git_status, None);
        assert_eq!(enhanced_info.sync_status.state, SyncState::NeverSynced);
        assert!(enhanced_info.file_extensions.is_empty());
    }

    #[tokio::test]
    async fn test_file_extension_statistics() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("test-extensions");
        fs::create_dir_all(&repo_path).unwrap();
        
        // Create files with different extensions
        fs::write(repo_path.join("file1.rs"), "fn main() {}").unwrap();
        fs::write(repo_path.join("file2.rs"), "struct Test {}").unwrap();
        fs::write(repo_path.join("file1.py"), "def main(): pass").unwrap();
        fs::write(repo_path.join("README.md"), "# Test").unwrap();
        fs::write(repo_path.join("config.json"), "{}").unwrap();
        fs::write(repo_path.join("no_extension"), "content").unwrap();
        
        let repo_config = RepositoryConfig {
            name: "test-extensions".to_string(),
            url: "https://github.com/test/test-extensions.git".to_string(),
            local_path: repo_path,
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            remote_name: Some("origin".to_string()),
            last_synced_commits: std::collections::HashMap::new(),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
        };
        
        let enhanced_info = get_enhanced_repository_info(&repo_config).await.unwrap();
        
        // Check file extension statistics
        assert!(!enhanced_info.file_extensions.is_empty());
        
        // Find rust files (should be most common with 2 files)
        let rust_ext = enhanced_info.file_extensions.iter()
            .find(|ext| ext.extension == "rs")
            .unwrap();
        assert_eq!(rust_ext.count, 2);
        
        // Check other extensions exist
        let py_ext = enhanced_info.file_extensions.iter()
            .find(|ext| ext.extension == "py");
        assert!(py_ext.is_some());
        assert_eq!(py_ext.unwrap().count, 1);
        
        let md_ext = enhanced_info.file_extensions.iter()
            .find(|ext| ext.extension == "md");
        assert!(md_ext.is_some());
        
        let json_ext = enhanced_info.file_extensions.iter()
            .find(|ext| ext.extension == "json");
        assert!(json_ext.is_some());
        
        let no_ext = enhanced_info.file_extensions.iter()
            .find(|ext| ext.extension == "no_extension");
        assert!(no_ext.is_some());
    }
} 