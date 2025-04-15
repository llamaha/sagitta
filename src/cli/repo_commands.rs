// src/cli/repo_commands.rs
use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use git2::{Repository, DiffOptions, DiffFindOptions, Delta, Cred, RemoteCallbacks, FetchOptions, CredentialType};
use qdrant_client::{
    qdrant::{ CollectionStatus, CreateCollectionBuilder, Distance, FieldType, VectorParamsBuilder, Filter, Condition, DeletePointsBuilder, PointStruct },
    Payload,
    Qdrant,
};
use std::{fs, path::PathBuf, sync::Arc, time::Duration, collections::HashSet, collections::HashMap};
use uuid::Uuid;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};

use crate::config::{self, AppConfig};
use crate::cli::commands::{
    ensure_payload_index, upsert_batch, CliArgs, FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, 
    FIELD_LANGUAGE, FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_FILE_EXTENSION, BATCH_SIZE
};
use crate::{syntax, vectordb::{embedding, embedding_logic::EmbeddingHandler}};

pub(crate) const DEFAULT_VECTOR_DIMENSION: u64 = 384;
const COLLECTION_NAME_PREFIX: &str = "repo_";

pub(crate) const FIELD_BRANCH: &str = "branch";
pub(crate) const FIELD_COMMIT_HASH: &str = "commit_hash";

// Helper function to check if a file extension is explicitly supported by a parser
fn is_supported_extension(extension: &str) -> bool {
    matches!(extension.to_lowercase().as_str(), 
        "rs" | "rb" | "go" | "js" | "jsx" | "ts" | "tsx" | "yaml" | "yml" | "md" | "mdx" | "py"
    )
}

