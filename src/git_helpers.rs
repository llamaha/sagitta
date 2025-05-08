use crate::config::RepositoryConfig;
use crate::error::VectorDBError;
use git2::Repository;
use log::info;
use anyhow::{Context, Result};
use crate::repo_helpers::create_fetch_options;

/// Switches the current HEAD of the repository to the specified branch.
pub fn switch_branch_impl(
    repo_config: &RepositoryConfig,
    branch_name: &str,
) -> Result<(), VectorDBError> {
    info!(
        "Switching repository '{}' to branch '{}'",
        repo_config.name,
        branch_name
    );
    let repo = Repository::open(&repo_config.local_path).map_err(|e| {
        VectorDBError::Other(format!("Failed to open repo {}: {}", repo_config.local_path.display(), e))
    })?;

    // Find the commit for the local branch
    let _branch_ref_name = format!("refs/heads/{}", branch_name); // Keep for clarity if needed later
    let branch_commit = repo.find_branch(branch_name, git2::BranchType::Local)
        .map_err(VectorDBError::GitError)? // Fixed: Move git error
        .get()
        .peel_to_commit()
        .map_err(VectorDBError::GitError)?; // Fixed: Move git error

    // Call merge_local_branch (which now contains the checkout logic)
    merge_local_branch(&repo, branch_name, &branch_commit)
        .map_err(|e| VectorDBError::GitMessageError(e.to_string()))?; // Fixed: Convert anyhow::Error

    info!(
        "Successfully switched repository '{}' to branch '{}'",
        repo_config.name,
        branch_name
    );
    Ok(())
}

/// Fetches the latest changes from the remote 'origin'.
pub fn fetch_remote_impl(
    repo_config: &RepositoryConfig,
) -> Result<(), VectorDBError> {
    info!(
        "Fetching latest changes for repository '{}' from remote '{}'",
        repo_config.name,
        repo_config.remote_name.as_deref().unwrap_or("<default>") // Fixed: Handle Option<String>
    );
    let repo = Repository::open(&repo_config.local_path).map_err(|e| {
        VectorDBError::Other(format!("Failed to open repo {}: {}", repo_config.local_path.display(), e))
    })?;

    let mut remote = repo
        .find_remote(&repo_config.remote_name.as_deref().unwrap_or("origin")) // Use default remote name if None
        .map_err(VectorDBError::GitError)?;

    // Pass necessary args to create_fetch_options
    let mut fetch_options = create_fetch_options(
        vec![repo_config.clone()],
        &repo_config.url,
        repo_config.ssh_key_path.as_ref(), // Fixed: Use ssh_key_path
        repo_config.ssh_key_passphrase.as_deref(), // Fixed: Use ssh_key_passphrase
    ).map_err(|e| VectorDBError::GitMessageError(e.to_string()))?; // Fixed: Convert anyhow::Error
    
    // Perform the fetch
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .map_err(VectorDBError::GitError)?; // Fixed: Move git error

    info!("Successfully fetched latest changes from remote '{}' for repository '{}'", 
         repo_config.remote_name.as_deref().unwrap_or("<default>"), // Fixed: Handle Option<String>
         repo_config.name
    );
    Ok(())
}

pub(crate) fn merge_local_branch<'repo>(
    repo: &'repo Repository,
    branch_name: &str,
    target_commit: &git2::Commit<'repo>,
) -> Result<()> {
    // Get the reference for the local branch
    let branch_ref_name = format!("refs/heads/{}", branch_name);
    let mut local_branch_ref = repo.find_reference(&branch_ref_name)
        .with_context(|| format!("Failed to find local branch reference: {}", branch_ref_name))?;

    // Get the commit object for the target commit

    // Get the current HEAD reference
    let head = repo.head().with_context(|| "Failed to get HEAD reference")?;

    // Check if the branch is already the current HEAD and pointing to the target commit
    if head.is_branch() && head.name() == Some(&branch_ref_name) && head.target() == Some(target_commit.id()) {
        info!("Branch '{}' is already up-to-date with the target commit.", branch_name);
        return Ok(());
    }

    // Checkout the target commit's tree first
    let commit_object = target_commit.as_object(); // Get &Object from Commit
    repo.checkout_tree(
        commit_object, // Pass &Object
        Some(
            git2::build::CheckoutBuilder::new()
                .force()
                .remove_untracked(true),
        ),
    )
    .with_context(|| format!("Failed to checkout tree for commit {}", target_commit.id()))?;

    // Update the branch reference to point to the target commit
    local_branch_ref.set_target(target_commit.id(), "Fast-forwarding branch")
        .with_context(|| format!("Failed to set target for branch '{}'", branch_name))?;

    // Update HEAD to point to the branch if it wasn't already
    if head.name() != Some(&branch_ref_name) {
        repo.set_head(&branch_ref_name)
            .with_context(|| format!("Failed to set HEAD to branch '{}'", branch_ref_name))?;
    }

    info!("Successfully merged branch '{}' to commit {}", branch_name, target_commit.id());
    Ok(())
}