use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use serde::{Serialize, Deserialize};
use anyhow::{Result, anyhow};
use log::{debug, info, warn, error};

use crate::vectordb::repo::{GitRepoConfig, canonical_repo_id};

/// Manager for multiple repository configurations
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RepoManager {
    /// Map of repository ID to configuration
    repositories: HashMap<String, GitRepoConfig>,
    /// Currently active repository ID
    active_repo_id: Option<String>,
    /// Path to the repository manager configuration file
    #[serde(skip)]
    config_path: PathBuf,
}

impl RepoManager {
    /// Create a new repository manager
    pub fn new(config_path: PathBuf) -> Result<Self> {
        debug!("Creating repository manager with config path: {}", config_path.display());
        
        // Try to load existing configuration
        if config_path.exists() {
            debug!("Config file exists, attempting to load");
            match fs::read_to_string(&config_path) {
                Ok(contents) => {
                    match serde_json::from_str::<RepoManager>(&contents) {
                        Ok(mut manager) => {
                            debug!("Successfully loaded repository manager with {} repositories", 
                                  manager.repositories.len());
                            
                            // Set the config path (not deserialized)
                            manager.config_path = config_path;
                            
                            // Validate all repository paths exist
                            let to_deactivate = manager.repositories.iter()
                                .filter(|(_, repo)| !repo.path.exists())
                                .map(|(id, _)| id.clone())
                                .collect::<Vec<_>>();
                            
                            // Deactivate repositories with missing paths
                            for id in to_deactivate {
                                if let Some(repo) = manager.repositories.get_mut(&id) {
                                    debug!("Repository path no longer exists, deactivating: {}", repo.path.display());
                                    repo.active = false;
                                }
                            }
                            
                            Ok(manager)
                        },
                        Err(e) => {
                            warn!("Failed to parse repository manager config: {}", e);
                            
                            // Create new empty manager on error
                            let manager = RepoManager {
                                repositories: HashMap::new(),
                                active_repo_id: None,
                                config_path,
                            };
                            
                            // Save the new manager
                            manager.save()?;
                            
                            Ok(manager)
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to read repository manager config: {}", e);
                    
                    // Create new empty manager on error
                    let manager = RepoManager {
                        repositories: HashMap::new(),
                        active_repo_id: None,
                        config_path,
                    };
                    
                    // Save the new manager
                    manager.save()?;
                    
                    Ok(manager)
                }
            }
        } else {
            debug!("Config file doesn't exist, creating new repository manager");
            
            // Create parent directory if it doesn't exist
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            // Create new empty manager
            let manager = RepoManager {
                repositories: HashMap::new(),
                active_repo_id: None,
                config_path,
            };
            
            // Save the new manager
            manager.save()?;
            
            Ok(manager)
        }
    }
    
    /// Add a new repository
    pub fn add_repository(&mut self, path: PathBuf, name: Option<String>) -> Result<String> {
        debug!("Adding repository: {}", path.display());
        
        // Generate canonical ID
        let id = canonical_repo_id(&path)?;
        debug!("Generated repository ID: {}", id);
        
        // Check if repository already exists with this ID
        if self.repositories.contains_key(&id) {
            return Err(anyhow!("Repository with this path already exists"));
        }
        
        // Create the repository configuration
        let repo_config = GitRepoConfig::new(path, name, id.clone())?;
        
        // Add to the collection
        self.repositories.insert(id.clone(), repo_config);
        
        // If this is the first repository, set it as active
        if self.active_repo_id.is_none() {
            debug!("Setting first repository as active: {}", id);
            self.active_repo_id = Some(id.clone());
        }
        
        // Save changes
        self.save()?;
        
        Ok(id)
    }
    
    /// Remove a repository by ID
    pub fn remove_repository(&mut self, id: &str) -> Result<()> {
        debug!("Removing repository: {}", id);
        
        // Check if repository exists
        if !self.repositories.contains_key(id) {
            return Err(anyhow!("Repository with ID '{}' not found", id));
        }
        
        // Remove the repository
        self.repositories.remove(id);
        
        // If this was the active repository, clear the active ID
        if let Some(active_id) = &self.active_repo_id {
            if active_id == id {
                debug!("Removing active repository, clearing active ID");
                self.active_repo_id = None;
                
                // Set the first repository (if any) as active
                if let Some((first_id, _)) = self.repositories.iter().next() {
                    debug!("Setting next repository as active: {}", first_id);
                    self.active_repo_id = Some(first_id.clone());
                }
            }
        }
        
        // Save changes
        self.save()?;
        
        Ok(())
    }
    
    /// Get a repository by ID
    pub fn get_repository(&self, id: &str) -> Option<&GitRepoConfig> {
        self.repositories.get(id)
    }
    
    /// Get a mutable reference to a repository by ID
    pub fn get_repository_mut(&mut self, id: &str) -> Option<&mut GitRepoConfig> {
        self.repositories.get_mut(id)
    }
    
    /// Set the active repository
    pub fn set_active_repository(&mut self, id: &str) -> Result<()> {
        debug!("Setting active repository: {}", id);
        
        // Check if repository exists
        if !self.repositories.contains_key(id) {
            return Err(anyhow!("Repository with ID '{}' not found", id));
        }
        
        // Set as active
        self.active_repo_id = Some(id.to_string());
        
        // Save changes
        self.save()?;
        
        Ok(())
    }
    
    /// Get the active repository
    pub fn get_active_repository(&self) -> Option<&GitRepoConfig> {
        self.active_repo_id.as_ref().and_then(|id| self.get_repository(id))
    }
    
    /// Get the active repository ID
    pub fn get_active_repository_id(&self) -> Option<&String> {
        self.active_repo_id.as_ref()
    }
    
    /// List all repositories
    pub fn list_repositories(&self) -> Vec<&GitRepoConfig> {
        self.repositories.values().collect()
    }
    
    /// List active repositories
    pub fn list_active_repositories(&self) -> Vec<&GitRepoConfig> {
        self.repositories.values()
            .filter(|repo| repo.active)
            .collect()
    }
    
    /// Check if repository ID exists
    pub fn id_exists(&self, id: &str) -> bool {
        self.repositories.contains_key(id)
    }
    
    /// Resolve a repository name or ID to an ID
    pub fn resolve_repo_name_to_id(&self, name_or_id: &str) -> Result<String> {
        // Check if it's an exact ID match
        if self.repositories.contains_key(name_or_id) {
            return Ok(name_or_id.to_string());
        }
        
        // Look for a repository with this name
        for (id, repo) in &self.repositories {
            if repo.name == name_or_id {
                return Ok(id.clone());
            }
        }
        
        // Not found
        Err(anyhow!("Repository with name or ID '{}' not found", name_or_id))
    }
    
    /// Update the indexed commit hash for a repository branch
    pub fn update_indexed_commit(&mut self, repo_id: &str, branch: &str, commit_hash: &str) -> Result<()> {
        debug!("Updating indexed commit for repository {}, branch {}: {}", 
              repo_id, branch, commit_hash);
        
        // Get the repository
        let repo = self.get_repository_mut(repo_id)
            .ok_or_else(|| anyhow!("Repository with ID '{}' not found", repo_id))?;
        
        // Update the commit hash
        repo.update_indexed_commit(branch, commit_hash);
        
        // Save changes
        self.save()?;
        
        Ok(())
    }
    
    /// Enable auto-sync for a repository
    pub fn enable_auto_sync(&mut self, repo_id: &str, min_interval: Option<u64>) -> Result<()> {
        debug!("Enabling auto-sync for repository {}", repo_id);
        
        // Get the repository
        let repo = self.get_repository_mut(repo_id)
            .ok_or_else(|| anyhow!("Repository with ID '{}' not found", repo_id))?;
        
        // Enable auto-sync
        repo.enable_auto_sync(min_interval);
        
        // Save changes
        self.save()?;
        
        Ok(())
    }
    
    /// Disable auto-sync for a repository
    pub fn disable_auto_sync(&mut self, repo_id: &str) -> Result<()> {
        debug!("Disabling auto-sync for repository {}", repo_id);
        
        // Get the repository
        let repo = self.get_repository_mut(repo_id)
            .ok_or_else(|| anyhow!("Repository with ID '{}' not found", repo_id))?;
        
        // Disable auto-sync
        repo.disable_auto_sync();
        
        // Save changes
        self.save()?;
        
        Ok(())
    }
    
    /// Get repositories with auto-sync enabled
    pub fn get_auto_sync_repos(&self) -> Vec<&GitRepoConfig> {
        self.repositories.values()
            .filter(|repo| repo.active && repo.auto_sync.enabled)
            .collect()
    }
    
    /// Check if auto-sync is enabled for a repository
    pub fn is_auto_sync_enabled(&self, repo_id: &str) -> bool {
        self.get_repository(repo_id)
            .map(|repo| repo.auto_sync.enabled)
            .unwrap_or(false)
    }
    
    /// Start the auto-sync daemon in the VectorDB
    pub fn start_auto_sync_daemon(&self) -> Result<()> {
        // Implementation is in VectorDB
        Ok(())
    }
    
    /// Stop the auto-sync daemon in the VectorDB
    pub fn stop_auto_sync_daemon(&self) -> Result<()> {
        // Implementation is in VectorDB
        Ok(())
    }
    
    /// Save the repository manager configuration
    pub fn save(&self) -> Result<()> {
        debug!("Saving repository manager configuration to {}", self.config_path.display());
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Serialize to JSON
        let json = serde_json::to_string_pretty(self)?;
        
        // Write to file
        fs::write(&self.config_path, json)?;
        
        Ok(())
    }
} 