// Helper to create FetchOptions with SSH credential callback
fn create_fetch_options<'a>(config: &'a AppConfig, repo_url: &'a str) -> Result<FetchOptions<'a>> {
    let mut callbacks = RemoteCallbacks::new();

    // Clone necessary config data for the callback closure
    // We need to find the RepositoryConfig corresponding to the URL being fetched
    let relevant_repo_config = config.repositories.iter()
        .find(|r| r.url == repo_url)
        .cloned(); // Clone the config for the closure
    
    callbacks.credentials(move |_url, username_from_git, allowed_types| {
        log::debug!("Credential callback triggered. URL: {}, Username: {:?}, Allowed: {:?}", _url, username_from_git, allowed_types);

        let repo_config = match &relevant_repo_config {
            Some(conf) => conf,
            None => {
                log::error!("Could not find repository config for URL '{}' in credential callback.", _url);
                 return Err(git2::Error::from_str("Repository config not found for credential callback"));
            }
        };

        // --- SSH Key Handling ---
        if allowed_types.contains(CredentialType::SSH_KEY) {
             if let Some(key_path) = &repo_config.ssh_key_path {
                 let user = username_from_git.unwrap_or("git"); // Default to 'git' user for SSH if not specified
                 log::debug!("Attempting SSH key authentication. User: '{}', Key Path: {}", user, key_path.display());
                 match Cred::ssh_key(
                     user,                 // username
                     None,                 // pubkey_path (libgit2 finds it based on privkey)
                     key_path,             // privkey_path
                     repo_config.ssh_key_passphrase.as_deref(), // passphrase
                 ) {
                     Ok(cred) => {
                         log::info!("SSH key credential created successfully for user '{}'.", user);
                         return Ok(cred);
                     }
                     Err(e) => {
                         log::error!("Failed to create SSH key credential from path {}: {}", key_path.display(), e);
                         // Don't return error immediately, maybe another method will work
                     }
                 }
             } else {
                log::debug!("SSH key requested, but no ssh_key_path configured for repo '{}'", repo_config.name);
             }
        }

        // --- TODO: Add other credential types (e.g., USERPASS_PLAINTEXT, DEFAULT) here later --- 

        // If no specific credential worked or was configured, try default system credentials
        if allowed_types.contains(CredentialType::DEFAULT) {
            log::debug!("Attempting default system credentials.");
            match Cred::default() {
                Ok(cred) => {
                    log::info!("Using default system credentials.");
                    return Ok(cred);
                }
                Err(e) => {
                    log::warn!("Failed to get default system credentials: {}", e);
                }
            }
        }
        
        log::error!("No suitable credentials found or configured for URL '{}', user '{:?}'", _url, username_from_git);
        Err(git2::Error::from_str("Authentication failed: no suitable credentials found"))
    });

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    Ok(fetch_opts)
}

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
    Add(AddRepoArgs),
    /// List managed repositories.
    List,
    /// Set the active repository for commands.
    Use(UseRepoArgs),
    /// Remove a managed repository (config and index).
    Remove(RemoveRepoArgs),
    /// Clear the index for a repository.
    Clear(ClearRepoArgs),
    /// Checkout a branch and set it as active for the current repository.
    UseBranch(UseBranchArgs),
    /// Fetch updates and sync the index for the current/specified repository.
    Sync(SyncRepoArgs),
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct AddRepoArgs {
    #[arg(short = 'p', long, value_parser)]
    /// Local path where the repository should be cloned or already exists.
    local_path: Option<PathBuf>,

    #[clap(long, value_parser)]
    /// URL of the repository to clone (e.g., https://gitlab.com/user/repo.git)
    url: String,

    /// Optional custom name for the repository (defaults to deriving from URL).
    #[arg(short, long)]
    name: Option<String>,

    /// Optional specific branch to track initially (defaults to the repo's default).
    #[arg(short, long)]
    branch: Option<String>,

    /// Optional name for the Git remote (defaults to "origin").
    #[arg(short = 'r', long)]
    remote: Option<String>,

    /// Optional path to the SSH private key for authentication.
    #[arg(long)]
    ssh_key: Option<PathBuf>,

    /// Optional passphrase for the SSH private key.
    #[arg(long, requires = "ssh_key")] // Passphrase only makes sense if key is provided
    ssh_passphrase: Option<String>,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct UseRepoArgs {
    /// Name of the repository to set as active.
    name: String,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct RemoveRepoArgs {
    /// Name of the repository to remove.
    name: String,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct UseBranchArgs {
    /// Name of the branch to checkout and set active.
    name: String,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct SyncRepoArgs {
    /// Optional name of the repository to sync (defaults to active repository).
    name: Option<String>,

    // Note: ONNX paths are handled globally via CliArgs, no need to duplicate here.
    // These flags might be useful later if sync needs specific overrides.
    // /// Path to ONNX model file (overrides global/config)
    // #[arg(short = 'm', long)]
    // onnx_model: Option<String>,

    // /// Path to ONNX tokenizer config directory (overrides global/config)
    // #[arg(short = 't', long)]
    // onnx_tokenizer_dir: Option<String>,
}

#[derive(Args, Debug)]
#[derive(Clone)]
struct ClearRepoArgs {
    /// Optional name of the repository index to clear (defaults to active repository).
    name: Option<String>,

    /// Confirm deletion without prompting.
    #[arg(short, long)]
    yes: bool,
}

pub(crate) fn get_collection_name(repo_name: &str) -> String {
    format!("{}{}", COLLECTION_NAME_PREFIX, repo_name)
}

pub async fn handle_repo_command(
    args: RepoArgs,
    cli_args: &CliArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>,
) -> Result<()> {
    match args.command {
        RepoCommand::Add(add_args) => add_repository(add_args, config, client).await,
        RepoCommand::List => list_repositories(config),
        RepoCommand::Use(use_args) => use_repository(use_args, config),
        RepoCommand::Remove(remove_args) => remove_repository(remove_args, config, client).await,
        RepoCommand::Clear(clear_args) => handle_repo_clear(clear_args, config, client).await,
        RepoCommand::UseBranch(branch_args) => use_branch(branch_args, config).await,
        RepoCommand::Sync(sync_args) => sync_repository(sync_args, cli_args, config, client).await,
    }
}

async fn add_repository(
    args: AddRepoArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>
) -> Result<()> {
    let repo_name = match args.name {
        Some(name) => name,
        None => PathBuf::from(&args.url)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".git").to_string())
            .ok_or_else(|| anyhow!("Could not derive repository name from URL"))?,
    };

    if config.repositories.iter().any(|r| r.name == repo_name) {
        bail!("Repository '{}' already exists.", repo_name);
    }

    let repo_base_path = config::get_repo_base_path()?;
    fs::create_dir_all(&repo_base_path)
        .with_context(|| format!("Failed to create repository base directory at {}", repo_base_path.display()))?;
    let local_path = args.local_path.unwrap_or(repo_base_path.join(&repo_name));

    let repo = if local_path.exists() {
         println!(
            "{}",
            format!(
                "Local directory '{}' already exists. Assuming it's the intended clone. Skipping clone.",
                local_path.display()
            ).yellow()
        );
        // We could potentially validate if it's a git repo and matches the URL, but let's keep it simple for now.
        // Need to open the existing repo to get its details
        Repository::open(&local_path)
            .with_context(|| format!("Failed to open existing repository at {}", local_path.display()))?
    } else {
        println!("Cloning repository '{}' from {}...", repo_name.cyan(), args.url.cyan());
        // Define FetchOptions and RemoteCallbacks for potential authentication
        let fetch_opts = create_fetch_options(&config, &args.url)?; // Assuming create_fetch_options exists and is suitable

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        let repo = builder.clone(&args.url, &local_path)
            .with_context(|| format!("Failed to clone repository from {}", args.url))?;
        println!("Repository cloned successfully to {}", local_path.display());
        repo
    };

    // --- Determine and set the default/initial branch ---
    let initial_branch_name = match args.branch {
        Some(branch_name) => branch_name,
        None => {
            // Find the symbolic reference HEAD usually points to (e.g., refs/heads/main)
            let head_ref = repo.find_reference("HEAD")?;
            let head_ref_resolved = head_ref.resolve()?; // Resolve symbolic ref to direct ref
            head_ref_resolved.shorthand()
                .ok_or_else(|| anyhow!("Could not determine default branch name from HEAD"))?
                .to_string()
        }
    };
    println!("Default/Initial branch detected: {}", initial_branch_name.cyan());


    let collection_name = get_collection_name(&repo_name);
    println!("Ensuring Qdrant collection '{}' exists...", collection_name.cyan());
    ensure_repository_collection_exists(&client, &collection_name).await?;
    println!("Qdrant collection ensured.");

    let new_repo_config = config::RepositoryConfig {
        name: repo_name.clone(),
        url: args.url.clone(),
        local_path: local_path.clone(),
        default_branch: initial_branch_name.clone(),
        tracked_branches: vec![initial_branch_name.clone()], // Start tracking the initial branch
        active_branch: Some(initial_branch_name.clone()), // Set the initial branch as active
        remote_name: Some(args.remote.unwrap_or_else(|| "origin".to_string())),
        ssh_key_path: args.ssh_key.clone(),
        ssh_key_passphrase: args.ssh_passphrase.clone(),
        last_synced_commits: HashMap::new(), // Initialize empty commit map
        indexed_languages: None,
    };

    config.repositories.push(new_repo_config);
    config.active_repository = Some(repo_name.clone()); // Set the new repo as active
    config::save_config(&config)?;

    println!("Set '{}' as the active repository.", repo_name.cyan());
    println!("{}", "Successfully added repository configuration.".green());
    println!("Run '{}' to fetch and index the '{}' branch.",
             format!("vectordb-cli repo sync {}", repo_name).cyan(),
             initial_branch_name.cyan());


    Ok(())
}

fn list_repositories(config: &AppConfig) -> Result<()> {
    if config.repositories.is_empty() {
        println!("No repositories configured yet. Use 'repo add <url>' to add one.");
        return Ok(());
    }

    println!("{}", "Managed Repositories:".bold());
    for repo in &config.repositories {
        let active_marker = if config.active_repository.as_ref() == Some(&repo.name) {
             "*".green().bold()
        } else {
            " ".normal()
        };
        println!(
            " {} {} ({}) -> {}",
            active_marker,
            repo.name.cyan().bold(),
            repo.url,
            repo.local_path.display()
        );
         println!("     Default Branch: {}", repo.default_branch);
         println!("     Tracked Branches: {:?}", repo.tracked_branches);
         // Display indexed languages if available
         if let Some(langs) = &repo.indexed_languages {
            if !langs.is_empty() {
                let mut sorted_langs = langs.clone();
                sorted_langs.sort();
                println!("     Indexed Languages: {}", sorted_langs.join(", "));
            }
         }
         // Optionally show last sync status here later
    }

    Ok(())
}

fn use_repository(args: UseRepoArgs, config: &mut AppConfig) -> Result<()> {
    if !config.repositories.iter().any(|r| r.name == args.name) {
        bail!(
            "Repository '{}' not found. Use 'repo list' to see available repositories.",
            args.name
        );
    }

    config.active_repository = Some(args.name.clone());
    config::save_config(&config)?;

    println!(
        "{}",
        format!("Set active repository to '{}'.", args.name.cyan()).green()
    );

    Ok(())
}

async fn remove_repository(
    args: RemoveRepoArgs, 
    config: &mut AppConfig,
    client: Arc<Qdrant>
) -> Result<()> {
    let repo_name = args.name;
    let repo_index = config.repositories.iter().position(|r| r.name == repo_name);

    match repo_index {
        Some(index) => {
            let repo_config = config.repositories.remove(index);
            println!("Removing repository configuration for '{}'.", repo_name.cyan());

            let collection_name = get_collection_name(&repo_config.name);
            println!("Attempting to delete Qdrant collection '{}'...", collection_name.cyan());
            match client.delete_collection(&collection_name).await {
                Ok(result) => {
                    if result.result {
                         println!("{}", format!("Successfully deleted Qdrant collection '{}'.", collection_name).green());
                    } else {
                        println!("{}", format!("Qdrant reported failure deleting collection '{}', maybe it didn't exist?", collection_name).yellow());
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("Error deleting Qdrant collection '{}': {}. Manual cleanup might be needed.", collection_name, e).red());
                }
            }

             println!(
                 "{}",
                 format!(
                     "Attempting to remove local clone at {}...",
                     repo_config.local_path.display()
                 )
                 .yellow()
             );
             match fs::remove_dir_all(&repo_config.local_path) {
                 Ok(_) => println!(
                     "{}",
                     format!(
                         "Successfully removed local directory '{}'.",
                         repo_config.local_path.display()
                     )
                     .green()
                 ),
                 Err(e) => eprintln!(
                     "{}",
                     format!(
                         "Warning: Failed to remove directory '{}': {}. Manual removal might be needed.",
                         repo_config.local_path.display(),
                         e
                     )
                     .red()
                 ),
             }

            if config.active_repository.as_ref() == Some(&repo_name) {
                config.active_repository = config.repositories.first().map(|r| r.name.clone());
                if let Some(new_active) = &config.active_repository {
                    println!("Set active repository to '{}'.", new_active.cyan());
                } else {
                     println!("{}", "No remaining repositories to set as active.".yellow());
                }
            }

            config::save_config(&config)?;
            println!(
                "{}",
                format!("Successfully removed repository '{}'.", repo_name).green()
            );
        }
        None => {
            bail!(
                "Repository '{}' not found. Use 'repo list' to see available repositories.",
                repo_name
            );
        }
    }

    Ok(())
}

async fn use_branch(args: UseBranchArgs, config: &mut AppConfig) -> Result<()> {
    let repo_name = match config.active_repository.clone() {
        Some(name) => name,
        None => bail!("No active repository set. Use 'repo use <name>' first."),
    };

    // Find the index immutably first
    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow::anyhow!("Active repository '{}' configuration not found.", repo_name))?;

    // Get immutable reference for reading URL and creating fetch options
    let repo_config = &config.repositories[repo_config_index];

    let repo = Repository::open(&repo_config.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config.local_path.display()))?;

    let target_branch_name = &args.name;
    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin");
    let repo_url = repo_config.url.clone(); // Clone URL needed for fetch options

    // Check if branch exists locally
    if repo.find_branch(target_branch_name, git2::BranchType::Local).is_err() {
        println!(
            "Local branch '{}' not found. Checking remote '{}'...",
            target_branch_name, remote_name
        );
        
        // Fetch first to ensure remote refs are up-to-date
        println!("Fetching from remote '{}' to update refs...", remote_name);
        let mut remote = repo.find_remote(remote_name)?;
        let mut fetch_opts = create_fetch_options(&config, &repo_url)?; // Pass cloned URL
        remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)
            .with_context(|| format!("Failed initial fetch from remote '{}' before branch check", remote_name))?;
        println!("Fetch for refs update complete.");

        // Try to find the branch on the configured remote again after fetch
        let remote_branch_ref = format!("{}/{}", remote_name, target_branch_name);
        match repo.find_branch(&remote_branch_ref, git2::BranchType::Remote) {
            Ok(remote_branch) => {
                println!(
                    "Branch '{}' found on remote '{}'. Creating local tracking branch...",
                    target_branch_name, remote_name
                );
                let commit = remote_branch.get().peel_to_commit()
                    .with_context(|| format!("Failed to get commit for remote branch {}", remote_branch_ref))?;
                repo.branch(target_branch_name, &commit, false)
                    .with_context(|| format!("Failed to create local branch '{}'", target_branch_name))?;
                // Set upstream
                let mut local_branch = repo.find_branch(target_branch_name, git2::BranchType::Local)?;
                local_branch.set_upstream(Some(&remote_branch_ref))
                    .with_context(|| format!("Failed to set upstream for branch '{}' to '{}'", target_branch_name, remote_branch_ref))?;
            }
            Err(_) => {
                bail!(
                    "Branch '{}' not found locally or on remote '{}'.",
                    target_branch_name,
                    remote_name
                );
            }
        }
    }

    println!("Checking out branch '{}'...", target_branch_name);
    let ref_name = format!("refs/heads/{}", target_branch_name);
    repo.set_head(&ref_name)
        .with_context(|| format!("Failed to checkout branch '{}'", target_branch_name))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .with_context(|| format!("Failed to force checkout head for branch '{}'", target_branch_name))?;

    // Now get mutable access to update config AFTER git operations
    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.active_branch = Some(target_branch_name.to_string());
    if !repo_config_mut.tracked_branches.contains(target_branch_name) {
        repo_config_mut.tracked_branches.push(target_branch_name.to_string());
    }

    config::save_config(&config)?;

    println!(
        "{}",
        format!(
            "Switched to branch '{}' for repository '{}'.",
            target_branch_name,
            repo_name.cyan()
        ).green()
    );

    Ok(())
}

