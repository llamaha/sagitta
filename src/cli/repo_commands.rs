// src/cli/repo_commands.rs
pub mod list; // Make public for testing
pub mod r#use; // Make public for testing
pub mod clear;
pub mod query;
pub mod sync;
pub mod use_branch;
pub mod remove;
pub mod config; // Add new config module

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::{path::PathBuf, sync::Arc};

use crate::cli::commands::CliArgs;
// Use config types from the core library (with underscore)
use vectordb_core::{AppConfig, save_config, embedding::EmbeddingHandler, config::get_repo_base_path};
use vectordb_core::qdrant_client_trait::QdrantClientTrait; // Use core trait
use std::fmt::Debug;
// Moved mockall imports inside #[cfg(test)]
// use mockall::{automock, predicate::*};
// use qdrant_client::qdrant::{Condition, Filter, PointId, PointsSelector, points_selector::PointsSelectorOneOf, PointsIdsList};
// // use vectordb_core::qdrant_client_trait::MockQdrantClientTrait; // Commented out - may be unused or moved
// use vectordb_core::config::RepositoryConfig;

const COLLECTION_NAME_PREFIX: &str = "repo_";
pub(crate) const FIELD_BRANCH: &str = "branch";
pub(crate) const FIELD_COMMIT_HASH: &str = "commit_hash";

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoCommand,
}

#[derive(Args, Debug, Clone)]
pub struct ListArgs {
    /// Output the list of repositories in JSON format.
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
#[derive(Clone)]
pub enum RepoCommand {
    /// Add a new repository to manage.
    Add(vectordb_core::repo_add::AddRepoArgs),
    /// List managed repositories.
    List(ListArgs),
    /// Set the active repository for commands.
    Use(r#use::UseRepoArgs),
    /// Remove a managed repository (config and index).
    Remove(remove::RemoveRepoArgs),
    /// Clear the index for a repository.
    Clear(clear::ClearRepoArgs),
    /// Checkout a branch and set it as active for the current repository.
    UseBranch(use_branch::UseBranchArgs),
    /// Query the index for a specific repository.
    Query(query::RepoQueryArgs),
    /// Fetch updates and sync the index for the current/specified repository.
    Sync(sync::SyncRepoArgs),
    /// Show statistics about the vector database collection for a repository.
    Stats(super::stats::StatsArgs),
    /// Configure repository settings.
    Config(config::ConfigArgs),
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
            // --- Prepare arguments for the refactored handle_repo_add --- 
            
            // 1. Initialize EmbeddingHandler and get dimension
            //    Note: This might require error handling if ONNX paths are not set
            let embedding_handler = EmbeddingHandler::new(config)
                 .context("Failed to initialize embedding handler (check ONNX config)")?;
            let embedding_dim = embedding_handler.dimension()
                 .context("Failed to get embedding dimension")?;

            // 2. Determine repo base path (prioritize args over config)
            let repo_base_path = match &add_args.repositories_base_path {
                Some(path) => path.clone(),
                None => get_repo_base_path(Some(config))
                            .context("Failed to determine repository base path")?,
            };
            // Ensure base path exists
            std::fs::create_dir_all(&repo_base_path)
                 .with_context(|| format!("Failed to create base directory: {}", repo_base_path.display()))?;

            // 3. Call the refactored function from vectordb_core
            let repo_config_result = vectordb_core::repo_add::handle_repo_add(
                add_args, 
                repo_base_path, // Pass determined base path
                embedding_dim as u64, // Pass dimension
                Arc::clone(&client)
            ).await;
            
            // Handle the result as before
            match repo_config_result {
                Ok(new_repo_config) => {
                    // Logic to update the main config if add succeeds
                    config.repositories.push(new_repo_config.clone());
                    // Optionally set as active? Depends on desired behavior.
                    // config.active_repository = Some(new_repo_config.name);
                    save_config(config, override_path)?;
                    Ok(())
                },
                Err(e) => Err(anyhow::Error::from(e)), // Propagate AddRepoError as anyhow::Error
            }
        },
        RepoCommand::List(list_args) => {
            list::list_repositories(&config, list_args.json)?;
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
            query::handle_repo_query(query_args, &config, Arc::clone(&client), cli_args).await?;
            Ok(())
        },
        RepoCommand::Sync(sync_args) => {
            sync::handle_repo_sync(sync_args, cli_args, config, client.clone(), override_path).await?;
            Ok(())
        },
        RepoCommand::Stats(stats_args) => {
            super::stats::handle_stats(stats_args, config.clone(), Arc::clone(&client)).await?;
            Ok(())
        },
        RepoCommand::Config(config_args) => {
            config::handle_config(config_args, config, override_path)?;
            Ok(())
        },
    };

