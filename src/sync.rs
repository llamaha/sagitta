// crates/vectordb-core/src/sync.rs

use crate::config::{AppConfig, RepositoryConfig};
use crate::constants::FIELD_LANGUAGE;
use crate::qdrant_client_trait::QdrantClientTrait;
use crate::repo_helpers; // Use core repo_helpers
use crate::error::Result; // Use core Result
use anyhow::{anyhow, Context}; // Ensure anyhow macro is imported

// Remove CLI specific import
// use crate::cli::CliArgs;

use qdrant_client::qdrant::{
    ScrollPointsBuilder,
    PayloadIncludeSelector,
    value::Kind as QdrantValueKind, // Alias to avoid conflict
};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    io::Write, // Add Write trait
};
use tokio::task;
use git2::{Repository, FetchOptions, RemoteCallbacks, AutotagOption, DiffOptions, Delta, Oid as GitOid};
use log::{info, warn, debug, trace};

/// Contains the results of a repository synchronization operation.
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Indicates whether the sync operation completed without fatal errors.
    pub success: bool,
    /// A summary message describing the outcome of the sync.
    pub message: String,
    /// List of programming languages detected and indexed in the repository after the sync.
    pub indexed_languages: Vec<String>,
    /// The Git commit hash that the repository was synced to.
    pub last_synced_commit: Option<String>,
    /// The number of files newly indexed or updated during the sync.
    pub files_indexed: usize,
    /// The number of files deleted from the index during the sync.
    pub files_deleted: usize,
}

/// Options to control the behavior of the repository synchronization process.
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// If true, forces a full re-indexing of the repository, ignoring the last synced commit.
    pub force: bool,
    /// If set, only files with these extensions will be indexed.
    pub extensions: Option<Vec<String>>,
    // Add other potential options here if needed later, e.g., prune
}

// Helper enum for sync logic clarity
#[derive(PartialEq, Debug)]
enum SyncType {
    None,
    Incremental,
    Full,
}

