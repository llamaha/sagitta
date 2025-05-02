pub mod list;
pub mod r#use;
pub mod clear;
pub mod query;
pub mod sync;
pub mod use_branch;
pub mod remove;
pub mod config;
pub mod search_file;
pub mod view_file;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use vectordb_core::config::{AppConfig, save_config};
use vectordb_core::embedding::EmbeddingHandler;
use vectordb_core::qdrant_client_trait::QdrantClientTrait;
use vectordb_core::config::get_repo_base_path;
use crate::cli::CliArgs;

#[derive(Args, Debug, Clone)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RepoCommand {
    Add(vectordb_core::repo_add::AddRepoArgs),
    List(list::ListArgs),
    Use(r#use::UseRepoArgs),
    Remove(remove::RemoveRepoArgs),
    Clear(clear::ClearRepoArgs),
    UseBranch(use_branch::UseBranchArgs),
    Query(query::RepoQueryArgs),
    Sync(sync::SyncRepoArgs),
    Stats(crate::cli::stats::StatsArgs),
    Config(config::ConfigArgs),
    SearchFile(search_file::SearchFileArgs),
    ViewFile(view_file::ViewFileArgs),
}

pub async fn handle_repo_command<C>(
    args: RepoArgs,
    cli_args: &CliArgs,
    config: &mut AppConfig,
    client: Arc<C>,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let command_result = match args.command {
        RepoCommand::Add(add_args) => {
            let embedding_handler = EmbeddingHandler::new(config)
                 .context("Failed to initialize embedding handler (check ONNX config)")?;
            let embedding_dim = embedding_handler.dimension()
                 .context("Failed to get embedding dimension")?;
            let repo_base_path = match &add_args.repositories_base_path {
                Some(path) => path.clone(),
                None => get_repo_base_path(Some(config))
                            .context("Failed to determine repository base path")?,
            };
            std::fs::create_dir_all(&repo_base_path)
                 .with_context(|| format!("Failed to create base directory: {}", repo_base_path.display()))?;
            let repo_config_result = vectordb_core::repo_add::handle_repo_add(
                add_args, 
                repo_base_path, 
                embedding_dim as u64, 
                Arc::clone(&client)
            ).await;
            match repo_config_result {
                Ok(new_repo_config) => {
                    config.repositories.push(new_repo_config.clone());
                    save_config(config, override_path)?;
                    Ok(())
                },
                Err(e) => Err(anyhow::Error::from(e)),
            }
        },
        RepoCommand::List(list_args) => {
            list::list_repositories(config, list_args)?;
            Ok(())
        },
        RepoCommand::Use(use_args) => {
            r#use::use_repository(use_args, config, override_path)?;
            Ok(())
        },
        RepoCommand::Remove(remove_args) => Ok(remove::handle_repo_remove(remove_args, config, client.clone(), override_path).await?),
        RepoCommand::Clear(clear_args) => Ok(clear::handle_repo_clear(clear_args, config, client.clone(), override_path).await?),
        RepoCommand::UseBranch(branch_args) => {
            use_branch::handle_use_branch(branch_args, config, override_path).await?;
            Ok(())
        },
        RepoCommand::Query(query_args) => {
            query::handle_repo_query(query_args, config, Arc::clone(&client), cli_args).await?;
            Ok(())
        },
        RepoCommand::Sync(sync_args) => {
            sync::handle_repo_sync(sync_args, config, client.clone(), override_path).await?;
            Ok(())
        },
        RepoCommand::Stats(stats_args) => {
            crate::cli::stats::handle_stats(stats_args, config.clone(), Arc::clone(&client)).await?;
            Ok(())
        },
        RepoCommand::Config(config_args) => {
            config::handle_config(config_args, config, override_path)?;
            Ok(())
        },
        RepoCommand::SearchFile(search_args) => {
            search_file::handle_repo_search_file(search_args, config).await?;
            Ok(())
        },
        RepoCommand::ViewFile(view_args) => {
            view_file::handle_repo_view_file(view_args, config).await?;
            Ok(())
        },
    };
    command_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::repo_commands::RepoArgs; 
    use vectordb_core::config::{AppConfig, RepositoryConfig, IndexingConfig, load_config, save_config};
    use std::path::PathBuf;
    use crate::cli::commands::Commands;
    use crate::cli::repo_commands::{RepoCommand};
    use crate::cli::repo_commands::remove::RemoveRepoArgs;
    use qdrant_client::Qdrant;
    use std::sync::Arc;
    use tokio::runtime::Runtime;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::{tempdir};
    use std::time::{SystemTime, UNIX_EPOCH};
    use crate::cli::CliArgs; 

    fn default_test_config() -> AppConfig {
        let temp_dir = tempdir().expect("Failed to create temp dir for test config");
        let base_path = temp_dir.path().join("repositories");
        fs::create_dir_all(&base_path).unwrap();
        let vocab_base = temp_dir.path().join("vocab_def");
        fs::create_dir_all(&vocab_base).unwrap();
        let config = AppConfig {
            qdrant_url: "http://localhost:6333".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            repositories_base_path: Some(base_path.to_string_lossy().into_owned()),
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()),
            repositories: vec![],
            active_repository: None,
            indexing: Default::default(),
        };
        config
    }

    fn create_test_config_data() -> AppConfig {
        let temp_dir = tempdir().expect("Failed to create temp dir for test config data");
        let repo_base = temp_dir.path().join("repos");
        fs::create_dir_all(&repo_base).unwrap();
        let vocab_base = temp_dir.path().join("vocab_data");
        fs::create_dir_all(&vocab_base).unwrap();
        let model_base = temp_dir.path().join("models");
        fs::create_dir_all(&model_base).unwrap();
        let dummy_model = model_base.join("model.onnx");
        let dummy_tokenizer_dir = model_base.join("tokenizer");
        fs::write(&dummy_model, "dummy").unwrap();
        fs::create_dir(&dummy_tokenizer_dir).unwrap();
        fs::write(dummy_tokenizer_dir.join("tokenizer.json"), "{}").unwrap();

        AppConfig {
            repositories: vec![
                RepositoryConfig {
                    name: "repo1".to_string(),
                    url: "url1".to_string(),
                    local_path: repo_base.join("repo1"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    remote_name: Some("origin".to_string()),
                    active_branch: Some("main".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: HashMap::new(),
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                },
                RepositoryConfig {
                    name: "repo2".to_string(),
                    url: "url2".to_string(),
                    local_path: repo_base.join("repo2"),
                    default_branch: "dev".to_string(),
                    tracked_branches: vec!["dev".to_string()],
                    remote_name: Some("origin".to_string()),
                    active_branch: Some("dev".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: HashMap::new(),
                    indexed_languages: None,
                    added_as_local_path: false,
                    target_ref: None,
                },
            ],
            active_repository: None,
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: Some(dummy_model.to_string_lossy().into_owned()),
            onnx_tokenizer_path: Some(dummy_tokenizer_dir.to_string_lossy().into_owned()),
            server_api_key_path: None,
            repositories_base_path: Some(repo_base.to_string_lossy().into_owned()),
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()),
            indexing: IndexingConfig::default(),
        }
    }

    fn create_dummy_cli_args(repo_command: RepoCommand) -> CliArgs {
        let dummy_model_path = Some(PathBuf::from("/tmp/dummy_model.onnx"));
        let dummy_tokenizer_dir = Some(PathBuf::from("/tmp/dummy_tokenizer/"));

        CliArgs {
             command: Commands::Repo(RepoArgs { command: repo_command }),
             onnx_model_path_arg: dummy_model_path.map(|p| p.to_string_lossy().into_owned()),
             onnx_tokenizer_dir_arg: dummy_tokenizer_dir.map(|p| p.to_string_lossy().into_owned()),
         }
      }

    fn unique_suffix() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis()
            .to_string()
    }

    #[test]
    fn test_handle_repo_clear_specific_repo() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let suffix = unique_suffix();
            let test_repo_name = format!("clear-specific-{}", suffix);
            let collection_name = format!("repo_{}", test_repo_name);
            let temp_dir = tempdir().unwrap();
            let temp_path = temp_dir.path().join("clear_specific_config.toml");

            let mut config = create_test_config_data();
            config.repositories.push(RepositoryConfig {
                 name: test_repo_name.clone(), 
                 url: "url_clear".to_string(), 
                 local_path: PathBuf::from("/tmp/clear_spec"), 
                 default_branch: "main".to_string(), 
                 tracked_branches: vec![], 
                 active_branch: None, 
                 remote_name: None, 
                 ssh_key_path: None, 
                 ssh_key_passphrase: None, 
                 last_synced_commits: HashMap::from([("main".to_string(), "dummy_commit".to_string())]),
                 indexed_languages: Some(vec!["rust".to_string()]),
                 added_as_local_path: false,
                 target_ref: None,
            });
            config.active_repository = Some("other_repo".to_string());
            save_config(&config, Some(&temp_path)).unwrap();
            
            let args = clear::ClearRepoArgs { name: Some(test_repo_name.to_string()), yes: true };
            let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));

            let client = Arc::new(Qdrant::from_url(&config.qdrant_url).build().expect("Failed to create Qdrant client"));

            let _ = client.delete_collection(&collection_name).await;

            let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
            
            assert!(result.is_ok(), "handle_repo_command failed: {:?}", result.err());

            let saved_config = load_config(Some(&temp_path)).unwrap();
            let updated_repo = saved_config.repositories.iter().find(|r| r.name == test_repo_name).expect("Test repo config not found after clear");
            assert!(updated_repo.last_synced_commits.is_empty(), "Sync status was not cleared");
            assert!(updated_repo.indexed_languages.is_none(), "Indexed languages were not cleared");
        
            let _ = client.delete_collection(&collection_name).await;
        });
    }
    #[test]
    fn test_handle_repo_clear_active_repo() {
         let rt = Runtime::new().unwrap();
         rt.block_on(async {
             let suffix = unique_suffix();
             let active_repo_name = format!("clear-active-{}", suffix);
             let collection_name = format!("repo_{}", active_repo_name);
             let temp_dir = tempdir().unwrap();
             let temp_path = temp_dir.path().join("clear_active_config.toml");

             let mut config = create_test_config_data(); 
             config.repositories.push(RepositoryConfig {
                 name: active_repo_name.clone(), 
                 url: "url_clear_active".to_string(), 
                 local_path: PathBuf::from("/tmp/clear_active"), 
                 default_branch: "main".to_string(), 
                 tracked_branches: vec![], 
                 active_branch: Some("main".to_string()), 
                 remote_name: None, 
                 ssh_key_path: None, 
                 ssh_key_passphrase: None, 
                 last_synced_commits: HashMap::from([("main".to_string(), "dummy_commit_2".to_string())]),
                 indexed_languages: Some(vec!["python".to_string()]),
                 added_as_local_path: false,
                 target_ref: None,
             });
             config.active_repository = Some(active_repo_name.clone());
             save_config(&config, Some(&temp_path)).unwrap();

             let args = clear::ClearRepoArgs { name: None, yes: true };
             let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));

             let client = Arc::new(Qdrant::from_url(&config.qdrant_url).build().expect("Failed to create Qdrant client"));

             let _ = client.delete_collection(&collection_name).await;

             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
             
             assert!(result.is_ok(), "handle_repo_command failed: {:?}", result.err());

             let saved_config = load_config(Some(&temp_path)).unwrap();
             let updated_repo = saved_config.repositories.iter().find(|r| r.name == active_repo_name).expect("Active repo config not found after clear");
             assert!(updated_repo.last_synced_commits.is_empty(), "Sync status was not cleared for active repo");
             assert!(updated_repo.indexed_languages.is_none(), "Indexed languages were not cleared for active repo");
         
             let _ = client.delete_collection(&collection_name).await;
         });
    }
    #[test]
    fn test_handle_repo_clear_no_active_or_specified_fails() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            let mut config = create_test_config_data();
            config.repositories.clear();
            config.active_repository = None;

            let args = clear::ClearRepoArgs { name: None, yes: true }; 
            let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));

            let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client, None).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("No active repository set"));
        });
    }

     #[test]
     fn test_handle_repo_use_existing() {
         let temp_dir = tempdir().unwrap();
         let temp_path = temp_dir.path().join("test_config.toml");

         let mut config = create_test_config_data();
         config.active_repository = Some("repo1".to_string());
         save_config(&config, Some(&temp_path)).unwrap();

         let use_args = r#use::UseRepoArgs { name: "repo2".to_string() };
         let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); 
         let dummy_cli_args = create_dummy_cli_args(RepoCommand::Use(use_args.clone()));

         let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
              handle_repo_command(RepoArgs{ command: RepoCommand::Use(use_args)}, &dummy_cli_args, &mut config, client, Some(&temp_path)).await
         });
         assert!(result.is_ok());
         
         let saved_config = load_config(Some(&temp_path)).unwrap();
         assert_eq!(saved_config.active_repository, Some("repo2".to_string()));

     }

     #[test]
     fn test_handle_repo_use_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().join("test_config.toml");

        let mut config = create_test_config_data();
        save_config(&config, Some(&temp_path)).unwrap();
        let initial_config_state = config.clone();
        
        let use_args = r#use::UseRepoArgs { name: "repo3".to_string() }; 
        let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); 
        let dummy_cli_args = create_dummy_cli_args(RepoCommand::Use(use_args.clone()));

        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
              handle_repo_command(RepoArgs{ command: RepoCommand::Use(use_args)}, &dummy_cli_args, &mut config, client, Some(&temp_path)).await
         });
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository 'repo3' not found"));

        let saved_config = load_config(Some(&temp_path)).unwrap();
        assert_eq!(saved_config.repositories, initial_config_state.repositories);
        assert_eq!(saved_config.active_repository, initial_config_state.active_repository);

     }

     #[test]
     fn test_handle_repo_remove_config_only_non_active() {
        let rt = Runtime::new().unwrap();
         rt.block_on(async {
             let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
             let temp_dir = tempdir().unwrap();
             let temp_path = temp_dir.path().join("test_config.toml");

             let mut config = create_test_config_data();
             config.active_repository = Some("repo1".to_string());
             save_config(&config, Some(&temp_path)).unwrap();
             let initial_repo_count = config.repositories.len();
             
             let remove_args = RemoveRepoArgs { 
                 name: "repo2".to_string(), 
                 yes: true,
                 delete_local: false
             }; 
             let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));
             let _ = fs::remove_dir_all("/tmp/vectordb_test_repo2");

             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
             assert!(result.is_ok());
             
             let saved_config = load_config(Some(&temp_path)).unwrap();
             assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
             assert!(!saved_config.repositories.iter().any(|r| r.name == "repo2"));
             assert_eq!(saved_config.active_repository, Some("repo1".to_string()));

         });
     }

      #[test]
      fn test_handle_repo_remove_config_only_active() {
         let rt = Runtime::new().unwrap();
          rt.block_on(async {
              let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
              let temp_dir = tempdir().unwrap();
              let temp_path = temp_dir.path().join("test_config.toml");

              let mut config = create_test_config_data();
              config.active_repository = Some("repo2".to_string());
              config.repositories.push(RepositoryConfig {
                  name: "repo3".to_string(),
                  url: "url3".to_string(),
                  local_path: PathBuf::from("/tmp/vectordb_test_repo3"),
                  default_branch: "main".to_string(),
                  tracked_branches: vec!["main".to_string()],
                  remote_name: Some("origin".to_string()),
                  active_branch: Some("main".to_string()),
                  ssh_key_path: None,
                  ssh_key_passphrase: None,
                  last_synced_commits: HashMap::new(),
                  indexed_languages: None,
                  added_as_local_path: false,
                  target_ref: None,
              });
              save_config(&config, Some(&temp_path)).unwrap();
              let initial_repo_count = config.repositories.len();

              let remove_args = RemoveRepoArgs { 
                  name: "repo2".to_string(), 
                  yes: true,
                  delete_local: false
              }; 
              let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));
              let _ = fs::remove_dir_all("/tmp/vectordb_test_repo2");

              let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
              assert!(result.is_ok());
              
              let saved_config = load_config(Some(&temp_path)).unwrap();
              assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
              assert!(!saved_config.repositories.iter().any(|r| r.name == "repo2"));
              assert_eq!(saved_config.active_repository, None);

          });
      }

      #[test]
      fn test_handle_repo_remove_with_local_delete() {
         let rt = Runtime::new().unwrap();
         rt.block_on(async {
             let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
             let temp_dir = tempdir().unwrap();
             let config_path = temp_dir.path().join("test_config.toml"); 
             let repo_base = temp_dir.path().join("repos_for_delete");
             fs::create_dir_all(&repo_base).unwrap();
             let repo_local_path = repo_base.join("repo_to_delete");
             fs::create_dir_all(&repo_local_path).unwrap();
             fs::write(repo_local_path.join("dummy.txt"), "test").unwrap();
             assert!(repo_local_path.exists());

             let mut config = default_test_config();
             config.repositories_base_path = Some(repo_base.to_string_lossy().into_owned());
             config.repositories.push(RepositoryConfig {
                 name: "repo_to_delete".to_string(),
                 url: "some_url".to_string(),
                 local_path: repo_local_path.clone(),
                 default_branch: "main".to_string(),
                 tracked_branches: vec![],
                 active_branch: None,
                 remote_name: None,
                 ssh_key_path: None,
                 ssh_key_passphrase: None,
                 last_synced_commits: HashMap::new(),
                 indexed_languages: None,
                 added_as_local_path: false,
                 target_ref: None,
             });
             config.active_repository = None;
             save_config(&config, Some(&config_path)).unwrap();
             let initial_repo_count = config.repositories.len();

             let remove_args = RemoveRepoArgs { 
                 name: "repo_to_delete".to_string(), 
                 yes: true,
                 delete_local: true
             }; 
             let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));

             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&config_path)).await;
             assert!(result.is_ok());
             
             let saved_config = load_config(Some(&config_path)).unwrap();
             assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
             assert!(!saved_config.repositories.iter().any(|r| r.name == "repo_to_delete"));

             assert!(!repo_local_path.exists(), "Local repository directory was not deleted");
         });
      }

       #[test]
       fn test_handle_repo_remove_nonexistent() {
          let rt = Runtime::new().unwrap();
          rt.block_on(async {
              let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
              let temp_dir = tempdir().unwrap();
              let temp_path = temp_dir.path().join("test_config.toml");

              let mut config = create_test_config_data();
              save_config(&config, Some(&temp_path)).unwrap();
              let initial_config_state = config.clone();

              let remove_args = RemoveRepoArgs { 
                  name: "repo3".to_string(), 
                  yes: true,
                  delete_local: false
              }; 
              let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));

              let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
              assert!(result.is_err());
              assert!(result.unwrap_err().to_string().contains("Configuration for repository 'repo3' not found."));
              
              let saved_config = load_config(Some(&temp_path)).unwrap();
              assert_eq!(saved_config.repositories, initial_config_state.repositories);
              assert_eq!(saved_config.active_repository, initial_config_state.active_repository);

          });
      }

     #[test]
     fn test_handle_repo_list() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().join("list_config.toml");
        let config = create_test_config_data();
        save_config(&config, Some(&temp_path)).unwrap();
        
        let list_args = list::ListArgs { json: false }; 
        let result = list::list_repositories(&config, list_args);
        assert!(result.is_ok());
     }
} 