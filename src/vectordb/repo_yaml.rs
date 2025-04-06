use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use log::{debug, warn};

use crate::vectordb::embedding::EmbeddingModelType;
use crate::vectordb::repo_manager::RepoManager;

/// Configuration for a repository in YAML format
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RepoYamlConfig {
    /// Path to the repository
    pub path: String,
    /// Repository name (optional, derived from path if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// File types to index (optional, uses default if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_types: Option<Vec<String>>,
    /// Embedding model type (optional, uses default if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    /// Auto-sync configuration (optional, disabled by default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_sync: Option<bool>,
    /// Auto-sync interval in seconds (optional, uses default if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_sync_interval: Option<u64>,
}

/// Container for multiple repository configurations in YAML
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RepoYamlFile {
    pub repositories: Vec<RepoYamlConfig>,
}

/// Result of importing repositories from YAML
#[derive(Debug)]
pub struct ImportResult {
    pub successful: Vec<String>,
    pub failed: Vec<(String, String)>, // (repo_path, error_message)
    pub skipped: Vec<String>,
}

impl ImportResult {
    /// Create a new empty import result
    pub fn new() -> Self {
        Self {
            successful: Vec::new(),
            failed: Vec::new(),
            skipped: Vec::new(),
        }
    }
}

/// Import repositories from a YAML file
pub fn import_repositories_from_yaml(
    yaml_path: &Path,
    repo_manager: &mut RepoManager,
    skip_existing: bool,
) -> Result<ImportResult> {
    debug!("Importing repositories from YAML file: {}", yaml_path.display());
    
    // Check if file exists
    if !yaml_path.exists() {
        return Err(anyhow!("YAML file does not exist: {}", yaml_path.display()));
    }
    
    // Read the YAML file
    let yaml_content = fs::read_to_string(yaml_path)?;
    
    // Parse the YAML file
    let yaml_config: RepoYamlFile = serde_yaml::from_str(&yaml_content)
        .map_err(|e| anyhow!("Failed to parse YAML file: {}", e))?;
    
    if yaml_config.repositories.is_empty() {
        return Err(anyhow!("No repositories defined in YAML file"));
    }
    
    // Process each repository
    let mut result = ImportResult::new();
    
    for repo_config in yaml_config.repositories {
        // Convert path to absolute path
        let repo_path = if Path::new(&repo_config.path).is_absolute() {
            PathBuf::from(&repo_config.path)
        } else {
            // If path is relative, make it relative to the YAML file's directory
            let yaml_dir = yaml_path.parent().unwrap_or_else(|| Path::new("."));
            yaml_dir.join(&repo_config.path)
        };
        
        // Try to add the repository
        match add_repository_from_yaml(repo_manager, repo_path, &repo_config, skip_existing) {
            Ok(id) => {
                // Apply additional configuration if repository was successfully added
                if let Err(e) = configure_repository_from_yaml(repo_manager, &id, &repo_config) {
                    warn!("Failed to fully configure repository {}: {}", repo_config.path, e);
                }
                result.successful.push(repo_config.path);
            },
            Err(e) => {
                if e.to_string().contains("already exists") && skip_existing {
                    result.skipped.push(repo_config.path);
                } else {
                    result.failed.push((repo_config.path, e.to_string()));
                }
            }
        }
    }
    
    Ok(result)
}

/// Add a repository from YAML configuration
fn add_repository_from_yaml(
    repo_manager: &mut RepoManager,
    repo_path: PathBuf,
    repo_config: &RepoYamlConfig,
    skip_existing: bool,
) -> Result<String> {
    // Check if repository path exists
    if !repo_path.exists() {
        return Err(anyhow!("Repository path does not exist: {}", repo_path.display()));
    }
    
    // Check if this is a git repository
    let git_dir = repo_path.join(".git");
    if !git_dir.exists() {
        return Err(anyhow!("Not a git repository: {}", repo_path.display()));
    }
    
    // Get the repository name from config
    let repo_name = repo_config.name.clone();
    
    // Add repository to manager
    match repo_manager.add_repository(repo_path, repo_name) {
        Ok(id) => Ok(id),
        Err(e) => {
            if e.to_string().contains("already exists") && skip_existing {
                // This is handled by the caller to add to the skipped list
                Err(e)
            } else {
                Err(e)
            }
        }
    }
}

/// Configure repository from YAML after it's been added
fn configure_repository_from_yaml(
    repo_manager: &mut RepoManager,
    repo_id: &str,
    repo_config: &RepoYamlConfig,
) -> Result<()> {
    // Get the repository
    let repo = repo_manager.get_repository_mut(repo_id)
        .ok_or_else(|| anyhow!("Repository with ID '{}' not found", repo_id))?;
    
    // Configure file types if provided
    if let Some(file_types) = &repo_config.file_types {
        if !file_types.is_empty() {
            repo.file_types = file_types.clone();
        }
    }
    
    // Configure embedding model if provided
    if let Some(model_type) = &repo_config.embedding_model {
        match model_type.to_lowercase().as_str() {
            "fast" => {
                repo.embedding_model = Some(EmbeddingModelType::Fast);
            },
            "onnx" => {
                repo.embedding_model = Some(EmbeddingModelType::Onnx);
            },
            _ => {
                warn!("Unknown embedding model type: {}", model_type);
            }
        }
    }
    
    // Configure auto-sync if provided
    if let Some(auto_sync) = repo_config.auto_sync {
        if auto_sync {
            repo.enable_auto_sync(repo_config.auto_sync_interval);
        } else {
            repo.disable_auto_sync();
        }
    }
    
    // Save changes
    repo_manager.save()?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use std::path::PathBuf;
    
    fn create_test_yaml(content: &str) -> Result<NamedTempFile> {
        let mut file = NamedTempFile::new()?;
        file.write_all(content.as_bytes())?;
        Ok(file)
    }
    
    #[test]
    fn test_parse_yaml_file() {
        let yaml_content = r#"
        repositories:
          - path: /path/to/repo1
            name: my-repo1
            file_types:
              - rs
              - go
            embedding_model: onnx
            
          - path: /path/to/repo2
            name: my-repo2
            file_types:
              - rs
              - py
            embedding_model: fast
            auto_sync: true
            auto_sync_interval: 3600
        "#;
        
        let file = create_test_yaml(yaml_content).unwrap();
        let yaml_file: RepoYamlFile = serde_yaml::from_str(yaml_content).unwrap();
        
        assert_eq!(yaml_file.repositories.len(), 2);
        assert_eq!(yaml_file.repositories[0].path, "/path/to/repo1");
        assert_eq!(yaml_file.repositories[0].name, Some("my-repo1".to_string()));
        assert_eq!(yaml_file.repositories[0].file_types, Some(vec!["rs".to_string(), "go".to_string()]));
        assert_eq!(yaml_file.repositories[0].embedding_model, Some("onnx".to_string()));
        assert_eq!(yaml_file.repositories[0].auto_sync, None);
        
        assert_eq!(yaml_file.repositories[1].path, "/path/to/repo2");
        assert_eq!(yaml_file.repositories[1].auto_sync, Some(true));
        assert_eq!(yaml_file.repositories[1].auto_sync_interval, Some(3600));
    }
} 