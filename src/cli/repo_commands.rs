// src/cli/repo_commands.rs
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use git2::{Repository, DiffOptions, DiffFindOptions, Delta, Cred, RemoteCallbacks, FetchOptions, CredentialType};
use qdrant_client::{
    qdrant::{ CollectionStatus, CreateCollectionBuilder, Distance, FieldType, VectorParamsBuilder, Filter, Condition, DeletePointsBuilder, PointStruct, CollectionInfo },
    Payload,
    Qdrant,
};
use std::{fs, path::PathBuf, sync::Arc, time::Duration, collections::HashSet};
use uuid::Uuid;
use indicatif::{ProgressBar, ProgressStyle};

use crate::config::{self, AppConfig, RepositoryConfig};
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
    /// Remove a managed repository.
    Remove(RemoveRepoArgs),
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

pub(crate) fn get_collection_name(repo_name: &str) -> String {
    format!("{}{}", COLLECTION_NAME_PREFIX, repo_name)
}

pub async fn handle_repo_command(
    args: RepoArgs, 
    cli_args: &CliArgs,
    config: AppConfig, 
    client: Arc<Qdrant>
) -> Result<()> {
    match args.command {
        RepoCommand::Add(add_args) => add_repository(add_args, config, client).await,
        RepoCommand::List => list_repositories(config),
        RepoCommand::Use(use_args) => use_repository(use_args, config),
        RepoCommand::Remove(remove_args) => remove_repository(remove_args, config, client).await,
        RepoCommand::UseBranch(branch_args) => use_branch(branch_args, config).await,
        RepoCommand::Sync(sync_args) => sync_repository(sync_args, cli_args, config, client).await,
    }
}

async fn add_repository(
    args: AddRepoArgs, 
    mut config: AppConfig,
    client: Arc<Qdrant>
) -> Result<()> {
    let repo_name = match args.name {
        Some(name) => name,
        None => PathBuf::from(&args.url)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".git").to_string())
            .ok_or_else(|| anyhow::anyhow!("Could not derive repository name from URL"))?,
    };

    if config.repositories.iter().any(|r| r.name == repo_name) {
        bail!("Repository '{}' already exists.", repo_name);
    }

    let repo_base_path = config::get_repo_base_path()?;
    fs::create_dir_all(&repo_base_path)
        .with_context(|| format!("Failed to create repository base directory at {}", repo_base_path.display()))?;
    let local_path = args.local_path.unwrap_or(repo_base_path.join(&repo_name));

    if local_path.exists() {
         println!(
            "{}",
            format!(
                "Local directory '{}' already exists. Assuming it's the intended clone. Skipping clone.",
                local_path.display()
            ).yellow()
        );
        // We could potentially validate if it's a git repo and matches the URL, but let's keep it simple for now.
    } else {
        println!("Cloning repository '{}' from {}...", repo_name.cyan(), args.url.cyan());
        let _repo = Repository::clone(&args.url, &local_path)
            .with_context(|| format!("Failed to clone repository from {}", args.url))?;
        println!("Repository cloned successfully to {}", local_path.display());
    }

    let default_branch_name = match args.branch {
         Some(b) => b,
         None => {
             let cloned_repo = Repository::open(&local_path)?;
             let head = cloned_repo.head()?;
             head.shorthand()
                 .ok_or_else(|| anyhow::anyhow!("Could not determine default branch name"))?
                 .to_string()
         }
     };

    let collection_name = get_collection_name(&repo_name);
    println!("Ensuring Qdrant collection '{}' exists...", collection_name.cyan());
    ensure_repository_collection_exists(&client, &collection_name).await?;
    println!("Qdrant collection ensured.");

    let repo_config = RepositoryConfig {
        name: repo_name.clone(),
        url: args.url.clone(),
        local_path,
        default_branch: default_branch_name.clone(),
        tracked_branches: vec![default_branch_name],
        remote_name: args.remote,
        last_synced_commits: Default::default(),
        active_branch: None,
        ssh_key_path: args.ssh_key,
        ssh_key_passphrase: args.ssh_passphrase,
        indexed_languages: None,
    };

    config.repositories.push(repo_config);
    if config.active_repository.is_none() {
        config.active_repository = Some(repo_name.clone());
         println!("Set '{}' as the active repository.", repo_name.cyan());
    }
    config::save_config(&config)?;

    println!(
        "{}",
        format!("Successfully added repository '{}'.", repo_name).green()
    );
    println!("{}", "Run 'index' command to populate the repository.".yellow());

    Ok(())
}

