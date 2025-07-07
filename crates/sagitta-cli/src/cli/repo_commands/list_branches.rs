use anyhow::Result;
use clap::Args;
use colored::*;
use git_manager::GitManager;
use sagitta_search::AppConfig;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct ListBranchesArgs {
    /// Optional name of the repository (defaults to active repository)
    pub name: Option<String>,
}

pub async fn handle_list_branches(
    args: ListBranchesArgs,
    config: &AppConfig,
) -> Result<()> {
    let repo_name = args.name.as_ref()
        .or(config.active_repository.as_ref())
        .ok_or_else(|| anyhow::anyhow!("No repository specified and no active repository set"))?;
    
    let repo_config = config.repositories.iter()
        .find(|r| r.name == *repo_name)
        .ok_or_else(|| anyhow::anyhow!("Repository '{}' not found", repo_name))?;
    
    let repo_path = PathBuf::from(&repo_config.local_path);
    let git_manager = GitManager::new();
    
    let branches = git_manager.list_branches(&repo_path)?;
    let current_branch = repo_config.active_branch.as_ref()
        .unwrap_or(&repo_config.default_branch);
    
    println!("Branches in repository '{}':", repo_name.cyan());
    for branch in branches {
        if branch == *current_branch {
            println!("  {} {}", "*".green(), branch.green().bold());
        } else {
            println!("    {branch}");
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sagitta_search::config::RepositoryConfig;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use git2::Repository;

    fn create_test_config(repo_name: &str, repo_path: &str, active_branch: Option<String>) -> AppConfig {
        let mut config = AppConfig::default();
        config.repositories.push(RepositoryConfig {
            name: repo_name.to_string(),
            url: "https://example.com/test.git".to_string(),
            local_path: PathBuf::from(repo_path),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::new(),
            active_branch,
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: vec![],
        });
        config.active_repository = Some(repo_name.to_string());
        config
    }

    fn create_test_repo_with_branches(path: &PathBuf, branches: &[&str]) -> Result<()> {
        let repo = Repository::init(path)?;
        
        // Create initial commit
        let sig = git2::Signature::now("Test User", "test@example.com")?;
        let tree_id = {
            let mut index = repo.index()?;
            index.write_tree()?
        };
        let tree = repo.find_tree(tree_id)?;
        
        let initial_commit = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        )?;
        
        // Create branches
        let commit = repo.find_commit(initial_commit)?;
        for branch_name in branches {
            repo.branch(branch_name, &commit, false)?;
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_list_branches_with_active_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Create test repository with branches
        create_test_repo_with_branches(&repo_path, &["feature-1", "feature-2", "bugfix"]).unwrap();
        
        let config = create_test_config("test-repo", repo_path.to_str().unwrap(), Some("main".to_string()));
        
        let args = ListBranchesArgs {
            name: None, // Use active repository
        };
        
        let result = handle_list_branches(args, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_branches_with_specific_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Create test repository with branches
        create_test_repo_with_branches(&repo_path, &["develop", "release"]).unwrap();
        
        let config = create_test_config("test-repo", repo_path.to_str().unwrap(), None);
        
        let args = ListBranchesArgs {
            name: Some("test-repo".to_string()),
        };
        
        let result = handle_list_branches(args, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_branches_no_active_repo() {
        let mut config = AppConfig::default();
        config.active_repository = None; // No active repository
        
        let args = ListBranchesArgs {
            name: None,
        };
        
        let result = handle_list_branches(args, &config).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "No repository specified and no active repository set"
        );
    }

    #[tokio::test]
    async fn test_list_branches_repo_not_found() {
        let config = AppConfig::default();
        
        let args = ListBranchesArgs {
            name: Some("non-existent-repo".to_string()),
        };
        
        let result = handle_list_branches(args, &config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository 'non-existent-repo' not found"));
    }

    #[tokio::test]
    async fn test_list_branches_with_custom_current_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Create test repository with branches
        create_test_repo_with_branches(&repo_path, &["feature-1", "feature-2"]).unwrap();
        
        // Set active branch to feature-1
        let config = create_test_config("test-repo", repo_path.to_str().unwrap(), Some("feature-1".to_string()));
        
        let args = ListBranchesArgs {
            name: None,
        };
        
        let result = handle_list_branches(args, &config).await;
        assert!(result.is_ok());
    }
} 