async fn sync_repository(
    args: SyncRepoArgs, 
    cli_args: &CliArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>
) -> Result<()> {
    let repo_name = match args.name {
        Some(name) => name,
        None => config.active_repository.clone().ok_or_else(|| {
            anyhow!("No active repository set and no repository specified. Use 'repo use <name>' or 'repo sync <name>'.")
        })?,
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow!("Repository '{}' not found in config.", repo_name))?;

    // Clone repo_config for immutable use within this scope, but pass mutable config down
    let repo_config = config.repositories[repo_config_index].clone();
    let active_branch_name = repo_config.active_branch.clone().ok_or_else(|| {
        anyhow!(
            "No active branch set for repository '{}'. Use 'repo use-branch <branch_name>' first.",
            repo_name
        )
    })?;
    let last_synced_commit_oid_str = repo_config.last_synced_commits.get(&active_branch_name);

    println!(
        "Syncing repository '{}' at {}...",
        repo_name.cyan(),
        repo_config.local_path.display()
    );

    let repo = Repository::open(&repo_config.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config.local_path.display()))?;

    println!(
        "Fetching updates for branch '{}' from remote '{}'...",
        active_branch_name.cyan(), repo_config.remote_name.as_deref().unwrap_or("origin").cyan()
    );

    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin"); // Use configured remote or default

    // Setup fetch options with credential handling
    let cloned_config = config.clone(); 
    let mut fetch_opts = create_fetch_options(&cloned_config, &repo_config.url)?;
    
    // Construct refspec for the active branch
    let refspec = format!("refs/heads/{}:refs/remotes/{}/{}", active_branch_name, remote_name, active_branch_name);
    let mut remote = repo.find_remote(remote_name)
        .with_context(|| format!("Failed to find remote '{}' in repository", remote_name))?;
    remote.fetch(&[refspec], Some(&mut fetch_opts), None)
        .with_context(|| format!("Failed to fetch updates for branch '{}' from remote '{}'", active_branch_name, remote_name))?;
    println!("Fetch completed.");

    // Get remote commit OID for the active branch
    let remote_branch_ref_name = format!("refs/remotes/{}/{}", remote_name, active_branch_name);
    let remote_commit_ref = repo.find_reference(&remote_branch_ref_name)
        .with_context(|| format!("Remote branch reference '{}' not found after fetch. Has the branch been pushed?", remote_branch_ref_name))?;
    let remote_commit = remote_commit_ref.peel_to_commit()?;
    let remote_commit_oid = remote_commit.id();
    let remote_commit_oid_str = remote_commit_oid.to_string();

    let collection_name = get_collection_name(&repo_name);

    // Determine sync type: Initial, Update, or Already Synced
    match last_synced_commit_oid_str {
        None => {
            // --- Initial Sync ---
            println!("Performing initial sync and index for branch '{}'...", active_branch_name.cyan());
            
            // Fast-forward local branch to remote commit
            merge_local_branch(&repo, &active_branch_name, &remote_commit)?;

            // Index all files in the current tree
            println!("Indexing all relevant files in the repository...");
            let tree = remote_commit.tree()?;
            let mut all_files = Vec::new();
            collect_files_from_tree(&repo, &tree, &mut all_files, &PathBuf::new())?;
            
            println!("Found {} total files in the branch tree.", all_files.len());
            let relevant_files: Vec<PathBuf> = all_files.into_iter()
                .filter(|path| {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    is_supported_extension(ext)
                })
                .collect();

            if relevant_files.is_empty() {
                println!("{}", "No relevant files found to index in this branch.".yellow());
            } else {
                println!("Found {} relevant files to index.", relevant_files.len());
                 index_files(
                     client.as_ref(),
                     cli_args,
                     &config, 
                     &repo_config.local_path,
                     &relevant_files,
                     &collection_name,
                     &active_branch_name,
                     &remote_commit_oid_str,
                 ).await.context("Failed to index files during initial sync")?;
            }

            // Pass mutable config to update function
            update_sync_status_and_languages(
                config, // Pass mutable reference
                repo_config_index,
                &active_branch_name,
                &remote_commit_oid_str,
                client.as_ref(),
                &collection_name
            ).await?;
            println!(
                "{}",
                format!(
                    "Repository '{}', branch '{}' initial sync complete to commit {}.",
                    repo_name.cyan(),
                    active_branch_name.cyan(),
                    &remote_commit_oid_str[..8].cyan()
                ).green()
            );
        }
        Some(last_sync_str) => {
            // --- Subsequent Sync ---
            if last_sync_str == &remote_commit_oid_str {
                 println!(
                     "{}",
                     format!("Branch '{}' is already up-to-date (commit {}).", active_branch_name.cyan(), &last_sync_str[..8].cyan()).green()
                 );
                // No indexing needed, just print query suggestion below
            } else {
                 println!(
                    "Branch '{}' has updates ({} -> {}). Analyzing changes...",
                    active_branch_name.cyan(),
                    &last_sync_str[..8].cyan(),
                    &remote_commit_oid_str[..8].cyan()
                );

                let last_synced_oid = git2::Oid::from_str(last_sync_str)?;
                let last_synced_commit = repo.find_commit(last_synced_oid)?;
                let last_synced_tree = last_synced_commit.tree()?;
                let current_tree = remote_commit.tree()?;

                // --- Diff and Process Changes ---
                let mut diff_opts = DiffOptions::new();
                diff_opts.include_untracked(false);
                diff_opts.ignore_submodules(true);
                // diff_opts.pathspec(&repo_config.local_path); // Unnecessary?

                let mut diff = repo.diff_tree_to_tree(Some(&last_synced_tree), Some(&current_tree), Some(&mut diff_opts))?;

                let mut find_opts = DiffFindOptions::new();
                find_opts.renames(true);
                find_opts.copies(true);
                diff.find_similar(Some(&mut find_opts))?;

                let mut added_or_modified_files = Vec::new();
                let mut deleted_files = Vec::new();
                let mut renamed_files_count = 0; // Just count for info

                let pb_diff = ProgressBar::new(diff.deltas().len() as u64);
                pb_diff.set_style(ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
                    .progress_chars("#>- "));
                pb_diff.set_message("Analyzing deltas");

                for delta in diff.deltas() {
                     match delta.status() {
                        Delta::Added | Delta::Modified | Delta::Copied => {
                            if let Some(new_file) = delta.new_file().path() {
                                let extension = new_file.extension().and_then(|e| e.to_str()).unwrap_or("");
                                if is_supported_extension(extension) {
                                    added_or_modified_files.push(new_file.to_path_buf());
                                }
                            }
                        }
                        Delta::Deleted => {
                            if let Some(old_file) = delta.old_file().path() {
                                let extension = old_file.extension().and_then(|e| e.to_str()).unwrap_or("");
                                if is_supported_extension(extension) {
                                    deleted_files.push(old_file.to_path_buf());
                                }
                            }
                        }
                        Delta::Renamed => {
                             if let (Some(old_file), Some(new_file)) = (delta.old_file().path(), delta.new_file().path()) {
                                let old_ext = old_file.extension().and_then(|e| e.to_str()).unwrap_or("");
                                let new_ext = new_file.extension().and_then(|e| e.to_str()).unwrap_or("");
                                if is_supported_extension(old_ext) || is_supported_extension(new_ext) {
                                    renamed_files_count += 1;
                                     if is_supported_extension(old_ext) { deleted_files.push(old_file.to_path_buf()); }
                                     if is_supported_extension(new_ext) { added_or_modified_files.push(new_file.to_path_buf()); }
                                }
                            }
                        }
                        _ => {} 
                    }
                    pb_diff.inc(1);
                }
                 pb_diff.finish_with_message("Delta analysis complete");

                 println!(
                    "Detected changes (relevant files only): {} added/modified, {} deleted, {} renamed.",
                    added_or_modified_files.len().to_string().green(),
                    deleted_files.len().to_string().red(),
                    renamed_files_count.to_string().yellow()
                );
                
                // Perform Deletions
                if !deleted_files.is_empty() {
                    println!(
                        "Deleting {} removed files from collection '{}'...",
                        deleted_files.len(), collection_name.cyan()
                    );
                    delete_points_for_files(
                        client.as_ref(), 
                        &collection_name, 
                        &active_branch_name, 
                        &deleted_files
                    ).await.context("Failed to delete points for removed files")?;
                    println!("Deletion complete.");
                }

                // Merge remote changes into local branch
                merge_local_branch(&repo, &active_branch_name, &remote_commit)?;

                // Index Added/Modified Files
                 if !added_or_modified_files.is_empty() {
                    println!(
                        "Attempting to index {} added/modified files for branch '{}'...",
                        added_or_modified_files.len(), active_branch_name.cyan()
                    );
                    index_files(
                        client.as_ref(),
                        cli_args,
                        &config, // Pass borrow of original config
                        &repo_config.local_path,
                        &added_or_modified_files,
                        &collection_name,
                        &active_branch_name,
                        &remote_commit_oid_str,
                    ).await.context("Failed to index added/modified files")?;
                    println!("Indexing complete.");
                }

                // Pass mutable config to update function
                update_sync_status_and_languages(
                    config, // Pass mutable reference
                    repo_config_index,
                    &active_branch_name,
                    &remote_commit_oid_str,
                    client.as_ref(),
                    &collection_name
                ).await?;
                println!(
                    "{}",
                    format!(
                        "Repository '{}', branch '{}' synced successfully to commit {}.",
                        repo_name.cyan(),
                        active_branch_name.cyan(),
                        &remote_commit_oid_str[..8].cyan()
                    ).green()
                );
            }
        }
    }

    // --- Final Query Suggestion ---
    println!(
        "\nRepository '{}' is synced and ready. Try:\n  {}",
        repo_name.cyan(),
        format!("vectordb-cli query -r {} \"your query\"", repo_name).white().bold()
    );

    Ok(())
}

