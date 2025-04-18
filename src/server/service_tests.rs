#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, RepositoryConfig};
    use crate::server::api::{SyncRepositoryRequest, StatusResponse};
    use crate::server::service::VectorDBServiceImpl;
    use crate::vectordb::VectorDBService;
    use qdrant_client::Qdrant;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tonic::Request;
    use git2;

    // Helper function to create a test service instance
    async fn setup_test_service(
        initial_repos: Vec<RepositoryConfig>,
    ) -> (VectorDBServiceImpl, Arc<AppConfig>, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let base_path = temp_dir.path().join("repos");
        std::fs::create_dir_all(&base_path).expect("Failed to create base path");

        // Create config with provided repos
        let config = AppConfig {
            repositories: initial_repos,
            active_repository: None,
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            repositories_base_path: Some(base_path.to_str().unwrap().to_string()),
        };
        config.save(&config_path).expect("Failed to save config");

        // Mock Qdrant client
        let client = Qdrant::from_url(&config.qdrant_url)
            .build()
            .expect("Failed to create Qdrant client");

        let service = VectorDBServiceImpl::new(Arc::new(config.clone()), client);
        (service, Arc::new(config), temp_dir)
    }

    // Helper to initialize a simple git repo
    fn init_test_git_repo(repo_path: &Path) -> (git2::Repository, git2::Oid) {
        let repo = git2::Repository::init(repo_path).expect("Failed to init test repo");
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        // Create an initial commit
        let mut index = repo.index().unwrap();
        let mut file = File::create(repo_path.join("README.md")).unwrap();
        writeln!(file, "Initial commit").unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        let oid = index.write_tree().unwrap();
        let signature = repo.signature().unwrap();
        let tree = repo.find_tree(oid).unwrap();
        let commit_oid = repo
            .commit(
                Some("HEAD"), // Point HEAD to our new commit
                &signature,   // Author
                &signature,   // Committer
                "Initial commit", // Commit message
                &tree,        // Tree
                &[],          // No parents means initial commit
            )
            .unwrap();
        (repo, commit_oid)
    }

    #[tokio::test]
    async fn test_get_server_info() {
        let (service, _config, _temp_dir) = setup_test_service(Vec::new()).await;
        let request = Request::new(crate::server::api::Empty {});
        let response = service.get_server_info(request).await;
        assert!(response.is_ok());
        let info = response.unwrap().into_inner();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_sync_repository_success() {
        let repo_name = "test_repo".to_string();
        let temp_repo_parent_dir = tempdir().expect("Failed to create temp parent dir");
        let repo_path = temp_repo_parent_dir.path().join(&repo_name);
        let (_git_repo, initial_commit_oid) = init_test_git_repo(&repo_path);

        let repo_config = RepositoryConfig {
            name: repo_name.clone(),
            url: repo_path.to_str().unwrap().to_string(), // Use local path as URL for test
            local_path: repo_path.clone(),
            active_branch: Some("main".to_string()),
            last_synced_commits: HashMap::new(), // Start fresh
            remote_name: None,
            default_branch: None, // Not relevant for sync test setup
            ssh_key_path: None,
            ssh_passphrase_env_var: None
        };

        let (service, _config, _temp_dir) = setup_test_service(vec![repo_config]).await;

        let request = Request::new(SyncRepositoryRequest {
            name: Some(repo_name.clone()),
            extensions: vec!["md".to_string()], // Filter for markdown
            force: false,
        });

        let response = service.sync_repository(request).await;

        // Assert basic success - deeper checks would need mocking git::sync_repository
        // or a running Qdrant instance to verify indexing.
        assert!(response.is_ok(), "gRPC call failed: {:?}", response.err());
        let status = response.unwrap().into_inner();
        assert!(status.success, "Sync reported failure: {}", status.message);
        assert!(status.message.contains(&format!("Successfully synced repository '{}'", repo_name)));

        // Cleanup temp repo dir
        // temp_repo_parent_dir.close().expect("Failed to close temp repo dir");
    }

    // TODO: Add tests for other repository operations
} 