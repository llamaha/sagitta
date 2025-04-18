// src/cli/repo_commands.rs
pub mod list; // Make public for testing
pub mod r#use; // Make public for testing
pub mod clear;
pub mod query;
pub mod sync;
pub mod use_branch;
pub mod add;
pub mod remove;
pub mod helpers; // Make public
pub mod config; // Add new config module

use anyhow::Result;
use clap::{Args, Subcommand};
use qdrant_client::Qdrant;
use std::{path::PathBuf, sync::Arc};

use crate::config::AppConfig;
use crate::cli::commands::CliArgs;

const COLLECTION_NAME_PREFIX: &str = "repo_";
pub(crate) const FIELD_BRANCH: &str = "branch";
pub(crate) const FIELD_COMMIT_HASH: &str = "commit_hash";

// Public functions for server use
pub use add::handle_repo_add as add_repository;
pub use r#use::use_repository as set_active_repo;
pub use remove::handle_repo_remove as remove_repository;
pub use sync::handle_repo_sync as sync_repository;
pub use use_branch::handle_use_branch as use_branch;
pub use list::get_managed_repos;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct RepoArgs {
    #[command(subcommand)]
    command: RepoCommand,
}

#[derive(Subcommand, Debug)]
#[derive(Clone)]
enum RepoCommand {
    /// Add a new repository to manage.
    Add(add::AddRepoArgs),
    /// List managed repositories.
    List,
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

pub async fn handle_repo_command(
    args: RepoArgs,
    cli_args: &CliArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    match args.command {
        RepoCommand::Add(add_args) => add::handle_repo_add(add_args, config, client, override_path).await,
        RepoCommand::List => list::list_repositories(config),
        RepoCommand::Use(use_args) => r#use::use_repository(use_args, config, override_path),
        RepoCommand::Remove(remove_args) => remove::handle_repo_remove(remove_args, config, client, override_path).await,
        RepoCommand::Clear(clear_args) => clear::handle_repo_clear(clear_args, config, client, override_path).await,
        RepoCommand::UseBranch(branch_args) => use_branch::handle_use_branch(branch_args, config, override_path).await,
        RepoCommand::Query(query_args) => query::handle_repo_query(query_args, config, client, cli_args).await,
        RepoCommand::Sync(sync_args) => sync::handle_repo_sync(sync_args, cli_args, config, client, override_path).await,
        RepoCommand::Stats(stats_args) => super::stats::handle_stats(stats_args, config.clone(), client).await,
        RepoCommand::Config(config_args) => config::handle_config(config_args, config, override_path),
    }
}

// Helper function for tests - allows access to the list_repositories function
#[cfg(test)]
pub fn handle_repo_command_test(config: &AppConfig) -> Result<()> {
    list::list_repositories(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, RepositoryConfig, load_config, save_config}; 
    use crate::cli::commands::Commands;
    use crate::cli::repo_commands::{RepoArgs, RepoCommand};
    use crate::cli::repo_commands::remove::RemoveRepoArgs;
    use qdrant_client::{Qdrant};
    use std::sync::Arc;
    use tokio::runtime::Runtime;
    use std::collections::HashMap;
    use std::path::{PathBuf};
    use std::fs;
    use tempfile::{tempdir};

    // Helper function to create a default AppConfig for tests
    fn create_test_config_data() -> AppConfig {
        AppConfig {
            repositories: vec![
                RepositoryConfig { name: "repo1".to_string(), url: "url1".to_string(), local_path: PathBuf::from("/tmp/vectordb_test_repo1"), default_branch: "main".to_string(), tracked_branches: vec!["main".to_string()], active_branch: Some("main".to_string()), remote_name: Some("origin".to_string()), ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None },
                RepositoryConfig { name: "repo2".to_string(), url: "url2".to_string(), local_path: PathBuf::from("/tmp/vectordb_test_repo2"), default_branch: "dev".to_string(), tracked_branches: vec!["dev".to_string()], active_branch: Some("dev".to_string()), remote_name: Some("origin".to_string()), ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None },
            ],
            active_repository: None,
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            repositories_base_path: None,
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

    // --- Updated Tests --- 
    // Note: repo clear tests might still need Qdrant connection or mocking
    // They don't save config, so isolation isn't strictly needed for that
    #[test]
    fn test_handle_repo_clear_specific_repo() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
            // Use create_test_config_data directly, no need for temp file here
            let mut config = create_test_config_data(); 
            let test_repo_name = "my-test-repo-clear-specific"; 
             config.repositories.push(RepositoryConfig { name: test_repo_name.to_string(), /* .. other fields .. */ url: "url_clear".to_string(), local_path: PathBuf::from("/tmp/clear_spec"), default_branch: "main".to_string(), tracked_branches: vec![], active_branch: None, remote_name: None, ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None});
             config.active_repository = Some("repo1".to_string()); 
            
            let args = clear::ClearRepoArgs { name: Some(test_repo_name.to_string()), yes: true };
            let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));
            let _ = client.delete_collection(&helpers::get_collection_name(test_repo_name)).await; 

            let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client, None).await;
            assert!(result.is_ok());
        });
    }
    #[test]
    fn test_handle_repo_clear_active_repo() {
         let rt = Runtime::new().unwrap();
         rt.block_on(async {
             let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap());
             // Use create_test_config_data directly
             let mut config = create_test_config_data(); 
             let active_repo_name = "my-test-repo-clear-active"; 
             config.repositories.push(RepositoryConfig { name: active_repo_name.to_string(), /* .. other fields .. */ url: "url_clear_active".to_string(), local_path: PathBuf::from("/tmp/clear_active"), default_branch: "main".to_string(), tracked_branches: vec![], active_branch: None, remote_name: None, ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None});
             config.active_repository = Some(active_repo_name.to_string());

             let args = clear::ClearRepoArgs { name: None, yes: true }; // Clear active
             let dummy_cli_args = create_dummy_cli_args(RepoCommand::Clear(args.clone()));
             let _ = client.delete_collection(&helpers::get_collection_name(active_repo_name)).await;

             let result = handle_repo_command(RepoArgs{ command: RepoCommand::Clear(args)}, &dummy_cli_args, &mut config, client, None).await;
             assert!(result.is_ok());

             // Add assertion for config state change if desired (e.g., sync status cleared)
             // let updated_repo = config.repositories.iter().find(|r| r.name == active_repo_name);
             // assert!(updated_repo.is_some());
             // assert!(updated_repo.unwrap().last_synced_commits.is_empty());
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
             
             let remove_args = RemoveRepoArgs { name: "repo2".to_string(), yes: true }; 
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
              config.repositories.push(RepositoryConfig { name: "repo3".to_string(), url: "url3".to_string(), local_path: PathBuf::from("/tmp/vectordb_test_repo3"), default_branch: "main".to_string(), tracked_branches: vec!["main".to_string()], active_branch: Some("main".to_string()), remote_name: Some("origin".to_string()), ssh_key_path: None, ssh_key_passphrase: None, last_synced_commits: HashMap::new(), indexed_languages: None });
              save_config(&config, Some(&temp_path)).unwrap();
              let initial_repo_count = config.repositories.len();

              let remove_args = RemoveRepoArgs { name: "repo2".to_string(), yes: true };
              let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));
              let _ = fs::remove_dir_all("/tmp/vectordb_test_repo2");

              let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
              assert!(result.is_ok());
              
              // Verify by loading from the temporary file
              let saved_config = load_config(Some(&temp_path)).unwrap();
              assert_eq!(saved_config.repositories.len(), initial_repo_count - 1);
              assert!(!saved_config.repositories.iter().any(|r| r.name == "repo2"));
              assert_eq!(saved_config.active_repository, Some("repo1".to_string())); // Should switch to repo1

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

               let remove_args = RemoveRepoArgs { name: "repo3".to_string(), yes: true }; 
               let dummy_cli_args = create_dummy_cli_args(RepoCommand::Remove(remove_args.clone()));

               let result = handle_repo_command(RepoArgs{ command: RepoCommand::Remove(remove_args)}, &dummy_cli_args, &mut config, client.clone(), Some(&temp_path)).await;
               assert!(result.is_err());
               assert!(result.unwrap_err().to_string().contains("Repository 'repo3' not found"));

               // Verify config file was NOT changed
               let saved_config = load_config(Some(&temp_path)).unwrap();
               assert_eq!(saved_config.repositories, initial_config_state.repositories);
               assert_eq!(saved_config.repositories.len(), initial_repo_count);

               // Keep temp_dir alive until end of test scope automatically
           });
       }

    // Keep repo list test as is, it doesn't save config
    #[test]
    fn test_handle_repo_list() {
        // Setup config
        let mut config = create_test_config_data();
        config.active_repository = Some("repo1".to_string());

        // Call list_repositories directly or via handle_repo_command
        // Since list doesn't modify/save, override_path isn't strictly needed, but let's pass None for consistency
        let list_args = RepoArgs { command: RepoCommand::List };
        let dummy_cli_args = create_dummy_cli_args(RepoCommand::List);
        let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); // Dummy client needed for handle_repo_command signature

        let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
             handle_repo_command(list_args, &dummy_cli_args, &mut config, client, None).await // Pass None
        });

        // List command prints to stdout, so we'd typically capture stdout to assert output
        // For now, just assert it runs without error
        assert!(result.is_ok());
    }

    // TODO: Add tests for sync_repository, especially for the extension filter.
    // #[tokio::test]
    // async fn test_sync_with_extension_filter() { ... }

    // #[tokio::test]
    // async fn test_sync_without_extension_filter() { ... }

    // #[tokio::test]
    // async fn test_sync_with_invalid_extension_filter() { ... }
}