// Helper function to update local branch (fast-forward or error)
fn merge_local_branch<'repo>(
    repo: &'repo Repository,
    branch_name: &str,
    target_commit: &git2::Commit<'repo>,
) -> Result<()> {
    println!("Updating local branch '{}'...", branch_name.cyan());
    let local_ref_name = format!("refs/heads/{}", branch_name);
    
    // Get the OID of the commit the local ref *currently* points to
    let local_ref_oid_opt = repo.find_reference(&local_ref_name).ok().and_then(|r| r.target());

    // Check if the local ref already points to the target commit
    if local_ref_oid_opt == Some(target_commit.id()) {
        println!("Local branch already points to target commit {}. No merge needed.", &target_commit.id().to_string()[..8]);
        // Ensure HEAD is pointing to the branch and checkout
        repo.set_head(&local_ref_name)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        return Ok(());
    }

    let annotated_commit = repo.find_annotated_commit(target_commit.id())?;
    let merge_analysis = repo.merge_analysis(&[&annotated_commit])?;

    if merge_analysis.0.is_fast_forward() || merge_analysis.0.is_up_to_date() {
        println!("Performing fast-forward merge...");
        let mut local_ref = repo.find_reference(&local_ref_name)?;
        local_ref.set_target(
            annotated_commit.id(),
            &format!("Fast-forward {} to commit {}", local_ref_name, target_commit.id())
        )?;
        repo.set_head(&local_ref_name)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        println!("Fast-forward merge successful.");
        Ok(())
    } else {
        bail!(
             "Merge required, but automatic non-fast-forward merges are not supported yet. 
             Please resolve conflicts manually in the repository for branch '{}' and then run sync again.",
             branch_name
         );
    }
}