/// Performs the synchronization process for a single repository.
/// 
/// This involves:
/// 1. Checking the target commit (based on `target_ref` or remote/local branch).
/// 2. Fetching from remote if applicable.
/// 3. Determining if a sync (full or incremental) is needed based on the last synced commit.
/// 4. Calculating file changes (added/modified/deleted) for incremental syncs.
/// 5. Deleting data for removed files from Qdrant.
/// 6. Indexing data for new/modified files into Qdrant.
/// 7. Returning the sync status and the commit ID that was synced.
pub async fn sync_repository<C>(
    client: Arc<C>,
    repo_config: &RepositoryConfig, // Use immutable ref
    options: SyncOptions,
    app_config: &AppConfig, // Pass AppConfig for embedding/language config
) -> Result<SyncResult>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    info!("Synchronizing repository: {}", repo_config.name);
    
    // --- Gather necessary info upfront ---
    let repo_name = &repo_config.name;
    let repo_path = &repo_config.local_path;
    let is_local_path_repo = repo_config.added_as_local_path;
    let active_branch = repo_config.active_branch.as_deref().unwrap_or("main"); // Default branch if not set
    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin");
    let last_synced_commits = &repo_config.last_synced_commits;
    let target_ref = repo_config.target_ref.as_deref();
    let force_sync = options.force;
    let extensions_filter = &options.extensions;

    // Clone values needed inside the blocking task
    let repo_path_clone = repo_path.clone();
    let remote_name_clone = remote_name.to_string();
    let active_branch_clone = active_branch.to_string();
    let last_synced_commit_map_clone = last_synced_commits.clone(); // Clone the map
    let target_ref_clone = target_ref.map(String::from);
    let is_local_path_repo_clone = is_local_path_repo;
    
    info!(
        "Starting sync: repo='{}', path='{}', branch/ref='{}', remote='{}', local={}, force={}",
        repo_name,
        repo_path.display(),
        target_ref.unwrap_or(active_branch),
        remote_name,
        is_local_path_repo,
        force_sync
    );
    
    // --- Step 1: Git Operations (blocking) ---
    let git_result = task::spawn_blocking(move || -> Result<(String, Vec<PathBuf>, Vec<PathBuf>, bool)> {
        debug!("Inside blocking task for Git operations.");
        let repo = Repository::open(&repo_path_clone)
            .with_context(|| format!("Failed to open repository at {}", repo_path_clone.display()))?;
        
        let commit_oid_str: String;
        let branch_commit: git2::Commit;
        let current_branch_name = target_ref_clone.as_deref().unwrap_or(&active_branch_clone);

        // Determine target commit (based on target_ref or branch)
        if let Some(target_ref) = &target_ref_clone {
            info!("Repository configured with target_ref: '{}'. Syncing static commit.", target_ref);
            let object = repo.revparse_single(target_ref)
                .with_context(|| format!("Could not find commit object for target_ref '{}'", target_ref))?;
            branch_commit = object.peel_to_commit()
                .with_context(|| format!("Target ref '{}' did not resolve to a commit.", target_ref))?;
            commit_oid_str = branch_commit.id().to_string();
            debug!("Found commit {} for target_ref '{}'", commit_oid_str, target_ref);
        } else {
            // Logic for syncing a branch (either remote-tracking or local)
            let branch_name = &active_branch_clone;
            if !is_local_path_repo_clone {
                debug!("Repository treated as remote. Fetching...");
                // Fetch from remote
                let mut remote = repo.find_remote(&remote_name_clone)
                    .with_context(|| format!("Could not find remote '{}'", remote_name_clone))?;
                
                // Use the appropriate create_fetch_options from repo_helpers::git_utils
                // We need AppConfig or RepositoryConfig vec here - how to pass?
                // For now, create basic options. Need to refactor create_fetch_options later.
                let mut fetch_opts = FetchOptions::new();
                let mut callbacks = RemoteCallbacks::new();
                // Basic progress logging
                callbacks.transfer_progress(|progress| {
                    // Write progress to stderr
                    eprint!("Fetching objects: {}/{} \r", progress.received_objects(), progress.total_objects());
                    let _ = std::io::stderr().flush(); // Flush stderr
                    true
                });
                // Add credential callback if needed (requires passing config info)
                fetch_opts.remote_callbacks(callbacks);
                fetch_opts.download_tags(AutotagOption::All);
                
                match remote.fetch(&[branch_name], Some(&mut fetch_opts), None) {
                    Ok(_) => info!("Fetched remote '{}' for branch '{}'", remote_name_clone, branch_name),
                    Err(e) => {
                        warn!("Failed to fetch specific branch '{}' from remote '{}': {}. Proceeding with local refs.", branch_name, remote_name_clone, e);
                        // Don't error out, maybe remote is unavailable but local is ok?
                    }
                };
                eprintln!(); // Newline to stderr after fetch progress

                // Find the remote-tracking branch reference
                let ref_name = format!("refs/remotes/{}/{}", remote_name_clone, branch_name);
                branch_commit = repo.find_reference(&ref_name)
                    .with_context(|| format!("Could not find remote-tracking reference '{}'. Was fetch successful?", ref_name))?
                    .peel_to_commit()
                    .with_context(|| format!("Could not peel reference '{}' to commit.", ref_name))?;
                commit_oid_str = branch_commit.id().to_string();
            } else {
                debug!("Repository treated as local-only.");
                // Find the local branch reference
                let ref_name = format!("refs/heads/{}", branch_name);
                branch_commit = repo.find_reference(&ref_name)
                    .with_context(|| format!("Could not find local branch '{}'", ref_name))?
                    .peel_to_commit()
                    .with_context(|| format!("Could not peel reference '{}' to commit.", ref_name))?;
                commit_oid_str = branch_commit.id().to_string();
            }
        }
        debug!("Target commit for sync: {}", commit_oid_str);

        // --- Determine Sync Type and Calculate Diff ---
        let mut sync_type = SyncType::None;
        let mut files_to_index = Vec::new();
        let mut files_to_delete = Vec::new();

        let last_synced_oid_str = last_synced_commit_map_clone.get(current_branch_name);

        if force_sync {
            sync_type = SyncType::Full;
            info!("Force flag set, performing full sync.");
        } else if let Some(last_commit_str) = last_synced_oid_str {
            if last_commit_str == &commit_oid_str {
                info!("Repository branch '{}' already synced to commit: {}", current_branch_name, commit_oid_str);
            } else {
                info!("Branch '{}' needs incremental sync from {} to {}", current_branch_name, last_commit_str, commit_oid_str);
                sync_type = SyncType::Incremental;
                let last_commit_oid = GitOid::from_str(last_commit_str)
                    .with_context(|| format!("Invalid OID string in config for last sync: {}", last_commit_str))?;
                let last_commit = repo.find_commit(last_commit_oid)
                    .with_context(|| format!("Could not find last synced commit object: {}", last_commit_str))?;
                
                let old_tree = last_commit.tree()?;
                let new_tree = branch_commit.tree()?;

                let mut diff_opts = DiffOptions::new();
                let diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut diff_opts))?;

                diff.foreach(
                    &mut |delta, _progress| {
                        match delta.status() {
                            Delta::Added | Delta::Modified | Delta::Renamed | Delta::Copied | Delta::Typechange => {
                                if let Some(path) = delta.new_file().path() { files_to_index.push(path.to_path_buf()); }
                            }
                            Delta::Deleted => {
                                if let Some(path) = delta.old_file().path() { files_to_delete.push(path.to_path_buf()); }
                            }
                            _ => { trace!("Ignoring delta status {:?} for file: {:?}", delta.status(), delta.new_file().path()); }
                        }
                        true // Continue processing
                    },
                    None, None, None,
                )?; // Process deltas
                info!("Incremental sync: {} files to index/update, {} files to delete.", files_to_index.len(), files_to_delete.len());
            }
        } else {
            info!("No previous sync found for branch '{}'. Performing initial full sync.", current_branch_name);
            sync_type = SyncType::Full;
        }
        
        // If full sync, get all files from the target tree
        if sync_type == SyncType::Full {
            info!("Gathering all files for full sync...");
            let tree = branch_commit.tree()?;
            // Use the helper function from repo_helpers
            repo_helpers::collect_files_from_tree(&repo, &tree, &mut files_to_index, &PathBuf::new())?;
            info!("Full sync: {} files found in tree.", files_to_index.len());
            files_to_delete.clear(); // Ensure no deletions on full sync
        }
        
        Ok((commit_oid_str, files_to_index, files_to_delete, sync_type != SyncType::None))

    }).await.context("Git operation task failed")??; // Handle JoinError and inner Result
    
    let (commit_oid_str, mut files_to_index, files_to_delete, sync_needed) = git_result;
    
    if !sync_needed {
        return Ok(SyncResult {
            success: true,
            message: format!("Repository '{}' branch/ref '{}' already synced to commit {}", 
                repo_name, target_ref.unwrap_or(active_branch), commit_oid_str),
            indexed_languages: Vec::new(), // Or query existing?
            last_synced_commit: Some(commit_oid_str),
            files_indexed: 0,
            files_deleted: 0,
        });
    }
    
    // --- Filter Files By Extension ---
    if let Some(exts) = extensions_filter {
        info!("Filtering files to index by extensions: {:?}", exts);
        let original_count = files_to_index.len();
        files_to_index.retain(|path| {
            path.extension()
                .and_then(|os_str| os_str.to_str())
                .map(|ext| exts.iter().any(|allowed_ext| allowed_ext == ext))
                .unwrap_or(false)
        });
        info!("Filtered files to index count: {} (from {})", files_to_index.len(), original_count);
    }

    let files_to_delete_count = files_to_delete.len();
    let files_to_index_count = files_to_index.len();
    info!("Files to index/update: {}, Files to delete: {}", files_to_index_count, files_to_delete_count);
    
    // --- Qdrant Operations ---
    let tenant_id = repo_config.tenant_id.as_deref()
        .ok_or_else(|| anyhow!("Tenant ID missing in RepositoryConfig for repository '{}' during sync operation", repo_name))?;
    let collection_name = repo_helpers::get_collection_name(tenant_id, repo_name, app_config);
    let current_sync_branch_or_ref = target_ref.unwrap_or(active_branch);
    
    // Ensure collection exists (might need embedding dimension)
    // Get dimension from AppConfig or model - requires AppConfig here
    let embedding_handler = crate::embedding::EmbeddingHandler::new(app_config)
        .context("Failed to initialize embedding handler for sync")?;
    let embedding_dim = embedding_handler.dimension()
        .context("Failed to get embedding dimension for sync")?;
    repo_helpers::ensure_repository_collection_exists(client.as_ref(), &collection_name, embedding_dim as u64).await?;
    
    // Handle Deletions
    if !files_to_delete.is_empty() {
        info!("Removing data for {} deleted files...", files_to_delete.len());
        repo_helpers::delete_points_for_files(
            client.as_ref(),
            &collection_name, 
            current_sync_branch_or_ref,
            &files_to_delete, 
        ).await.context("Failed to delete points for removed files")?; 
        info!("Finished removing data for deleted files.");
    }

    // Handle Indexing (Additions/Modifications)
    if !files_to_index.is_empty() {
        info!("Indexing {} added/modified files...", files_to_index.len());
        // Pass AppConfig instead of mutable ref
        repo_helpers::index_files(
            client.clone(),
            app_config, 
            repo_path,
            &files_to_index,
            &collection_name,
            current_sync_branch_or_ref,
            &commit_oid_str,
        ).await.context("Failed to index new/modified files")?;
        info!("Finished indexing added/modified files.");
    } else {
        info!("No new or modified files to index.");
    }
    
    info!("Repository '{}' sync actions completed for commit {}", repo_name, commit_oid_str);

    // --- Query Indexed Languages --- 
    // (This logic remains largely the same, using the QdrantClientTrait)
    info!("Querying for current set of indexed languages...");
    let mut indexed_languages_set = HashSet::new();
    let scroll_filter = repo_helpers::create_branch_filter(current_sync_branch_or_ref);
    let mut scroll_offset: Option<qdrant_client::qdrant::PointId> = None;
    
    loop {
        let mut scroll_request = ScrollPointsBuilder::new(collection_name.clone())
            .filter(scroll_filter.clone())
            .limit(250) // Increased limit slightly
            .with_payload(PayloadIncludeSelector { fields: vec![FIELD_LANGUAGE.to_string()] })
            .with_vectors(false);
        if let Some(offset) = scroll_offset {
            scroll_request = scroll_request.offset(offset);
        }
        
        let response = client.scroll(scroll_request.into()).await
            .context("Failed to scroll points for language query")?;
            
        if response.result.is_empty() {
            break;
        }
        
        for point in response.result {
            if let Some(lang_val) = point.payload.get(FIELD_LANGUAGE) {
                if let Some(QdrantValueKind::StringValue(lang)) = &lang_val.kind {
                    indexed_languages_set.insert(lang.clone());
                }
            }
        }
        
        scroll_offset = response.next_page_offset;
        if scroll_offset.is_none() {
            break;
        }
    }
    
    let mut indexed_languages: Vec<String> = indexed_languages_set.into_iter().collect();
    indexed_languages.sort();
    info!("Indexed languages after sync: {:?}", indexed_languages);
    
    Ok(SyncResult {
        success: true,
        message: format!("Successfully synced repository '{}' branch/ref '{}' to commit {}", 
            repo_name, current_sync_branch_or_ref, commit_oid_str),
        indexed_languages,
        last_synced_commit: Some(commit_oid_str),
        files_indexed: files_to_index_count,
        files_deleted: files_to_delete_count,
    })
} 