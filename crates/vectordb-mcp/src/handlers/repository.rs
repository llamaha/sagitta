use crate::mcp::{
    error_codes,
    types::{
        ErrorObject, RepositoryAddParams, RepositoryAddResult, RepositoryInfo, RepositoryListParams,
        RepositoryListResult, RepositoryRemoveParams, RepositoryRemoveResult, RepositorySyncParams,
        RepositorySyncResult,
    },
};
use crate::server::map_add_repo_error; // Import helper from server for now
use anyhow::{anyhow, Result};
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};
use vectordb_core::{
    config::{get_repo_base_path, save_config, AppConfig, RepositoryConfig},
    embedding::EmbeddingHandler,
    indexing::{self, gather_files},
    qdrant_client_trait::QdrantClientTrait,
    repo_add::{handle_repo_add, AddRepoArgs},
    repo_helpers::{delete_repository_data, get_collection_name, sync_repository_branch},
};
use crate::server::map_core_error_to_user_message; // Import helper from server for now

#[instrument(skip(config, qdrant_client, embedding_handler), fields(repo_name = %params.name, url = ?params.url))]
pub async fn handle_repository_add<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositoryAddParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    embedding_handler: Arc<EmbeddingHandler>,
) -> Result<RepositoryAddResult, ErrorObject> {
    let initial_base_path = get_repo_base_path(Some(&*config.read().await)).map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to determine repository base path: {}", e),
        data: None,
    })?;
    fs::create_dir_all(&initial_base_path).map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!(
            "Failed to create repository base path {}: {}",
            initial_base_path.display(),
            e
        ),
        data: None,
    })?;

    let embedding_dim = embedding_handler.dimension().map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to get embedding dimension: {}", e),
        data: None,
    })? as u64;

    let new_repo_config_result = handle_repo_add(
        AddRepoArgs {
            url: params.url,
            local_path: params.local_path,
            name: Some(params.name),
            branch: params.branch,
            remote: None,
            repositories_base_path: None,
            ssh_key: params.ssh_key,
            ssh_passphrase: params.ssh_passphrase,
        },
        initial_base_path,
        embedding_dim,
        qdrant_client.clone(),
    )
    .await;

    match new_repo_config_result {
        Ok(repo_config) => {
            let mut config_write_guard = config.write().await;

            // Check if repo already exists in config (handle_repo_add doesn't check config)
            if config_write_guard
                .repositories
                .iter()
                .any(|r| r.name == repo_config.name)
            {
                return Err(ErrorObject {
                    code: error_codes::REPO_ALREADY_EXISTS,
                    message: format!(
                        "Repository '{}' already exists in configuration.",
                        repo_config.name
                    ),
                    data: None,
                });
            }

            config_write_guard.repositories.push(repo_config.clone());

            if let Err(e) = save_config(&*config_write_guard, None) {
                error!(error = %e, repo_name=%repo_config.name, "Failed to save config after adding repository {} to memory", repo_config.name);
                // Attempt to remove the repo we just added to memory if save fails
                config_write_guard.repositories.pop();
                return Err(ErrorObject {
                    code: error_codes::CONFIG_SAVE_FAILED,
                    message: format!("Failed to save configuration after adding repository: {}", e),
                    data: None,
                });
            }
            info!(repo_name=%repo_config.name, "Successfully added repository and saved config.");

            Ok(RepositoryAddResult {
                name: repo_config.name,
                url: repo_config.url,
                local_path: repo_config.local_path,
                default_branch: repo_config.default_branch,
                active_branch: repo_config.active_branch,
            })
        }
        Err(e) => {
            error!(error = %e, "Core handle_repo_add failed");
            Err(map_add_repo_error(e)) // Use imported helper
        }
    }
}

#[instrument(skip(config))]
pub async fn handle_repository_list(
    _params: RepositoryListParams,
    config: Arc<RwLock<AppConfig>>,
) -> Result<RepositoryListResult, ErrorObject> {
    let config_read_guard = config.read().await;
    let repo_infos: Vec<RepositoryInfo> = config_read_guard
        .repositories
        .iter()
        .map(|r| RepositoryInfo {
            name: r.name.clone(),
            remote: r.url.clone(),
            description: None,
            branch: r.active_branch.clone(),
            last_updated: None,
        })
        .collect();
    Ok(RepositoryListResult {
        repositories: repo_infos,
    })
}