fn list_repositories(config: AppConfig) -> Result<()> {
    if config.repositories.is_empty() {
        println!("No repositories configured yet. Use 'repo add <url>' to add one.");
        return Ok(());
    }

    println!("{}", "Managed Repositories:".bold());
    for repo in config.repositories {
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

fn use_repository(args: UseRepoArgs, mut config: AppConfig) -> Result<()> {
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
    mut config: AppConfig,
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

async fn use_branch(args: UseBranchArgs, mut config: AppConfig) -> Result<()> {
    let repo_name = config.active_repository.clone().ok_or_else(|| {
        anyhow::anyhow!("No active repository set. Use 'repo use <name>' first.")
    })?;

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
    config: AppConfig,
    client: Arc<Qdrant>
) -> Result<()> {
    let repo_name = match args.name {
        Some(name) => name,
        None => config.active_repository.clone().ok_or_else(|| {
            anyhow::anyhow!("No active repository set and no repository specified. Use 'repo use <name>' or 'repo sync <name>'.")
        })?,
    };

    let repo_config_index = config
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found in config.", repo_name))?;

    // Clone config for immutable use in processing, mutable borrow later for updates
    let repo_config = config.repositories[repo_config_index].clone(); 

    println!(
        "Syncing repository '{}' at {}...",
        repo_name.cyan(),
        repo_config.local_path.display()
    );

    let repo = Repository::open(&repo_config.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config.local_path.display()))?;

    let active_branch_name = repo_config.active_branch.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "No active branch set for repository '{}'. Use 'repo use-branch <branch_name>' first.",
            repo_name
        )
    })?;
    
    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin"); // Use configured remote or default

    println!(
        "Fetching updates for branch '{}' from remote '{}'...",
        active_branch_name.cyan(), remote_name.cyan()
    );

    let mut remote = repo.find_remote(remote_name)
        .with_context(|| format!("Failed to find remote '{}' in repository", remote_name))?;

    // Setup fetch options with credential handling
    // Clone config here so fetch_opts doesn't borrow the original config needed later
    let cloned_config = config.clone(); // Clone into a longer-lived binding
    let mut fetch_opts = create_fetch_options(&cloned_config, &repo_config.url)?;
    
    // Construct refspec for the active branch
    let refspec = format!("refs/heads/{}:refs/remotes/{}/{}", active_branch_name, remote_name, active_branch_name);
    remote.fetch(&[refspec], Some(&mut fetch_opts), None)
        .with_context(|| format!("Failed to fetch updates for branch '{}' from remote '{}'", active_branch_name, remote_name))?;
    println!("Fetch completed.");

    // Get local and remote commit OIDs for the active branch
    let local_branch_ref_name = format!("refs/heads/{}", active_branch_name);
    let local_commit = repo.find_reference(&local_branch_ref_name)?.peel_to_commit()?;
    let remote_branch_ref_name = format!("refs/remotes/{}/{}", remote_name, active_branch_name);
    let remote_commit = repo.find_reference(&remote_branch_ref_name)?.peel_to_commit()?;

    if local_commit.id() == remote_commit.id() {
        println!(
            "{}",
            format!("Branch '{}' is already up-to-date.", active_branch_name).green()
        );
        return Ok(());
    }

    println!(
        "Branch '{}' has updates ({} -> {}). Analyzing changes...",
        active_branch_name.cyan(),
        &local_commit.id().to_string()[..8].cyan(),
        &remote_commit.id().to_string()[..8].cyan()
    );

    // Get the tree objects for diffing
    let local_tree = local_commit.tree()?;
    let remote_tree = remote_commit.tree()?;

    let mut diff_opts = DiffOptions::new();
    diff_opts.include_untracked(false);
    diff_opts.ignore_submodules(true);
    diff_opts.pathspec(&repo_config.local_path); // Ensure diff is within the repo path? Needs verification.

    let mut diff = repo.diff_tree_to_tree(Some(&local_tree), Some(&remote_tree), Some(&mut diff_opts))?;

    let mut find_opts = DiffFindOptions::new();
    find_opts.renames(true);
    find_opts.copies(true);
    // find_opts.for_untracked(true); // Might be needed if we handle untracked files later

    diff.find_similar(Some(&mut find_opts))?;

    let mut added_or_modified_files = Vec::new();
    let mut deleted_files = Vec::new();
    let mut renamed_files = Vec::new(); // Store as (old_path, new_path)

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
                    
                    // Check if either old or new is supported
                    if is_supported_extension(old_ext) || is_supported_extension(new_ext) {
                        // Treat rename as delete + add for simplicity in indexing
                         if is_supported_extension(old_ext) {
                             deleted_files.push(old_file.to_path_buf());
                         }
                         if is_supported_extension(new_ext) {
                             added_or_modified_files.push(new_file.to_path_buf());
                             renamed_files.push((old_file.to_path_buf(), new_file.to_path_buf())); // Keep track for info
                         }
                    }
                }
            }
            _ => { /* TypeChanged, Unmodified, Ignored, Untracked, Unreadable - skip */ }
        }
        pb_diff.inc(1);
    }
    pb_diff.finish_with_message("Delta analysis complete");

    println!(
        "Detected changes: {} added/modified, {} deleted, {} renamed (relevant files only)",
        added_or_modified_files.len().to_string().green(),
        deleted_files.len().to_string().red(),
        renamed_files.len().to_string().yellow()
    );

    // Update Qdrant Index based on diff
    let collection_name = get_collection_name(&repo_name);

    // --- Perform Deletions ---
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

    // --- Merge remote changes into local branch ---
    println!("Merging remote changes into local branch '{}'...", active_branch_name.cyan());
    let remote_ref = repo.find_reference(&remote_branch_ref_name)?;
    let annotated_commit = repo.find_annotated_commit(remote_ref.target().unwrap())?;
    let (merge_analysis, _) = repo.merge_analysis(&[&annotated_commit])?;

    if merge_analysis.is_fast_forward() {
        println!("Performing fast-forward merge...");
        let mut local_ref = repo.find_reference(&local_branch_ref_name)?;
        local_ref.set_target(
            annotated_commit.id(),
            &format!("Fast-forward {} to remote {}", local_branch_ref_name, remote_branch_ref_name)
        )?;
        repo.set_head(&local_branch_ref_name)?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
        println!("Fast-forward merge successful.");
    } else if merge_analysis.is_up_to_date() {
         println!("Branch already up-to-date (should have been caught earlier, but handling).", );
         // This case should ideally not happen if we checked commit IDs earlier, but good to handle
    } else {
         bail!(
             "Merge required, but automatic non-fast-forward merges are not supported yet. 
             Please resolve conflicts manually in '{}' for branch '{}' and then run sync again.",
             repo_config.local_path.display(), active_branch_name
         );
        // Later, we might implement a merge strategy or provide better instructions
    }

    // --- Index Added/Modified Files ---
    if !added_or_modified_files.is_empty() {
        println!(
            "Indexing {} added/modified files for branch '{}'...",
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
            &remote_commit.id().to_string(),
        ).await.context("Failed to index added/modified files")?;
        println!("Indexing complete.");
    }

    // --- Collect indexed languages/extensions --- 
    let mut current_languages = HashSet::new();
    match client.collection_info(collection_name.to_string()).await {
        Ok(info) => {
             // This might be inefficient if there are many points.
             // A better approach might involve storing indexed languages directly in config during index/sync.
             // For now, we query the collection's payload index info if available or scan points.
            if let Some(payload_schema) = info.result.and_then(|i| Some(i.payload_schema)) {
                if payload_schema.contains_key(FIELD_LANGUAGE) {
                    // If FIELD_LANGUAGE index exists, we *assume* languages were indexed.
                    // This is an approximation. We don't know *which* languages without querying points.
                    // We'll refine this if needed. For now, let's try querying a small sample.
                    log::debug!("Attempting to query distinct languages from collection '{}'", collection_name);
                    let scroll_request = qdrant_client::qdrant::ScrollPointsBuilder::new(collection_name.to_string())
                        .limit(1000) // Limit sample size
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
                             log::info!("Found indexed languages from sample: {:?}", current_languages);
                         }
                         Err(e) => log::warn!("Could not scroll points to determine languages: {}", e),
                     }
                }
            }
        }
        Err(e) => log::warn!("Could not get collection info to determine languages: {}", e),
    }
    let languages_vec: Vec<String> = current_languages.into_iter().collect();

    // --- Update config with the new sync status AND languages ---
    println!("Updating sync status...");
    let mut mutable_config = config; // Move the original config now, it's no longer borrowed by fetch_opts
    let repo_config_mut = mutable_config
        .repositories
        .get_mut(repo_config_index)
        .unwrap(); // Should always exist

    repo_config_mut.last_synced_commits.insert(
        active_branch_name.clone(),
        remote_commit.id().to_string()
    );
    // Update indexed languages
    repo_config_mut.indexed_languages = if languages_vec.is_empty() { None } else { Some(languages_vec) };

    config::save_config(&mutable_config)?;
    println!(
        "{}",
        format!(
            "Repository '{}', branch '{}' synced successfully to commit {}.",
            repo_name.cyan(),
            active_branch_name.cyan(),
            &remote_commit.id().to_string()[..8].cyan()
        ).green()
    );

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
        return Ok(());
    }
    log::info!("Indexing {} files for branch '{}' commit '{}'...", relative_paths.len(), branch_name, commit_hash);
    println!("Indexing {} files for branch '{}' (commit {})...", relative_paths.len(), branch_name.cyan(), &commit_hash[..7].cyan());

    // --- Initialize Embedding Handler --- 
    let model_env_var = std::env::var("VECTORDB_ONNX_MODEL").ok();
    let tokenizer_env_var = std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").ok();

    let onnx_model_path_str = cli_args.onnx_model_path_arg.as_ref()
        .or(model_env_var.as_ref())
        .or(config.onnx_model_path.as_ref())
        .ok_or_else(|| anyhow::anyhow!("ONNX model path must be provided via --onnx-model, VECTORDB_ONNX_MODEL, or config"))?;
    let onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_ref()
        .or(tokenizer_env_var.as_ref())
        .or(config.onnx_tokenizer_path.as_ref())
        .ok_or_else(|| anyhow::anyhow!("ONNX tokenizer path must be provided via --onnx-tokenizer-dir, VECTORDB_ONNX_TOKENIZER_DIR, or config"))?;

    let onnx_model_path = PathBuf::from(onnx_model_path_str);
    let onnx_tokenizer_path = PathBuf::from(onnx_tokenizer_dir_str);

    if !onnx_model_path.exists() { return Err(anyhow::anyhow!("Resolved ONNX model path does not exist: {}", onnx_model_path.display())); }
    if !onnx_tokenizer_path.is_dir() { return Err(anyhow::anyhow!("Resolved ONNX tokenizer path is not a directory: {}", onnx_tokenizer_path.display())); }
    let tokenizer_file = onnx_tokenizer_path.join("tokenizer.json");
    if !tokenizer_file.exists() { return Err(anyhow::anyhow!("tokenizer.json not found in the ONNX tokenizer directory: {}", onnx_tokenizer_path.display())); }

    log::info!("Initializing embedding handler for indexing...");
    let embedding_handler = Arc::new(
        EmbeddingHandler::new(
            embedding::EmbeddingModelType::Onnx,
            Some(onnx_model_path),
            Some(onnx_tokenizer_path),
        )
        .context("Failed to initialize embedding handler")?,
    );

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

            if points_batch.len() >= BATCH_SIZE {
                let batch_to_upsert = std::mem::take(&mut points_batch);
                upsert_batch(client, collection_name, batch_to_upsert, &pb).await?;
                total_points_processed += BATCH_SIZE;
            }
        }
        total_files_processed += 1;
        pb.inc(1);
    }

    // Upsert remaining points
    if !points_batch.is_empty() {
        let final_batch_size = points_batch.len();
        upsert_batch(client, collection_name, points_batch, &pb).await?;
        total_points_processed += final_batch_size;
    }

    pb.finish_with_message("File processing complete!"); // Different message for sync

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

    ensure_payload_index(client, collection_name, FIELD_FILE_PATH, FieldType::Keyword).await?;
    ensure_payload_index(client, collection_name, FIELD_START_LINE, FieldType::Integer).await?;
    ensure_payload_index(client, collection_name, FIELD_END_LINE, FieldType::Integer).await?;
    ensure_payload_index(client, collection_name, FIELD_LANGUAGE, FieldType::Keyword).await?;
    ensure_payload_index(client, collection_name, FIELD_BRANCH, FieldType::Keyword).await?;
    ensure_payload_index(client, collection_name, FIELD_COMMIT_HASH, FieldType::Keyword).await?;

    Ok(())
} 