// Helper to recursively collect files from a tree
fn collect_files_from_tree(
    repo: &Repository,
    tree: &git2::Tree,
    file_list: &mut Vec<PathBuf>,
    current_path: &PathBuf,
) -> Result<()> {
    for entry in tree.iter() {
        let entry_path = current_path.join(entry.name().unwrap_or_default());
        match entry.kind() {
            Some(git2::ObjectType::Blob) => {
                file_list.push(entry_path);
            }
            Some(git2::ObjectType::Tree) => {
                let subtree = repo.find_tree(entry.id())?;
                collect_files_from_tree(repo, &subtree, file_list, &entry_path)?;
            }
            _ => {} // Ignore commits, tags, etc.
        }
    }
    Ok(())
}


// Helper function to update config with sync status and detected languages
async fn update_sync_status_and_languages(
    config: &mut AppConfig, // Changed to mutable reference
    repo_config_index: usize,
    branch_name: &str,
    commit_oid_str: &str,
    client: &Qdrant, // Borrow client
    collection_name: &str,
) -> Result<()> {
    println!("Updating sync status and checking indexed languages...");
    
    // --- Collect indexed languages/extensions --- 
    let mut current_languages = HashSet::new();
    match client.collection_info(collection_name.to_string()).await {
        Ok(info) => {
             if let Some(payload_schema) = info.result.and_then(|i| Some(i.payload_schema)) {
                 if payload_schema.contains_key(FIELD_LANGUAGE) {
                    log::debug!("Attempting to query distinct languages from collection '{}'", collection_name);
                    // Use scroll with filter for the current branch
                    let scroll_request = qdrant_client::qdrant::ScrollPointsBuilder::new(collection_name.to_string())
                        .limit(10000) // Increase sample size maybe? Or make it optional?
                        .filter(Filter::must([Condition::matches(FIELD_BRANCH, branch_name.to_string())]))
                        .with_payload(qdrant_client::qdrant::with_payload_selector::SelectorOptions::Include( 
                            qdrant_client::qdrant::PayloadIncludeSelector { fields: vec![FIELD_LANGUAGE.to_string()] }
                        ))
                        .build();
                     match client.scroll(scroll_request).await {
                         Ok(mut scroll_response) => {
                             while let Some(result) = scroll_response.result.pop() {
                                 if let Some(lang_value) = result.payload.get(FIELD_LANGUAGE).and_then(|v| v.kind.as_ref()) {
                                     if let qdrant_client::qdrant::value::Kind::StringValue(s) = lang_value {
                                         if !s.is_empty() {
                                            current_languages.insert(s.clone());
                                         }
                                     }
                                 }
                             }
                             if !current_languages.is_empty() {
                                 log::info!("Found indexed languages for branch '{}': {:?}", branch_name, current_languages);
                             } else {
                                 log::debug!("No specific languages found in payload sample for branch '{}'.", branch_name);
                             }
                         }
                         Err(e) => log::warn!("Could not scroll points to determine languages for branch '{}': {}", branch_name, e),
                     }
                 } else {
                    log::debug!("Payload index for '{}' does not exist on collection '{}'. Cannot determine indexed languages.", FIELD_LANGUAGE, collection_name);
                 }
            } else {
                log::warn!("Could not access payload schema for collection '{}'", collection_name);
            }
        }
        Err(e) => log::warn!("Could not get collection info to determine languages: {}", e),
    }
    let languages_vec: Vec<String> = current_languages.into_iter().collect();

    // --- Update config (directly mutate the passed reference) ---
    let repo_config_mut = config
        .repositories
        .get_mut(repo_config_index)
        .unwrap(); // Should always exist

    repo_config_mut.last_synced_commits.insert(
        branch_name.to_string(), // Use to_string here
        commit_oid_str.to_string() // Use to_string here
    );
    // Update indexed languages (only if non-empty, otherwise keep existing None)
    if !languages_vec.is_empty() {
        repo_config_mut.indexed_languages = Some(languages_vec);
    } else if repo_config_mut.indexed_languages.is_none() {
         // If languages were not found AND there was no previous language data, set to empty vec
         repo_config_mut.indexed_languages = Some(vec![]); // Explicitly empty means we checked
    }
    // If languages_vec is empty but repo_config_mut.indexed_languages was Some(..), we keep the old data

    config::save_config(&config)?; // Save the modified config (pass immutable reference to save_config)
    Ok(())
}