#[instrument(skip(config, qdrant_client), fields(repo_name = %params.name))]
pub async fn handle_repository_remove<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositoryRemoveParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
) -> Result<RepositoryRemoveResult, ErrorObject> {
    let mut config_write_guard = config.write().await;

    let repo_index = config_write_guard
        .repositories
        .iter()
        .position(|r| r.name == params.name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::REPO_NOT_FOUND,
            message: format!("Repository '{}' not found for removal", params.name),
            data: None,
        })?;

    let repo_config_to_remove = config_write_guard.repositories[repo_index].clone();

    // Attempt to delete data first
    if let Err(e) = delete_repository_data(&repo_config_to_remove, qdrant_client.clone()).await {
        // Log error but proceed with config removal
        error!(repo_name = %params.name, error = %e, "Failed to delete repository data, proceeding with config removal.");
    }

    // Remove from config
    config_write_guard.repositories.remove(repo_index);

    // Save updated config
    if let Err(e) = save_config(&*config_write_guard, None) {
        error!(error = %e, "Failed to save config after removing repository");
        // TODO: Should we attempt to restore the removed repo in memory?
        return Err(ErrorObject {
            code: error_codes::CONFIG_SAVE_FAILED,
            message: format!("Failed to save configuration after removal: {}", e),
            data: None,
        });
    }

    Ok(RepositoryRemoveResult {
        name: params.name,
        status: "Removed".to_string(),
    })
}

