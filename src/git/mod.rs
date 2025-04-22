use anyhow::{Result, Context};
use qdrant_client::qdrant::{
        ScrollPointsBuilder,
        PayloadIncludeSelector,
    };
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
};
use tokio::task;

use vectordb_core::{
    config::{AppConfig, RepositoryConfig},
    constants::FIELD_LANGUAGE,
    qdrant_client_trait::QdrantClientTrait,
    repo_helpers,
};

use crate::cli::CliArgs;

use git2::{Repository, FetchOptions, RemoteCallbacks, AutotagOption};

pub struct SyncResult {
    pub success: bool,
    pub message: String,
    pub indexed_languages: Vec<String>,
}

/// Shared implementation of repository sync that can be used by both CLI and server
pub async fn sync_repository<C>(
    client: Arc<C>,
    repo_config: RepositoryConfig,
    options: SyncOptions,
    cli_args: &CliArgs,
    app_config: &AppConfig,
) -> Result<SyncResult>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    log::info!("Synchronizing repository: {}", repo_config.name);
    
    // Clone all values we'll need before moving into spawn_blocking
    let repo_name = repo_config.name.clone();
    let repo_path = repo_config.local_path.clone();
    // Determine if the repository should be treated as primarily local
    // Check if the stored URL points to an existing local directory
    let path_from_url = PathBuf::from(&repo_config.url);
    let is_local_path_repo = repo_config.added_as_local_path;

    let active_branch = match &repo_config.active_branch {
        Some(branch) => branch.clone(),
        None => "main".to_string(), // Default branch if not set, consider erroring?
    };
    let remote_name = repo_config.remote_name.clone().unwrap_or_else(|| "origin".to_string());
    let last_synced_commits = repo_config.last_synced_commits.clone();
    let force_sync = options.force;
    let extensions_filter = options.extensions.clone();
    
    log::info!("Starting sync for repository '{}' on branch '{}'", repo_name, active_branch);
    
    // Make copies of values that will be needed after the closure
    let repo_name_outside = repo_name.clone();
    let repo_path_outside = repo_path.clone();
    let active_branch_outside = active_branch.clone();
    
    // Step 1: Perform ALL git operations in a blocking thread since git2 isn't Send
    let git_result = task::spawn_blocking(move || -> Result<(String, Vec<PathBuf>, bool)> {
        log::info!("Using branch '{}' for repository '{}'", active_branch, repo_name);
        
        // Open the repository
        let repo = Repository::open(&repo_path)?;
        
        let commit_oid_str: String;
        let branch_commit: git2::Commit;

        if !repo_config.added_as_local_path {
            log::debug!("Repository '{}' is treated as remote (added_as_local_path=false).", repo_name);
            // --- Remote Repo Logic (Fetch and check remote ref) ---
            
            // Set up the fetch options
            let mut fetch_opts = FetchOptions::new();
            let mut callbacks = RemoteCallbacks::new();
            callbacks.update_tips(|name, old, new| {
                log::info!("[{}] {} -> {}", name, old, new);
                true
            });
            fetch_opts.remote_callbacks(callbacks);
            fetch_opts.download_tags(AutotagOption::All);
            
            // Fetch updates from the remote
            let mut remote = repo.find_remote(&remote_name)?;
            // Fetch only the active branch if possible, fall back to fetching all if specific fails?
            // Using `fetch(&[&active_branch], ...)` might be restrictive if branch name format differs (e.g. no refs/heads prefix)
            // Consider fetching all refs: remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)?;
             match remote.fetch(&[&active_branch], Some(&mut fetch_opts), None) {
                Ok(_) => log::info!("Fetched remote '{}' for branch '{}'", remote_name, active_branch),
                Err(e) => log::warn!("Failed to fetch specific branch '{}' from remote '{}': {}. Consider checking remote config or branch name.", active_branch, remote_name, e),
            };

            // Find the remote branch reference
            let branch_ref_name = format!("refs/remotes/{}/{}", remote_name, active_branch);
            let branch_ref = repo.find_reference(&branch_ref_name)
                .with_context(|| format!("Could not find remote-tracking reference '{}'. Was the fetch successful?", branch_ref_name))?;
            branch_commit = branch_ref.peel_to_commit()
                .with_context(|| format!("Could not peel reference '{}' to a commit.", branch_ref_name))?;
            commit_oid_str = branch_commit.id().to_string();

        } else {
             log::debug!("Repository '{}' is treated as local-only (added_as_local_path=true).", repo_name);
            // --- Local Repo Logic (Check local head) ---
            // Find the local branch reference
            let branch_ref_name = format!("refs/heads/{}", active_branch);
            let branch_ref = repo.find_reference(&branch_ref_name)
                 .with_context(|| format!("Could not find local branch reference '{}'. Does the branch exist locally?", branch_ref_name))?;
            branch_commit = branch_ref.peel_to_commit()
                 .with_context(|| format!("Could not peel reference '{}' to a commit.", branch_ref_name))?;
            commit_oid_str = branch_commit.id().to_string();
        }
        
        // Check if we need a full sync (Compare target commit with last synced)
        let mut full_sync_needed = force_sync;
        if !force_sync {
            // Direct HashMap access
            if let Some(last_commit) = last_synced_commits.get(&active_branch) {
                if last_commit == &commit_oid_str {
                    log::info!("Repository already synced to commit: {}", commit_oid_str);
                    // Return early with empty file list if no sync needed
                    return Ok((commit_oid_str, Vec::new(), false));
                }
                log::info!("Repository needs syncing from {} to {}", last_commit, commit_oid_str);
            }
            full_sync_needed = true;
        }
        
        if full_sync_needed {
            log::info!("Performing full sync of repository");
            
            // Get the tree for the commit
            let tree = branch_commit.tree()?;
            
            // Collect all files from tree
            let mut files_to_index = Vec::new();
            tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
                if let Some(name) = entry.name() {
                    if let Some(kind) = entry.kind() {
                        if kind == git2::ObjectType::Blob {
                            let path = if dir.is_empty() {
                                PathBuf::from(name)
                            } else {
                                PathBuf::from(dir).join(name)
                            };
                            files_to_index.push(path);
                        }
                    }
                }
                git2::TreeWalkResult::Ok
            })?;
            
            Ok((commit_oid_str, files_to_index, true))
        } else {
            Ok((commit_oid_str, Vec::new(), false))
        }
    }).await??;  // Handle both the task JoinError and the Result
    
    let (commit_oid_str, mut files_to_index, sync_performed) = git_result;
    
    // Return early if no files to index (no changes)
    if !sync_performed || files_to_index.is_empty() {
        return Ok(SyncResult {
            success: true,
            message: format!("Repository '{}' already synced to the latest commit on branch '{}'", 
                repo_name_outside, active_branch_outside),
            indexed_languages: Vec::new(),
        });
    }
    
    // Filter files by extension if specified
    if let Some(exts) = &extensions_filter {
        log::info!("Filtering files by extensions: {:?}", exts);
        files_to_index = files_to_index.into_iter()
            .filter(|path| {
                if let Some(ext) = path.extension() {
                    if let Some(ext_str) = ext.to_str() {
                        return exts.iter().any(|e| e == ext_str);
                    }
                }
                false
            })
            .collect::<Vec<_>>();
    }
    
    log::info!("Found {} files to index", files_to_index.len());
    
    // Create collection name
    let collection_name = repo_helpers::get_collection_name(&repo_name_outside);
    
    // Ensure the collection exists before indexing
    let default_dimension = repo_helpers::DEFAULT_VECTOR_DIMENSION;
    repo_helpers::ensure_repository_collection_exists(client.as_ref(), &collection_name, default_dimension).await?;
    
    // Perform indexing using the passed cli_args and app_config
    repo_helpers::index_files(
        client.as_ref(),
        None,
        None,
        app_config,
        &repo_path_outside,
        &files_to_index,
        &collection_name,
        &active_branch_outside,
        &commit_oid_str,
    ).await?;
    
    log::info!("Repository '{}' synced successfully to commit {}", repo_name_outside, commit_oid_str);
    
    // Query for indexed languages
    let mut indexed_languages = HashSet::new();
    let scroll_filter = repo_helpers::create_branch_filter(&active_branch_outside);
    
    // Create scroll request manually using Qdrant client libraries
    let mut scroll_request = ScrollPointsBuilder::new(&collection_name)
        .filter(scroll_filter.clone())
        .limit(100)
        .with_payload(PayloadIncludeSelector { fields: vec![FIELD_LANGUAGE.to_string()] })
        .with_vectors(false);
    
    // Loop through all points to collect languages
    loop {
        let response = client.scroll(scroll_request.into()).await?;
        let points = response.result;
        
        if points.is_empty() {
            break;
        }
        
        // Collect languages from each point
        for point in points {
            let payload = point.payload;
            if let Some(lang_val) = payload.get(FIELD_LANGUAGE) { 
                match lang_val.kind.as_ref() {
                    Some(qdrant_client::qdrant::value::Kind::StringValue(lang)) => {
                        indexed_languages.insert(lang.clone());
                    },
                    _ => {
                        log::warn!("Non-string value found for FIELD_LANGUAGE: {:?}", lang_val);
                    }
                }
            }
        }
        
        // Update the scroll request for the next batch
        if let Some(next_offset) = response.next_page_offset {
            let scroll_filter_clone = scroll_filter.clone();
            scroll_request = ScrollPointsBuilder::new(&collection_name)
                .filter(scroll_filter_clone)
                .limit(100)
                .with_payload(PayloadIncludeSelector { fields: vec![FIELD_LANGUAGE.to_string()] })
                .with_vectors(false)
                .offset(next_offset);
        } else {
            break;  // No more results
        }
    }
    
    // Sort languages for consistent output
    let mut indexed_languages: Vec<String> = indexed_languages.into_iter().collect();
    indexed_languages.sort();
    
    if !indexed_languages.is_empty() {
        log::info!("Indexed languages: {:?}", indexed_languages);
    }
    
    Ok(SyncResult {
        success: true,
        message: format!("Successfully synced repository '{}' on branch '{}'", 
            repo_name_outside, active_branch_outside),
        indexed_languages,
    })
}

/// Options for repository sync
#[derive(Debug)]
pub struct SyncOptions {
    pub force: bool,
    pub extensions: Option<Vec<String>>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            force: false,
            extensions: None,
        }
    }
} 