async fn delete_points_for_files(
    client: &Qdrant,
    collection_name: &str,
    branch_name: &str,
    relative_paths: &[PathBuf],
) -> Result<()> {
    if relative_paths.is_empty() {
        log::debug!("No relative paths provided for deletion, skipping.");
        return Ok(());
    }
    log::info!("Deleting points for branch '{}' and {} files...", branch_name, relative_paths.len());

    // --- Progress Bar --- 
    let pb_style = ProgressStyle::with_template(
        "{spinner:.red} [{elapsed_precise}] [{bar:40.red/yellow}] Deleting {pos}/{len} files ({per_sec}) {msg}",
    )?
    .progress_chars("-> ");
    let pb = ProgressBar::new(relative_paths.len() as u64);
    pb.set_style(pb_style);
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message("Preparing deletion...");

    // We need to delete points matching ANY of these file paths for the given branch.
    // Qdrant filters might require constructing a should filter if deleting many paths at once is needed.
    // Simpler approach: Delete per file path.
    let mut deletion_errors = Vec::new();
    for path in relative_paths {
        pb.set_message(format!("Deleting {}...", path.display()));
        log::debug!("Preparing to delete points for file: {}", path.display());
        let filter = Filter::must([
            Condition::matches(FIELD_BRANCH, branch_name.to_string()),
            Condition::matches(FIELD_FILE_PATH, path.to_string_lossy().to_string()),
        ]);
        match client.delete_points(DeletePointsBuilder::new(collection_name).points(filter)).await {
            Ok(_) => {
                log::debug!("Successfully deleted points for file: {}", path.display());
            }
            Err(e) => {
                let err_msg = format!("Failed to delete points for file {}: {}", path.display(), e);
                log::error!("{}", err_msg);
                pb.println(format!("Error: {}", err_msg));
                deletion_errors.push(err_msg);
            }
        }
        pb.inc(1); // Increment progress bar
    }

    pb.finish_with_message("Deletion process complete.");

    if !deletion_errors.is_empty() {
        let error_summary = format!("Encountered {} errors during point deletion:", deletion_errors.len());
        log::error!("{}:\n - {}", error_summary, deletion_errors.join("\n - "));
        // Return an error summarizing the failures
        bail!("{}\n - {}", error_summary, deletion_errors.join("\n - "));
    }

    Ok(())
}