#[instrument(skip(config, qdrant_client, embedding_handler), fields(repo_name = %params.name))]
pub async fn handle_repository_sync<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositorySyncParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    embedding_handler: Arc<EmbeddingHandler>,
) -> Result<RepositorySyncResult, ErrorObject> {
    let commit_hash: String;
    let repo_name = params.name.clone();
    let repo_config_clone: RepositoryConfig;
    let app_config_clone: AppConfig;

    {
        let mut config_write_guard = config.write().await;

        let repo_index = config_write_guard
            .repositories
            .iter()
            .position(|r| r.name == repo_name)
            .ok_or_else(|| ErrorObject {
                code: error_codes::REPO_NOT_FOUND,
                message: format!("Repository '{}' not found", repo_name),
                data: None,
            })?;

        // Pass config_write_guard directly, but it needs to be AppConfig, not &mut AppConfig
        // Also, sync_repository_branch now takes &AppConfig
        let sync_result = sync_repository_branch(&*config_write_guard, repo_index, qdrant_client.clone(), true).await;

        match sync_result {
            Ok(hash) => {
                commit_hash = hash;
                repo_config_clone = config_write_guard.repositories[repo_index].clone();
                app_config_clone = (*config_write_guard).clone();
                if let Err(e) = save_config(&*config_write_guard, None) {
                    error!(error = %e, "Failed to save config after repository sync attempt");
                    return Err(ErrorObject {
                        code: error_codes::CONFIG_SAVE_FAILED,
                        message: format!("Failed to save configuration after sync: {}", e),
                        data: None,
                    });
                }
            }
            Err(e) => {
                // Save config even on sync failure (maybe sync status changed)
                if let Err(save_err) = save_config(&*config_write_guard, None) {
                    error!(error = %save_err, "Failed to save config after failed repository sync attempt");
                }
                error!(repo_name = %repo_name, error = %e, "Repository sync failed (git stage)");
                 let user_message = map_core_error_to_user_message(&anyhow!(e), "Sync failed (git stage)"); // Use imported helper
                return Err(ErrorObject {
                    code: error_codes::CORE_LOGIC_ERROR,
                     message: user_message,
                    data: None,
                });
            }
        }
    } // config_write_guard dropped here

    info!(repo_name = %repo_name, commit = %commit_hash, "Starting indexing stage after successful sync.");

    let repo_root = &repo_config_clone.local_path;
    let branch_name = repo_config_clone
        .active_branch
        .as_deref()
        .unwrap_or(&repo_config_clone.default_branch);
    let collection_name = get_collection_name(&repo_name);

    let files_to_index_abs = match gather_files(&[repo_root.clone()], None) {
        Ok(files) => files,
        Err(e) => {
            error!(repo_name = %repo_name, error = %e, "Failed to list files for indexing");
            return Err(ErrorObject {
                code: error_codes::INTERNAL_ERROR,
                message: format!("Failed to gather files in {}: {}", repo_root.display(), e),
                data: None,
            });
        }
    };

    let files_to_index_rel: Vec<std::path::PathBuf> = files_to_index_abs
        .iter()
        .filter_map(|abs_path| abs_path.strip_prefix(repo_root).ok().map(|p| p.to_path_buf()))
        .collect();

    if files_to_index_rel.is_empty() {
        warn!(repo_name = %repo_name, "No supported files found to index in repository.");
    } else {
        info!(repo_name = %repo_name, count = files_to_index_rel.len(), "Found files to index");

        match indexing::index_repo_files(
            &app_config_clone, // Pass cloned config
            repo_root,
            &files_to_index_rel,
            &collection_name,
            branch_name,
            &commit_hash,
            qdrant_client.clone(),
            embedding_handler.clone(),
            None,
            app_config_clone.indexing.max_concurrent_upserts, // Use cloned config
        )
        .await
        {
            Ok(count) => {
                info!(repo_name = %repo_name, count = count, "Successfully indexed files.");
            }
            Err(e) => {
                error!(repo_name = %repo_name, error = %e, "Indexing failed during sync");
                 let user_message = map_core_error_to_user_message(&anyhow!(e), "Indexing failed"); // Use imported helper
                return Err(ErrorObject {
                    code: error_codes::CORE_LOGIC_ERROR,
                    message: user_message,
                    data: None,
                });
            }
        }
    }

    Ok(RepositorySyncResult {
        name: repo_name,
        status: "Synced and Indexed".to_string(),
        commit_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::{RepositoryListParams, RepositoryListResult, RepositoryInfo};
    use vectordb_core::config::{AppConfig, RepositoryConfig, IndexingConfig};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use std::path::PathBuf;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_handle_repository_list_success() {
        // 1. Setup Mock Config
        let repo1 = RepositoryConfig {
            name: "repo1".to_string(),
            url: "url1".to_string(),
            local_path: PathBuf::from("/path/to/repo1"),
            default_branch: "main".to_string(),
            active_branch: Some("main".to_string()),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::new(),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
        };
        let repo2 = RepositoryConfig {
            name: "repo2".to_string(),
            url: "url2".to_string(),
            local_path: PathBuf::from("/path/to/repo2"),
            default_branch: "dev".to_string(),
            active_branch: Some("dev".to_string()),
            tracked_branches: vec!["dev".to_string()],
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::new(),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
        };
        let config = AppConfig {
            qdrant_url: "dummy".to_string(),
            repositories_base_path: Some(PathBuf::from("/base")), 
            repositories: vec![repo1.clone(), repo2.clone()],
            active_repository: Some("repo1".to_string()),
            indexing: IndexingConfig { max_concurrent_upserts: 1 },
            // Other fields can be default or None as they aren't used by list
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
        };
        let config_arc = Arc::new(RwLock::new(config));

        // 2. Prepare Params
        let params = RepositoryListParams {};

        // 3. Call Handler
        let result = handle_repository_list(params, config_arc).await;

        // 4. Assertions
        assert!(result.is_ok());
        let list_result = result.unwrap(); // Correct: Just unwrap the Result

        assert_eq!(list_result.repositories.len(), 2);

        // Check repo1 details (adjust fields based on RepositoryInfo definition)
        let res_repo1 = list_result.repositories.iter().find(|r| r.name == "repo1").expect("repo1 not found");
        assert_eq!(res_repo1.remote, "url1");
        assert_eq!(res_repo1.branch, Some("main".to_string())); // Assuming RepositoryInfo has 'branch' field mapped from active_branch

        // Check repo2 details
        let res_repo2 = list_result.repositories.iter().find(|r| r.name == "repo2").expect("repo2 not found");
        assert_eq!(res_repo2.remote, "url2");
        assert_eq!(res_repo2.branch, Some("dev".to_string()));
    }
} 