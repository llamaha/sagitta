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

use git2::{Repository, FetchOptions, RemoteCallbacks, AutotagOption, DiffOptions, Delta, DiffFindOptions};

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
    config: &mut AppConfig,
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
    let git_result = task::spawn_blocking(move || -> Result<(String, Vec<PathBuf>, Vec<PathBuf>, bool)> {
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
        
        // Check if we need a full sync or incremental sync
        let mut sync_type = SyncType::None; // Default to no sync needed
        let mut files_to_index = Vec::new();
        let mut files_to_delete = Vec::new();

        if force_sync {
            sync_type = SyncType::Full;
            log::info!("Force flag set, performing full sync.");
        } else if let Some(last_commit_oid_str) = last_synced_commits.get(&active_branch) {
            if last_commit_oid_str == &commit_oid_str {
                log::info!("Repository already synced to commit: {}", commit_oid_str);
                // sync_type remains SyncType::None
            } else {
                log::info!("Repository needs incremental syncing from {} to {}", last_commit_oid_str, commit_oid_str);
                sync_type = SyncType::Incremental;
                // Find the commit object for the last synced commit
                let last_commit_oid = git2::Oid::from_str(last_commit_oid_str)?;
                let last_commit = repo.find_commit(last_commit_oid)?;
                
                // Get trees for both commits
                let old_tree = last_commit.tree()?;
                let new_tree = branch_commit.tree()?;

                // Calculate the diff
                let mut diff_opts = DiffOptions::new();
                // Include untracked files? Maybe not for sync. Consider options.
                // diff_opts.include_untracked(true); 
                let diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut diff_opts))?;

                // Find renames/copies to handle them correctly (optional but good)
                // let mut find_opts = DiffFindOptions::new();
                // find_opts.renames(true);
                // diff.find_similar(Some(&mut find_opts))?; // This modifies the diff in place

                // Process the deltas
                for delta in diff.deltas() {
                    match delta.status() {
                        Delta::Added | Delta::Modified | Delta::Renamed | Delta::Copied | Delta::Typechange => {
                             if let Some(new_file_path) = delta.new_file().path() {
                                files_to_index.push(new_file_path.to_path_buf());
                             }
                        }
                        Delta::Deleted => {
                             if let Some(old_file_path) = delta.old_file().path() {
                                files_to_delete.push(old_file_path.to_path_buf());
                             }
                        }
                        Delta::Untracked | Delta::Unreadable | Delta::Ignored | Delta::Conflicted | Delta::Unmodified => {
                            // Ignore these statuses for sync purposes
                            log::trace!("Ignoring delta status {:?} for file: {:?}", delta.status(), delta.new_file().path());
                        }
                    }
                }
                log::info!("Incremental sync: {} files to index/update, {} files to delete.", files_to_index.len(), files_to_delete.len());

            }
        } else {
            // No last synced commit found for this branch, treat as initial full sync
            log::info!("No previous sync found for branch '{}'. Performing initial full sync.", active_branch);
            sync_type = SyncType::Full;
        }
        
        // If full sync is needed, get all files from the current tree
        if sync_type == SyncType::Full {
            log::info!("Performing full sync of repository tree");
            let tree = branch_commit.tree()?;
            tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
                if let Some(name) = entry.name() {
                    if let Some(kind) = entry.kind() {
                        if kind == git2::ObjectType::Blob {
                            let path = if dir.is_empty() {
                                PathBuf::from(name)
                            } else {
                                PathBuf::from(dir).join(name)
                            };
                            files_to_index.push(path); // Add all files for full sync
                        }
                    }
                }
                git2::TreeWalkResult::Ok
            })?;
            log::info!("Full sync: {} files to index.", files_to_index.len());
            files_to_delete.clear(); // Ensure no deletions are processed on a full sync
        }
        
        // Return results: commit OID, files to index, files to delete, and whether any sync action is needed
        Ok((commit_oid_str, files_to_index, files_to_delete, sync_type != SyncType::None))

    }).await??;  // Handle both the task JoinError and the Result
    
    let (commit_oid_str, mut files_to_index, files_to_delete, sync_performed) = git_result;
    
    // Return early if no sync action was performed
    if !sync_performed {
        // Message already logged inside the thread if up-to-date
        return Ok(SyncResult {
            success: true,
            message: format!("Repository '{}' already synced to the latest commit on branch '{}'", 
                repo_name_outside, active_branch_outside),
            indexed_languages: Vec::new(),
        });
    }
    
    // Filter files by extension if specified (applies to added/modified files)
    if let Some(exts) = &extensions_filter {
        log::info!("Filtering files to index by extensions: {:?}", exts);
        let original_count = files_to_index.len();
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
        log::info!("Filtered files to index count: {} (from {})", files_to_index.len(), original_count);
    }
    
    // Filter deleted files too? Probably not needed, delete based on path regardless of extension.
    
    log::info!("Files to index/update: {}, Files to delete: {}", files_to_index.len(), files_to_delete.len());
    
    // Create collection name
    let collection_name = repo_helpers::get_collection_name(&repo_name_outside);
    
    // Ensure the collection exists before indexing/deleting
    let default_dimension = repo_helpers::DEFAULT_VECTOR_DIMENSION;
    repo_helpers::ensure_repository_collection_exists(client.as_ref(), &collection_name, default_dimension).await?;
    
    // --- Handle Deletions ---
    if !files_to_delete.is_empty() {
        log::info!("Removing data for {} deleted files...", files_to_delete.len());
        // Use the actual function name and correct arguments
        repo_helpers::delete_points_for_files(
            client.as_ref(), // Pass client reference
            &collection_name, 
            &active_branch_outside,
            &files_to_delete, 
        ).await?; 
        log::info!("Finished removing data for deleted files.");
    }

    // --- Handle Indexing (Additions/Modifications) ---
    if !files_to_index.is_empty() {
        log::info!("Indexing {} added/modified files...", files_to_index.len());
        // Perform indexing using the passed cli_args and app_config
        repo_helpers::index_files(
            client.clone(),
            config,
            &repo_path_outside,
            &files_to_index,
            &collection_name,
            &active_branch_outside,
            &commit_oid_str,
        ).await?;
        log::info!("Finished indexing added/modified files.");
    } else {
        log::info!("No new or modified files to index.");
    }
    
    log::info!("Repository '{}' sync actions completed for commit {}", repo_name_outside, commit_oid_str);

    // --- Update Sync Status in Config ---
    // This should happen only if the sync operations were successful
    // Let the caller (handle_repo_sync) handle saving the config.
    // We just need to update the config object passed in mutably.
    if let Some(repo_idx) = config.repositories.iter().position(|r| r.name == repo_name_outside) {
        // No need to clone config, update it directly
        repo_helpers::update_sync_status_and_languages(
            config,
            repo_idx,
            &active_branch_outside,
            &commit_oid_str,
            client.as_ref(),
            &collection_name,
        ).await?;
         log::info!("Updated sync status for branch '{}' to commit {}", active_branch_outside, commit_oid_str);

    } else {
        log::error!("Could not find repository '{}' in config to update sync status.", repo_name_outside);
        // Return an error? Or just log?
    }


    // --- Query for indexed languages (after updates) ---
    // Note: This query might be slow on large collections. Consider if needed every time.
    log::info!("Querying for current set of indexed languages...");
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

// Helper enum for sync logic clarity
#[derive(PartialEq, Debug)]
enum SyncType {
    None,
    Incremental,
    Full,
} 