async fn index_files(
    client: &Qdrant,
    cli_args: &CliArgs,
    config: &AppConfig,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    collection_name: &str,
    branch_name: &str,
    commit_hash: &str,
) -> Result<()> {
    if relative_paths.is_empty() {
        log::info!("No files provided to index_files.");
        return Ok(());
    }

    // --- Resolve ONNX paths (priority: args -> env -> config) --- 
    let onnx_model_path_str = cli_args
        .onnx_model_path_arg
        .as_ref()
        .or(config.onnx_model_path.as_ref())
        .ok_or_else(|| anyhow!("ONNX model path not specified via args, env, or config"))?;

    let onnx_tokenizer_dir_str = cli_args
        .onnx_tokenizer_dir_arg
        .as_ref()
        .or(config.onnx_tokenizer_path.as_ref()) 
        .ok_or_else(|| anyhow!("ONNX tokenizer directory not specified via args, env, or config"))?;

    // Convert to PathBuf for display and use
    let onnx_model_path = PathBuf::from(onnx_model_path_str);
    let onnx_tokenizer_dir = PathBuf::from(onnx_tokenizer_dir_str);

    println!(
        "Using ONNX model: {}\nUsing ONNX tokenizer dir: {}",
        onnx_model_path.display(),
        onnx_tokenizer_dir.display()
    );

    // --- Initialize Embedding Handler ---
    println!("Initializing embedding handler...");
    let embedding_handler = match EmbeddingHandler::new(
        embedding::EmbeddingModelType::Onnx,
        Some(onnx_model_path.clone()), // Pass cloned PathBuf
        Some(onnx_tokenizer_dir.clone()) // Pass cloned PathBuf
    ) {
        Ok(handler) => {
            println!("Embedding handler initialized successfully.");
            handler
        }
        Err(e) => {
            log::error!("Failed to initialize Embedding Handler: {}", e);
            bail!("Failed to initialize embedding handler: {}", e);
        }
    };
    // Remove unused variables
    // let batch_size = 32; // TODO: Make configurable?
    // let mut points_to_upsert: Vec<PointStruct> = Vec::new();

    // --- Progress Bar --- 
    let pb_style = ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({per_sec}) {msg}",
    )?
    .progress_chars("#>-;");
    let pb = ProgressBar::new(relative_paths.len() as u64);
    pb.set_style(pb_style);
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message("Processing files...");

    // --- File Processing Loop --- 
    let mut total_points_processed = 0;
    let mut total_files_processed = 0;
    let mut total_files_skipped = 0;
    // Define batching variables here
    let mut batch_num = 1;
    let total_batches_estimate = (relative_paths.len() / BATCH_SIZE).max(1); // Estimate batches
    
    let model = embedding_handler
        .create_embedding_model()
        .context("Failed to create embedding model")?;
    let mut points_batch = Vec::with_capacity(BATCH_SIZE);

    for relative_path in relative_paths {
        let absolute_path = repo_root.join(relative_path);
        if !absolute_path.exists() {
             log::warn!("File {} does not exist (likely deleted between diff and processing). Skipping.", absolute_path.display());
             pb.println(format!("Warning: File {} not found, skipping.", absolute_path.display()));
             total_files_skipped += 1;
             pb.inc(1);
             continue;
        }
        let relative_path_str = relative_path.to_string_lossy().to_string();

        // Get Chunks
        let chunks = match syntax::get_chunks(&absolute_path) {
            Ok(chunks) => chunks,
            Err(e) => {
                log::warn!("Failed to parse file {}: {}. Skipping.", absolute_path.display(), e);
                pb.println(format!("Warning: Failed to parse {}, skipping.", absolute_path.display()));
                total_files_skipped += 1;
                pb.inc(1);
                continue;
            }
        };

        if chunks.is_empty() {
            log::debug!("No text chunks found in file {}. Skipping.", absolute_path.display());
            total_files_skipped += 1;
            pb.inc(1);
            continue;
        }

        // Generate Embeddings
        let chunk_contents: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let embeddings = match model.embed_batch(&chunk_contents) {
            Ok(embeddings) => embeddings,
            Err(e) => {
                log::error!("Failed to generate embeddings for {}: {}. Skipping file.", absolute_path.display(), e);
                pb.println(format!("Error embedding {}, skipping.", absolute_path.display()));
                total_files_skipped += 1;
                pb.inc(1);
                continue;
            }
        };

        // Create PointStructs
        let file_extension = absolute_path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_string();
        for (i, chunk) in chunks.iter().enumerate() {
            let mut payload = Payload::new();
            payload.insert(FIELD_FILE_PATH, relative_path_str.clone()); 
            payload.insert(FIELD_START_LINE, chunk.start_line as i64);
            payload.insert(FIELD_END_LINE, chunk.end_line as i64);
            payload.insert(FIELD_LANGUAGE, chunk.language.to_string());
            payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
            payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.clone());
            payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());
            payload.insert(FIELD_BRANCH, branch_name.to_string()); // Add branch
            payload.insert(FIELD_COMMIT_HASH, commit_hash.to_string()); // Add commit hash

            let point = PointStruct::new(
                Uuid::new_v4().to_string(),
                embeddings[i].clone(),
                payload,
            );
            points_batch.push(point);

            // Upsert batch if full
            if points_batch.len() >= BATCH_SIZE {
                let batch_to_upsert = std::mem::take(&mut points_batch);
                let current_batch_size = batch_to_upsert.len();
                // Pass batch_num and total_batches_estimate, and borrow pb
                upsert_batch(client, collection_name, batch_to_upsert, batch_num, total_batches_estimate, &pb).await?;
                total_points_processed += current_batch_size;
                batch_num += 1; 
            }
        }
        total_files_processed += 1;
        pb.inc(1);
    }

    // Upsert any remaining points
    if !points_batch.is_empty() {
        let final_batch_size = points_batch.len();
        // Pass the final batch_num and total_batches_estimate, and borrow pb
        upsert_batch(client, collection_name, points_batch, batch_num, total_batches_estimate, &pb).await?;
        total_points_processed += final_batch_size;
    }

    pb.finish_with_message("File processing complete");

    log::info!("Indexing for sync finished. Processed: {}, Skipped: {}, Points: {}", total_files_processed, total_files_skipped, total_points_processed);
    println!("  (Processed {} files, skipped {}, upserted {} points)", total_files_processed, total_files_skipped, total_points_processed);

    Ok(())
}