    // Evaluate the result of the match arm
    command_result
}

// Helper function for tests - allows access to the list_repositories function
#[cfg(test)]
pub fn handle_repo_command_test(config: &AppConfig) -> Result<()> {
    list::list_repositories(config, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::repo_commands::RepoArgs; 
    use vectordb_core::config::{AppConfig, RepositoryConfig, IndexingConfig, load_config, save_config};
    use std::path::PathBuf;
    // use mockall::predicate::*;
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
    // Remove import for MockQdrantClientTrait as it seems unresolved/unused
    // use vectordb_core::qdrant_client_trait::MockQdrantClientTrait; 
    use crate::cli::CliArgs; 

    // Helper function to create a default AppConfig for tests
    fn default_test_config() -> AppConfig {
        let temp_dir = tempdir().expect("Failed to create temp dir for test config");
        let base_path = temp_dir.path().join("repositories");
        fs::create_dir_all(&base_path).unwrap();
        let vocab_base = temp_dir.path().join("vocab_def"); // Unique name
        fs::create_dir_all(&vocab_base).unwrap();
        let config = AppConfig {
            qdrant_url: "http://localhost:6333".to_string(),
            onnx_model_path: None, // Correct field name
            onnx_tokenizer_path: None, // Correct field name
            server_api_key_path: None, // Correct field name
            repositories_base_path: Some(base_path.to_string_lossy().into_owned()), // Use temp path
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()), // Added temp vocab path
            repositories: vec![], // Added default
            active_repository: None, // Added default
            indexing: Default::default(), // Added default
        };
        config
    }

    // Helper function to create a default AppConfig for tests
    fn create_test_config_data() -> AppConfig {
        // Use tempdir for base paths in test config data
        let temp_dir = tempdir().expect("Failed to create temp dir for test config data");
        let repo_base = temp_dir.path().join("repos");
        fs::create_dir_all(&repo_base).unwrap();
        let vocab_base = temp_dir.path().join("vocab_data");
        fs::create_dir_all(&vocab_base).unwrap();
        let model_base = temp_dir.path().join("models"); // For dummy paths
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
            qdrant_url: "http://localhost:6334".to_string(), // Keep distinct port for tests if needed
            onnx_model_path: Some(dummy_model.to_string_lossy().into_owned()), // Correct field name
            onnx_tokenizer_path: Some(dummy_tokenizer_dir.to_string_lossy().into_owned()), // Correct field name
            server_api_key_path: None, // Correct field name
            repositories_base_path: Some(repo_base.to_string_lossy().into_owned()), // Use temp path
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()), // Use temp path
            indexing: IndexingConfig::default(),
        }
    }

    // Helper function to create dummy CliArgs
     fn create_dummy_cli_args(repo_command: RepoCommand) -> CliArgs {
        // Add default dummy paths for ONNX, tests needing real paths should override
        let dummy_model_path = Some(PathBuf::from("/tmp/dummy_model.onnx"));
        let dummy_tokenizer_dir = Some(PathBuf::from("/tmp/dummy_tokenizer/"));

        CliArgs {
             command: Commands::Repo(RepoArgs { command: repo_command }),
             // Convert PathBuf to String
             onnx_model_path_arg: dummy_model_path.map(|p| p.to_string_lossy().into_owned()),
             onnx_tokenizer_dir_arg: dummy_tokenizer_dir.map(|p| p.to_string_lossy().into_owned()),
         }
      }

    // Helper to create a unique suffix for collections/repos
    fn unique_suffix() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis()
            .to_string()
    }

    // --- Updated Tests --- 
    // Note: repo clear tests might still need Qdrant connection or mocking
    // They don't save config, so isolation isn't strictly needed for that
    #[test]
    fn test_handle_repo_clear_specific_repo() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let suffix = unique_suffix();
            let test_repo_name = format!("clear-specific-{}", suffix);
            let collection_name = format!("repo_{}", test_repo_name);
            // Use tempdir for config isolation
            let temp_dir = tempdir().unwrap();
            let temp_path = temp_dir.path().join("clear_specific_config.toml");

            // Setup config and save initial state to temp path
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
                 last_synced_commits: HashMap::from([("main".to_string(), "dummy_commit".to_string())]), // Add some dummy sync state
                 indexed_languages: Some(vec!["rust".to_string()]),
                 added_as_local_path: false,
                 target_ref: None,
            });
            config.active_repository = Some("other_repo".to_string()); // Ensure it clears the specified one
            save_config(&config, Some(&temp_path)).unwrap(); // Save initial state
            
            let args = clear::ClearRepoArgs { name: Some(test_repo_name.to_string()), yes: true };
            let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));

            let client = Arc::new(Qdrant::from_url(&config.qdrant_url).build().expect("Failed to create Qdrant client"));

            // Pre-cleanup just in case
            let _ = client.delete_collection(&collection_name).await;

            // Execute command, passing the override path
            let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
            
            assert!(result.is_ok(), "handle_repo_command failed: {:?}", result.err());

            // --- DEBUG ---
            // dbg!(&config.repositories.iter().find(|r| r.name == test_repo_name)); // REMOVE THIS
            // --- END DEBUG ---

            // Verify by loading from the temporary file
            let saved_config = load_config(Some(&temp_path)).unwrap();
            let updated_repo = saved_config.repositories.iter().find(|r| r.name == test_repo_name).expect("Test repo config not found after clear");
            assert!(updated_repo.last_synced_commits.is_empty(), "Sync status was not cleared");
            assert!(updated_repo.indexed_languages.is_none(), "Indexed languages were not cleared");
        
            // Cleanup
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
             // Use tempdir for config isolation
             let temp_dir = tempdir().unwrap();
             let temp_path = temp_dir.path().join("clear_active_config.toml");

             // Setup config and save initial state to temp path
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
             save_config(&config, Some(&temp_path)).unwrap(); // Save initial state

             let args = clear::ClearRepoArgs { name: None, yes: true }; // No name uses active
             let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));

             let client = Arc::new(Qdrant::from_url(&config.qdrant_url).build().expect("Failed to create Qdrant client"));

             // Pre-cleanup just in case
             let _ = client.delete_collection(&collection_name).await;

             // Execute command, passing the override path
             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
             
             assert!(result.is_ok(), "handle_repo_command failed: {:?}", result.err());

             // Verify by loading from the temporary file
             let saved_config = load_config(Some(&temp_path)).unwrap();
             let updated_repo = saved_config.repositories.iter().find(|r| r.name == active_repo_name).expect("Active repo config not found after clear");
             assert!(updated_repo.last_synced_commits.is_empty(), "Sync status was not cleared for active repo");
             assert!(updated_repo.indexed_languages.is_none(), "Indexed languages were not cleared for active repo");
         
             // Cleanup
             let _ = client.delete_collection(&collection_name).await;
         });
    }
    #[test]
    fn test_handle_repo_clear_no_active_or_specified_fails() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            // Use create_test_config_data directly
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
         let temp_dir = tempdir().unwrap(); // Use tempdir
         let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

         let mut config = create_test_config_data();
         config.active_repository = Some("repo1".to_string());
         save_config(&config, Some(&temp_path)).unwrap(); // Save initial state to temp path

         let use_args = r#use::UseRepoArgs { name: "repo2".to_string() };
         let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); 
         let dummy_cli_args = create_dummy_cli_args(RepoCommand::Use(use_args.clone()));

         // Pass Some(&temp_path) as override_path
         let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
              handle_repo_command(RepoArgs{ command: RepoCommand::Use(use_args)}, &dummy_cli_args, &mut config, client, Some(&temp_path)).await
         });
         assert!(result.is_ok());
         
         // Verify by loading from the temporary file
         let saved_config = load_config(Some(&temp_path)).unwrap();
         assert_eq!(saved_config.active_repository, Some("repo2".to_string()));

         // Keep temp_dir alive until end of test scope automatically
     }

     #[test]
     fn test_handle_repo_use_nonexistent() {
        let temp_dir = tempdir().unwrap(); // Use tempdir
        let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

        let mut config = create_test_config_data();
        save_config(&config, Some(&temp_path)).unwrap();
        let initial_config_state = config.clone(); // Save for comparison
        
        let use_args = r#use::UseRepoArgs { name: "repo3".to_string() }; 
        let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); 
        let dummy_cli_args = create_dummy_cli_args(RepoCommand::Use(use_args.clone()));

        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
              handle_repo_command(RepoArgs{ command: RepoCommand::Use(use_args)}, &dummy_cli_args, &mut config, client, Some(&temp_path)).await
         });
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository 'repo3' not found"));

        // Verify config file was NOT changed because the command errored before saving
        let saved_config = load_config(Some(&temp_path)).unwrap();
        assert_eq!(saved_config.repositories, initial_config_state.repositories);
        assert_eq!(saved_config.active_repository, initial_config_state.active_repository);

        // Keep temp_dir alive until end of test scope automatically
     }

     #[test]
     fn test_handle_repo_remove_config_only_non_active() {
        let rt = Runtime::new().unwrap();
         rt.block_on(async {
             let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
             let temp_dir = tempdir().unwrap(); // Use tempdir
             let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

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
             let _ = fs::remove_dir_all("/tmp/vectordb_test_repo2"); // Keep dummy dir removal

             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
             assert!(result.is_ok());
             
             // Verify by loading from the temporary file
             let saved_config = load_config(Some(&temp_path)).unwrap();
             assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
             assert!(!saved_config.repositories.iter().any(|r| r.name == "repo2"));
             assert_eq!(saved_config.active_repository, Some("repo1".to_string()));

             // Keep temp_dir alive until end of test scope automatically
         });
     }

      #[test]
      fn test_handle_repo_remove_config_only_active() {
         let rt = Runtime::new().unwrap();
          rt.block_on(async {
              let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
              let temp_dir = tempdir().unwrap(); // Use tempdir
              let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

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
              
              // Verify by loading from the temporary file
              let saved_config = load_config(Some(&temp_path)).unwrap();
              assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
              assert!(!saved_config.repositories.iter().any(|r| r.name == "repo2"));
              assert_eq!(saved_config.active_repository, None, "Active repository should be None after removal");

              // Keep temp_dir alive until end of test scope automatically
          });
      }

       #[test]
       fn test_handle_repo_remove_nonexistent() {
          let rt = Runtime::new().unwrap();
           rt.block_on(async {
               let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
               let temp_dir = tempdir().unwrap(); // Use tempdir
               let temp_path = temp_dir.path().join("test_config.toml"); // Define path within tempdir

               let mut config = create_test_config_data();
               save_config(&config, Some(&temp_path)).unwrap();
               let initial_config_state = config.clone();
               let initial_repo_count = config.repositories.len();

               let remove_args = RemoveRepoArgs { 
                   name: "repo3".to_string(), 
                   yes: true,
                   delete_local: false
               }; 
               let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));

               let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
               assert!(result.is_err());
               assert!(result.unwrap_err().to_string().contains(&format!("Configuration for repository '{}' not found", "repo3")));

               // Verify config file was NOT changed
               let saved_config = load_config(Some(&temp_path)).unwrap();
               assert_eq!(saved_config.repositories, initial_config_state.repositories);
               assert_eq!(saved_config.repositories.len(), initial_repo_count);

               // Keep temp_dir alive until end of test scope automatically
           });
       }

    // Test for handling the List command
    #[test]
    fn test_handle_repo_list() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); // Mock or real client
            let mut config = create_test_config_data();
            config.active_repository = Some("repo1".to_string());

            // Define ListArgs
            let list_args = ListArgs { json: false }; 

            // Create dummy CliArgs with the correct RepoCommand structure
            let dummy_cli_args = create_dummy_cli_args(RepoCommand::List(list_args.clone())); 

            // Create RepoArgs containing the List command with ListArgs
            let repo_args = RepoArgs { command: RepoCommand::List(list_args) };
            
            // Execute handle_repo_command
            let result = handle_repo_command(repo_args, &dummy_cli_args, &mut config, client, None).await;
            
            // Assertions (basic check that it runs)
            assert!(result.is_ok(), "handle_repo_command failed for List: {:?}", result.err());
            // Add more specific assertions based on expected output capture if needed
        });
    }

    // TODO: Add tests for sync_repository, especially for the extension filter.
    // #[tokio::test]
    // async fn test_sync_with_extension_filter() { ... }

    // #[tokio::test]
    // async fn test_sync_without_extension_filter() { ... }

    // #[tokio::test]
    // async fn test_sync_with_invalid_extension_filter() { ... }
}

