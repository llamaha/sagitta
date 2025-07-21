use anyhow::Result;
use sagitta_search::{AppConfig, RepositoryConfig, OrphanedRepository};
use sagitta_cli::cli::repo_commands::orphaned::{
    handle_orphaned_command, OrphanedArgs, OrphanedCommand, ListOrphanedArgs,
    RecloneArgs, AddOrphanedArgs, RemoveOrphanedArgs, CleanOrphanedArgs
};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::tempdir;
use std::fs;

fn create_test_config(base_path: &PathBuf) -> AppConfig {
    AppConfig {
        qdrant_url: "http://localhost:6334".to_string(),
        repositories: vec![],
        active_repository: None,
        repositories_base_path: Some(base_path.to_string_lossy().to_string()),
        embed_model: None,
        ..Default::default()
    }
}

fn create_test_repo_config(name: &str, url: &str, local_path: PathBuf, added_as_local: bool) -> RepositoryConfig {
    RepositoryConfig {
        name: name.to_string(),
        url: url.to_string(),
        local_path,
        default_branch: "main".to_string(),
        tracked_branches: vec!["main".to_string()],
        active_branch: Some("main".to_string()),
        remote_name: Some("origin".to_string()),
        ssh_key_path: None,
        ssh_key_passphrase: None,
        last_synced_commits: HashMap::new(),
        indexed_languages: None,
        added_as_local_path: added_as_local,
        target_ref: None,
        dependencies: Vec::new(),
        last_synced_commit: None,
    }
}

#[tokio::test]
async fn test_list_orphaned_empty() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::List(ListOrphanedArgs { json: false }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_list_orphaned_with_orphaned_and_missing() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    // Create configured repo directory
    fs::create_dir(&base_path.join("configured-repo"))?;
    
    // Create orphaned repo directory
    fs::create_dir(&base_path.join("orphaned-repo"))?;
    
    let mut config = create_test_config(&base_path);
    
    // Add configured repo that exists
    config.repositories.push(create_test_repo_config(
        "configured-repo",
        "https://example.com/configured.git",
        base_path.join("configured-repo"),
        false,
    ));
    
    // Add missing repo (in config but not on filesystem)
    config.repositories.push(create_test_repo_config(
        "missing-repo",
        "https://example.com/missing.git",
        base_path.join("missing-repo"),
        false,
    ));
    
    let args = OrphanedArgs {
        command: OrphanedCommand::List(ListOrphanedArgs { json: false }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_list_orphaned_json() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    // Create orphaned repo
    fs::create_dir(&base_path.join("orphaned-repo"))?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::List(ListOrphanedArgs { json: true }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_reclone_not_found() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    let mut config = create_test_config(&base_path);
    
    // Add a repository that is missing (so we have at least one missing repo)
    config.repositories.push(create_test_repo_config(
        "missing-repo",
        "https://example.com/missing.git",
        base_path.join("missing-repo"),
        false,
    ));
    
    // Don't create the directory so it's missing
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Reclone(RecloneArgs {
            name: Some("non-existent".to_string()),
            yes: true,
        }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found or not missing"));
    Ok(())
}

#[tokio::test]
async fn test_reclone_local_path_repo() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    let mut config = create_test_config(&base_path);
    
    // Add a repo that was added as local path
    config.repositories.push(create_test_repo_config(
        "local-repo",
        "local://path",
        base_path.join("local-repo"),
        true, // added_as_local_path
    ));
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Reclone(RecloneArgs {
            name: Some("local-repo".to_string()),
            yes: true,
        }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok()); // Should not error at command level, but no repos will be recloned
    Ok(())
}

#[tokio::test]
async fn test_add_orphaned_not_found() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Add(AddOrphanedArgs {
            name: "non-existent".to_string(),
            yes: true,
        }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
    Ok(())
}

#[tokio::test]
async fn test_add_orphaned_success() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    // Create orphaned repo
    fs::create_dir(&base_path.join("orphaned-repo"))?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Add(AddOrphanedArgs {
            name: "orphaned-repo".to_string(),
            yes: true,
        }),
    };
    
    // Use temp directory for config override
    let config_file = temp_dir.path().join("config.toml");
    
    let result = handle_orphaned_command(args, &mut config, Some(&config_file)).await;
    if let Err(e) = &result {
        eprintln!("Error in test_add_orphaned_success: {:?}", e);
    }
    assert!(result.is_ok());
    
    // Verify repo was added
    assert_eq!(config.repositories.len(), 1);
    assert_eq!(config.repositories[0].name, "orphaned-repo");
    assert!(config.repositories[0].added_as_local_path);
    
    Ok(())
}

#[tokio::test]
async fn test_remove_orphaned_not_found() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Remove(RemoveOrphanedArgs {
            name: "non-existent".to_string(),
            yes: true,
        }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
    Ok(())
}

#[tokio::test]
async fn test_remove_orphaned_success() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    // Create orphaned repo with some content
    let orphaned_path = base_path.join("orphaned-repo");
    fs::create_dir(&orphaned_path)?;
    fs::write(orphaned_path.join("test.txt"), "test content")?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Remove(RemoveOrphanedArgs {
            name: "orphaned-repo".to_string(),
            yes: true,
        }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok());
    
    // Verify directory was removed
    assert!(!orphaned_path.exists());
    
    Ok(())
}

#[tokio::test]
async fn test_clean_orphaned_empty() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Clean(CleanOrphanedArgs { yes: true }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_clean_orphaned_multiple() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    // Create multiple orphaned repos
    let orphaned1 = base_path.join("orphaned1");
    let orphaned2 = base_path.join("orphaned2");
    fs::create_dir(&orphaned1)?;
    fs::create_dir(&orphaned2)?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::Clean(CleanOrphanedArgs { yes: true }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok());
    
    // Verify directories were removed
    assert!(!orphaned1.exists());
    assert!(!orphaned2.exists());
    
    Ok(())
}

#[tokio::test]
async fn test_orphaned_with_git_repo() -> Result<()> {
    let temp_dir = tempdir()?;
    let base_path = temp_dir.path().join("repositories");
    fs::create_dir_all(&base_path)?;
    
    // Create orphaned git repo
    let git_repo = base_path.join("git-repo");
    fs::create_dir(&git_repo)?;
    fs::create_dir(&git_repo.join(".git"))?;
    
    let mut config = create_test_config(&base_path);
    
    let args = OrphanedArgs {
        command: OrphanedCommand::List(ListOrphanedArgs { json: false }),
    };
    
    let result = handle_orphaned_command(args, &mut config, None).await;
    assert!(result.is_ok());
    Ok(())
}