async fn ensure_repository_collection_exists(
    client: &Qdrant,
    collection_name: &str,
) -> Result<()> {
    let exists = client.collection_exists(collection_name).await?;
    if exists {
        log::debug!("Collection '{}' already exists.", collection_name);
    } else {
        log::info!("Collection '{}' does not exist. Creating...", collection_name);
        let vector_params = VectorParamsBuilder::new(DEFAULT_VECTOR_DIMENSION, Distance::Cosine).build();
        let create_request = CreateCollectionBuilder::new(collection_name)
             .vectors_config(vector_params)
             .build();

        client
            .create_collection(create_request)
            .await?;
        log::info!("Collection '{}' created successfully.", collection_name);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut attempts = 0;
        loop {
            let info = client.collection_info(collection_name).await?;
            if info.result.map_or(false, |i| i.status == CollectionStatus::Green as i32) {
                break;
            }
            attempts += 1;
            if attempts > 50 {
                bail!("Collection '{}' did not become ready in time.", collection_name);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        log::info!("Collection '{}' is ready.", collection_name);

    }

    ensure_payload_index(client, collection_name, FIELD_FILE_PATH, FieldType::Keyword, true, None).await?;
    ensure_payload_index(client, collection_name, FIELD_START_LINE, FieldType::Integer, false, None).await?;
    ensure_payload_index(client, collection_name, FIELD_END_LINE, FieldType::Integer, false, None).await?;
    ensure_payload_index(client, collection_name, FIELD_LANGUAGE, FieldType::Keyword, true, None).await?;
    ensure_payload_index(client, collection_name, FIELD_BRANCH, FieldType::Keyword, true, None).await?;
    ensure_payload_index(client, collection_name, FIELD_COMMIT_HASH, FieldType::Keyword, true, None).await?;
    ensure_payload_index(client, collection_name, FIELD_ELEMENT_TYPE, FieldType::Keyword, true, None).await?;

    Ok(())
}

async fn handle_repo_clear(
    args: ClearRepoArgs,
    config: &AppConfig, // Borrow config to find active/specified repo
    client: Arc<Qdrant>,
) -> Result<()> {
    let repo_name = match args.name {
        Some(name) => name,
        None => config.active_repository.clone().ok_or_else(|| {
            anyhow!("No active repository set and no repository specified. Use 'repo use <name>' or 'repo clear <name>'.")
        })?,
    };

    // Verify the repository exists in the config
    if !config.repositories.iter().any(|r| r.name == repo_name) {
        bail!("Repository '{}' not found in configuration.", repo_name);
    }

    let collection_name = get_collection_name(&repo_name);
    log::info!("Preparing to clear index for repository: '{}', collection: '{}'", repo_name, collection_name);

    // Confirmation
    if !args.yes {
        print!(
            "{}",
            format!(
                "Are you sure you want to delete the index for repository '{}' (collection '{}')? [y/N]: ",
                repo_name.yellow().bold(),
                collection_name.yellow().bold()
            )
            .red()
        );
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut confirmation = String::new();
        io::stdin()
            .read_line(&mut confirmation)
            .context("Failed to read confirmation line")?;

        if confirmation.trim().to_lowercase() != "y" {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    // Delete Collection
    log::info!("Attempting to delete collection '{}'...", collection_name);
    println!("Deleting collection '{}'...", collection_name);

    match client.delete_collection(collection_name.clone()).await {
        Ok(op_result) => {
            if op_result.result {
                println!(
                    "{}",
                    format!("Successfully deleted collection '{}'.", collection_name).green()
                );
                 log::info!("Collection '{}' deleted successfully.", collection_name);
            } else {
                 println!(
                     "{}",
                     format!("Collection '{}' might not have existed or deletion failed server-side.", collection_name).yellow()
                 );
                 log::warn!("Delete operation for collection '{}' returned false.", collection_name);
            }
        }
        Err(e) => {
             // Check if it's a "not found" type error - treat as success in clearing
             if e.to_string().contains("Not found") || e.to_string().contains("doesn't exist") {
                 println!(
                     "{}",
                     format!("Collection '{}' did not exist.", collection_name).yellow()
                 );
                 log::warn!("Collection '{}' not found during delete attempt.", collection_name);
             } else {
                 // For other errors, report them
                 eprintln!(
                     "{}",
                     format!("Failed to delete collection '{}': {}", collection_name, e).red()
                 );
                 return Err(e).context(format!("Failed to delete collection '{}'", collection_name));
             }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RepositoryConfig; // Import RepositoryConfig
    use qdrant_client::Qdrant;
    use std::sync::Arc;
    use tokio::runtime::Runtime;
    use std::collections::HashMap; // Import HashMap
    use std::path::PathBuf; // Import PathBuf
    // Mock Qdrant client and AppConfig setup needed here

    #[test]
    #[ignore] // Ignored because it requires a running Qdrant instance and config setup
    fn test_handle_repo_clear_specific_repo() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // --- Setup Mock Client & Config ---
            // let mock_client = Qdrant::from_url("http://localhost:6334").build().unwrap(); // Replace with mock
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); // Placeholder
            let test_repo_name = "my-test-repo";
            let config = AppConfig {
                repositories: vec![RepositoryConfig {
                    name: test_repo_name.to_string(),
                    url: "dummy_url".to_string(),
                    local_path: PathBuf::from("/tmp/dummy"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    active_branch: Some("main".to_string()),
                    remote_name: Some("origin".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: HashMap::new(),
                    indexed_languages: None,
                }],
                active_repository: Some(test_repo_name.to_string()),
                qdrant_url: "http://localhost:6334".to_string(), // Must be String, not Option<String>
                onnx_model_path: None,
                onnx_tokenizer_path: None,
            };

            // --- Prepare Args ---
            let args = ClearRepoArgs { name: Some(test_repo_name.to_string()), yes: true };
            let expected_collection_name = get_collection_name(test_repo_name);

            // --- Expected Call ---
            // Mock expectation: client.delete_collection(expected_collection_name) called once
            // For simplicity, run and check Ok result

            // --- Execute ---
            let result = handle_repo_clear(args, &config, client).await;

            // --- Assert ---
            assert!(result.is_ok());
            // In a real test, verify delete_collection was called with expected_collection_name
        });
    }

    #[test]
    #[ignore] // Ignored because it requires a running Qdrant instance and config setup
    fn test_handle_repo_clear_active_repo() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
             // --- Setup Mock Client & Config ---
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); // Placeholder
            let active_repo_name = "active-repo";
            let config = AppConfig {
                repositories: vec![RepositoryConfig {
                    name: active_repo_name.to_string(),
                    url: "dummy_url".to_string(),
                    local_path: PathBuf::from("/tmp/dummy"),
                    default_branch: "main".to_string(),
                    tracked_branches: vec!["main".to_string()],
                    active_branch: Some("main".to_string()),
                    remote_name: Some("origin".to_string()),
                    ssh_key_path: None,
                    ssh_key_passphrase: None,
                    last_synced_commits: HashMap::new(),
                    indexed_languages: None,
                }],
                active_repository: Some(active_repo_name.to_string()),
                qdrant_url: "http://localhost:6334".to_string(), // Must be String, not Option<String>
                onnx_model_path: None,
                onnx_tokenizer_path: None,
            };

            // --- Prepare Args ---
            let args = ClearRepoArgs { name: None, yes: true }; // No name specified, should use active
            let expected_collection_name = get_collection_name(active_repo_name);

            // --- Execute ---
            let result = handle_repo_clear(args, &config, client).await;

            // --- Assert ---
            assert!(result.is_ok());
            // In a real test, verify delete_collection was called with expected_collection_name
        });
    }

    #[test]
    #[ignore] // Ignored because it requires config setup
    fn test_handle_repo_clear_no_active_or_specified_fails() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
             // --- Setup Mock Client & Config ---
            let client = Arc::new(Qdrant::from_url("http://localhost:6334").build().unwrap()); // Placeholder
            let config = AppConfig {
                repositories: vec![], // No repos
                active_repository: None, // No active repo
                qdrant_url: "http://localhost:6334".to_string(), // Must be String, not Option<String>
                onnx_model_path: None,
                onnx_tokenizer_path: None,
            };

            // --- Prepare Args ---
            let args = ClearRepoArgs { name: None, yes: true };

            // --- Execute ---
            let result = handle_repo_clear(args, &config, client).await;

            // --- Assert ---
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("No active repository set"));
        });
    }
} 
