# Git Manager Migration Guide

This guide provides step-by-step instructions for migrating existing sagitta tools to use the new `git-manager` crate.

## Overview

The `git-manager` crate centralizes all git functionality with enhanced features:
- **Automatic resync** on branch switching
- **Merkle tree optimization** for efficient change detection
- **Unified API** across all tools
- **Comprehensive error handling** and recovery
- **Modular architecture** for maintainability

## Migration Timeline

### Phase 3: Migration Preparation ✅ COMPLETED
- [x] Finalized public API
- [x] Created migration guides
- [x] Added compatibility layers
- [x] Performance validation
- [x] Edge case testing

### Phase 4: Tool Migration (Current)
1. **sagitta-cli** migration
2. **sagitta-mcp** migration  
3. **sagitta-code** migration

## 1. sagitta-cli Migration

### Current State Analysis

**Files to migrate:**
- `src/cli/repo_commands/use_branch.rs` - Branch switching command
- `src/cli/repo_commands/sync.rs` - Repository sync operations
- Dependencies on `sagitta_search::repo_helpers::switch_repository_branch`

### Migration Steps

#### Step 1: Add git-manager dependency

**File:** `crates/sagitta-cli/Cargo.toml`
```toml
[dependencies]
git-manager = { path = "../git-manager" }
# ... existing dependencies
```

#### Step 2: Update use_branch.rs

**File:** `crates/sagitta-cli/src/cli/repo_commands/use_branch.rs`

**Before:**
```rust
use sagitta_search::repo_helpers::switch_repository_branch;

pub async fn handle_use_branch(args: UseBranchArgs, config: &mut AppConfig, override_path: Option<&PathBuf>) -> Result<()> {
    // ... repository lookup logic ...
    
    switch_repository_branch(config, &repo_name_clone, target_branch_name)
        .context("Failed to switch repository branch")?;
    
    // ... config update logic ...
}
```

**After:**
```rust
use git_manager::{GitManager, SwitchOptions};
use std::path::PathBuf;

pub async fn handle_use_branch(args: UseBranchArgs, config: &mut AppConfig, override_path: Option<&PathBuf>) -> Result<()> {
    // ... repository lookup logic ...
    
    let repo_config = &config.repositories[repo_config_index];
    let repo_path = PathBuf::from(&repo_config.local_path);
    
    // Initialize git manager
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await
        .context("Failed to initialize repository")?;
    
    // Switch branch with automatic resync
    let switch_result = git_manager.switch_branch(&repo_path, target_branch_name).await
        .context("Failed to switch repository branch")?;
    
    // Update config with new branch
    let repo_config_mut = &mut config.repositories[repo_config_index];
    repo_config_mut.active_branch = Some(target_branch_name.to_string());
    if !repo_config_mut.tracked_branches.contains(target_branch_name) {
        repo_config_mut.tracked_branches.push(target_branch_name.to_string());
    }
    
    save_config(config, override_path)?;
    
    // Enhanced output with sync information
    println!("{}", format!(
        "Switched to branch '{}' for repository '{}'.",
        target_branch_name,
        repo_name_clone.cyan()
    ).green());
    
    if let Some(sync_result) = switch_result.sync_result {
        if sync_result.success {
            println!("🔄 Automatic resync completed: {} files updated, {} files added, {} files removed",
                sync_result.files_updated, sync_result.files_added, sync_result.files_removed);
        }
    }
    
    Ok(())
}
```

#### Step 3: Update sync.rs for enhanced sync detection

**File:** `crates/sagitta-cli/src/cli/repo_commands/sync.rs`

**Add sync requirement analysis:**
```rust
use git_manager::{GitManager, SyncType};

pub async fn handle_repo_sync<C>(
    args: SyncRepoArgs, 
    config: &mut AppConfig,
    client: Arc<C>,
    cli_args: &crate::cli::CliArgs,
    override_path: Option<&PathBuf>,
) -> Result<()>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // ... existing setup logic ...
    
    let repo_config = &config.repositories[repo_config_index];
    let repo_path = PathBuf::from(&repo_config.local_path);
    
    // Initialize git manager for enhanced sync detection
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await?;
    
    // Check current branch sync requirements
    let current_branch = repo_config.active_branch.as_ref()
        .unwrap_or(&repo_config.default_branch);
    
    let sync_req = git_manager.calculate_sync_requirements(&repo_path, current_branch).await?;
    
    match sync_req.sync_type {
        SyncType::None => {
            println!("✅ Repository is already up to date");
            return Ok(());
        },
        SyncType::Incremental => {
            println!("🔄 Incremental sync needed: {} files to update", 
                sync_req.files_to_update.len() + sync_req.files_to_add.len());
        },
        SyncType::Full => {
            println!("🔄 Full resync required");
        }
    }
    
    // ... continue with existing sync logic ...
}
```

#### Step 4: Add new branch management commands

**File:** `crates/sagitta-cli/src/cli/repo_commands/mod.rs`

**Add new commands:**
```rust
#[derive(Subcommand, Debug, Clone)]
pub enum RepoCommand {
    // ... existing commands ...
    
    /// List branches in a repository
    ListBranches(list_branches::ListBranchesArgs),
    /// Create a new branch
    CreateBranch(create_branch::CreateBranchArgs),
    /// Delete a branch
    DeleteBranch(delete_branch::DeleteBranchArgs),
    /// Show repository status
    Status(status::StatusArgs),
}
```

**Create new command files:**

**File:** `crates/sagitta-cli/src/cli/repo_commands/list_branches.rs`
```rust
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
            println!("    {}", branch);
        }
    }
    
    Ok(())
}
```

### Testing Strategy

1. **Unit tests** for new command handlers
2. **Integration tests** with real repositories
3. **Backward compatibility** validation
4. **Performance benchmarks** vs old implementation

## 2. sagitta-mcp Migration

### Current State Analysis

**Files to migrate:**
- `src/handlers/repository.rs` - Main repository operations
- `src/mcp/types.rs` - MCP type definitions
- `src/server.rs` - Request handling

### Migration Steps

#### Step 1: Add git-manager dependency

**File:** `crates/sagitta-mcp/Cargo.toml`
```toml
[dependencies]
git-manager = { path = "../git-manager" }
# ... existing dependencies
```

#### Step 2: Add new MCP endpoints

**File:** `crates/sagitta-mcp/src/mcp/types.rs`

**Add new parameter types:**
```rust
/// Parameters for branch switching
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySwitchBranchParams {
    /// Name of the repository
    pub repository_name: String,
    /// Target branch name
    pub branch_name: String,
    /// Force switch even with uncommitted changes
    #[serde(default)]
    pub force: bool,
    /// Disable automatic resync
    #[serde(default)]
    pub no_auto_resync: bool,
}

/// Result of branch switching
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySwitchBranchResult {
    /// Previous branch name
    pub previous_branch: String,
    /// New branch name
    pub new_branch: String,
    /// Whether sync was performed
    pub sync_performed: bool,
    /// Number of files changed
    pub files_changed: usize,
    /// Sync details if performed
    pub sync_details: Option<SyncDetails>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SyncDetails {
    pub files_added: usize,
    pub files_updated: usize,
    pub files_removed: usize,
}

/// Parameters for listing branches
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListBranchesParams {
    /// Name of the repository
    pub repository_name: String,
}

/// Result of listing branches
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryListBranchesResult {
    /// List of branch names
    pub branches: Vec<String>,
    /// Current active branch
    pub current_branch: String,
}
```

#### Step 3: Update repository handlers

**File:** `crates/sagitta-mcp/src/handlers/repository.rs`

**Add new handler functions:**
```rust
use git_manager::{GitManager, SwitchOptions};

/// Handle repository branch switching with automatic resync
#[instrument(skip(config, qdrant_client), fields(repo_name = %params.repository_name, target_branch = %params.branch_name))]
pub async fn handle_repository_switch_branch<C>(
    params: RepositorySwitchBranchParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    tenant_id_override: Option<String>,
) -> Result<RepositorySwitchBranchResult, ErrorObject>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let config_read_guard = config.read().await;
    
    // Find repository configuration
    let repo_config = config_read_guard.repositories.iter()
        .find(|r| r.name == params.repository_name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::REPO_NOT_FOUND,
            message: format!("Repository '{}' not found", params.repository_name),
            data: None,
        })?;
    
    let repo_path = PathBuf::from(&repo_config.local_path);
    drop(config_read_guard);
    
    // Initialize git manager
    let mut git_manager = GitManager::new();
    git_manager.initialize_repository(&repo_path).await
        .map_err(|e| ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
            message: format!("Failed to initialize repository: {}", e),
            data: None,
        })?;
    
    // Configure switch options
    let switch_options = SwitchOptions {
        force: params.force,
        auto_resync: !params.no_auto_resync,
        ..Default::default()
    };
    
    // Perform branch switch
    let switch_result = git_manager.switch_branch_with_options(
        &repo_path,
        &params.branch_name,
        switch_options,
    ).await.map_err(|e| ErrorObject {
        code: error_codes::GIT_OPERATION_FAILED,
        message: format!("Failed to switch branch: {}", e),
        data: None,
    })?;
    
    // Update repository configuration
    {
        let mut config_write_guard = config.write().await;
        if let Some(repo_mut) = config_write_guard.repositories.iter_mut()
            .find(|r| r.name == params.repository_name) {
            repo_mut.active_branch = Some(params.branch_name.clone());
            if !repo_mut.tracked_branches.contains(&params.branch_name) {
                repo_mut.tracked_branches.push(params.branch_name.clone());
            }
        }
        
        // Save configuration
        sagitta_search::config::save_config(&*config_write_guard, None)
            .map_err(|e| ErrorObject {
                code: error_codes::CONFIG_SAVE_FAILED,
                message: format!("Failed to save configuration: {}", e),
                data: None,
            })?;
    }
    
    // Prepare result
    let sync_details = switch_result.sync_result.map(|sync| SyncDetails {
        files_added: sync.files_added,
        files_updated: sync.files_updated,
        files_removed: sync.files_removed,
    });
    
    Ok(RepositorySwitchBranchResult {
        previous_branch: switch_result.previous_branch,
        new_branch: switch_result.new_branch,
        sync_performed: switch_result.sync_result.is_some(),
        files_changed: switch_result.files_changed,
        sync_details,
    })
}

/// Handle listing repository branches
#[instrument(skip(config), fields(repo_name = %params.repository_name))]
pub async fn handle_repository_list_branches(
    params: RepositoryListBranchesParams,
    config: Arc<RwLock<AppConfig>>,
    _tenant_id_override: Option<String>,
) -> Result<RepositoryListBranchesResult, ErrorObject> {
    let config_read_guard = config.read().await;
    
    // Find repository configuration
    let repo_config = config_read_guard.repositories.iter()
        .find(|r| r.name == params.repository_name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::REPO_NOT_FOUND,
            message: format!("Repository '{}' not found", params.repository_name),
            data: None,
        })?;
    
    let repo_path = PathBuf::from(&repo_config.local_path);
    let current_branch = repo_config.active_branch.clone()
        .unwrap_or_else(|| repo_config.default_branch.clone());
    
    drop(config_read_guard);
    
    // List branches using git manager
    let git_manager = GitManager::new();
    let branches = git_manager.list_branches(&repo_path)
        .map_err(|e| ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
            message: format!("Failed to list branches: {}", e),
            data: None,
        })?;
    
    Ok(RepositoryListBranchesResult {
        branches,
        current_branch,
    })
}
```

#### Step 4: Update server request handling

**File:** `crates/sagitta-mcp/src/server.rs`

**Add new endpoints:**
```rust
match request.method.as_str() {
    // ... existing endpoints ...
    
    "repository/switch_branch" | "mcp_sagitta_mcp_repository_switch_branch" => {
        let params: RepositorySwitchBranchParams = deserialize_params(request.params, "repository/switch_branch")?;
        let result = handle_repository_switch_branch(params, config, qdrant_client, None).await?;
        ok_some(result)
    }
    "repository/list_branches" | "mcp_sagitta_mcp_repository_list_branches" => {
        let params: RepositoryListBranchesParams = deserialize_params(request.params, "repository/list_branches")?;
        let result = handle_repository_list_branches(params, config, None).await?;
        ok_some(result)
    }
    
    // ... rest of endpoints ...
}
```

#### Step 5: Update tool definitions

**File:** `crates/sagitta-mcp/src/handlers/tool.rs`

**Add new tool definitions:**
```rust
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        // ... existing tools ...
        
        ToolDefinition {
            name: "repository_switch_branch".to_string(),
            description: Some("Switch to a different branch with automatic resync".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": { "type": "string", "description": "Name of the repository" },
                    "branch_name": { "type": "string", "description": "Target branch name" },
                    "force": { "type": "boolean", "description": "Force switch even with uncommitted changes", "default": false },
                    "no_auto_resync": { "type": "boolean", "description": "Disable automatic resync", "default": false }
                },
                "required": ["repository_name", "branch_name"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("Switch Branch".to_string()),
                read_only_hint: Some(false),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
        
        ToolDefinition {
            name: "repository_list_branches".to_string(),
            description: Some("List all branches in a repository".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": { "type": "string", "description": "Name of the repository" }
                },
                "required": ["repository_name"]
            }),
            annotations: Some(ToolAnnotations {
                title: Some("List Branches".to_string()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        },
    ]
}
```

## 3. sagitta-code Migration

### Current State Analysis

**Files to migrate:**
- `src/gui/repository/manager.rs` - Repository management
- `src/gui/repository/sync.rs` - Sync operations
- `src/tools/repository/` - Repository tools

### Migration Steps

#### Step 1: Add git-manager dependency

**File:** `crates/sagitta-code/Cargo.toml`
```toml
[dependencies]
git-manager = { path = "../git-manager" }
# ... existing dependencies
```

#### Step 2: Update RepositoryManager

**File:** `crates/sagitta-code/src/gui/repository/manager.rs`

**Add git manager integration:**
```rust
use git_manager::{GitManager, SwitchOptions, SyncType};

impl RepositoryManager {
    // Add git manager field
    git_manager: GitManager,
    
    pub fn new(config: Arc<Mutex<AppConfig>>) -> Self {
        Self {
            config,
            client: None,
            git_manager: GitManager::new(),
        }
    }
    
    /// Switch repository branch with automatic resync
    pub async fn switch_branch(&mut self, repo_name: &str, branch_name: &str) -> Result<()> {
        log::info!("[GUI RepoManager] Switching branch: {} -> {}", repo_name, branch_name);
        
        let config_guard = self.config.lock().await;
        let repo_config = config_guard.repositories.iter()
            .find(|r| r.name == repo_name)
            .ok_or_else(|| anyhow!("Repository '{}' not found", repo_name))?;
        
        let repo_path = PathBuf::from(&repo_config.local_path);
        drop(config_guard);
        
        // Initialize repository if not already done
        self.git_manager.initialize_repository(&repo_path).await?;
        
        // Check sync requirements first
        let sync_req = self.git_manager.calculate_sync_requirements(&repo_path, branch_name).await?;
        
        match sync_req.sync_type {
            SyncType::None => {
                log::info!("No sync required for branch switch");
            },
            SyncType::Incremental => {
                log::info!("Incremental sync will be performed: {} files to update", 
                    sync_req.files_to_update.len() + sync_req.files_to_add.len());
            },
            SyncType::Full => {
                log::info!("Full resync will be performed");
            }
        }
        
        // Perform the switch
        let switch_result = self.git_manager.switch_branch(&repo_path, branch_name).await?;
        
        // Update configuration
        {
            let mut config_guard = self.config.lock().await;
            if let Some(repo_mut) = config_guard.repositories.iter_mut()
                .find(|r| r.name == repo_name) {
                repo_mut.active_branch = Some(branch_name.to_string());
                if !repo_mut.tracked_branches.contains(&branch_name.to_string()) {
                    repo_mut.tracked_branches.push(branch_name.to_string());
                }
            }
            
            self.save_core_config_with_guard(&*config_guard).await?;
        }
        
        log::info!("Branch switch completed: {} -> {}", 
            switch_result.previous_branch, switch_result.new_branch);
        
        if let Some(sync_result) = switch_result.sync_result {
            log::info!("Automatic resync completed: {} files updated, {} added, {} removed",
                sync_result.files_updated, sync_result.files_added, sync_result.files_removed);
        }
        
        Ok(())
    }
    
    /// Get available branches for a repository
    pub async fn list_branches(&self, repo_name: &str) -> Result<Vec<String>> {
        let config_guard = self.config.lock().await;
        let repo_config = config_guard.repositories.iter()
            .find(|r| r.name == repo_name)
            .ok_or_else(|| anyhow!("Repository '{}' not found", repo_name))?;
        
        let repo_path = PathBuf::from(&repo_config.local_path);
        drop(config_guard);
        
        let branches = self.git_manager.list_branches(&repo_path)?;
        Ok(branches)
    }
}
```

#### Step 3: Update sync operations

**File:** `crates/sagitta-code/src/gui/repository/sync.rs`

**Add enhanced sync status:**
```rust
use git_manager::{GitManager, SyncType};

pub fn render_sync_panel(ui: &mut egui::Ui, state: &mut RepoPanelState, repo_manager: &Arc<Mutex<RepositoryManager>>) {
    // ... existing UI code ...
    
    // Add sync requirement analysis
    if ui.button("Analyze Sync Requirements").clicked() {
        let repo_manager_clone = Arc::clone(repo_manager);
        let selected_repos = state.selected_repos.clone();
        
        // Spawn analysis task
        tokio::spawn(async move {
            let manager = repo_manager_clone.lock().await;
            for repo_name in selected_repos {
                if let Ok(repo_config) = manager.get_repository_config(&repo_name) {
                    let repo_path = PathBuf::from(&repo_config.local_path);
                    let current_branch = repo_config.active_branch.as_ref()
                        .unwrap_or(&repo_config.default_branch);
                    
                    let mut git_manager = GitManager::new();
                    if let Ok(_) = git_manager.initialize_repository(&repo_path).await {
                        if let Ok(sync_req) = git_manager.calculate_sync_requirements(&repo_path, current_branch).await {
                            match sync_req.sync_type {
                                SyncType::None => log::info!("Repository '{}' is up to date", repo_name),
                                SyncType::Incremental => log::info!("Repository '{}' needs incremental sync: {} files", 
                                    repo_name, sync_req.files_to_update.len()),
                                SyncType::Full => log::info!("Repository '{}' needs full resync", repo_name),
                            }
                        }
                    }
                }
            }
        });
    }
    
    // ... rest of sync UI ...
}
```

#### Step 4: Add branch switching UI

**File:** `crates/sagitta-code/src/gui/repository/branch_manager.rs`

**Create new branch management UI:**
```rust
use git_manager::GitManager;
use super::types::{RepoPanelState, RepoPanelTab};
use super::manager::RepositoryManager;

pub fn render_branch_manager(
    ui: &mut egui::Ui, 
    state: &mut RepoPanelState, 
    repo_manager: &Arc<Mutex<RepositoryManager>>
) {
    ui.heading("Branch Management");
    
    if state.repositories.is_empty() {
        ui.label("No repositories available");
        return;
    }
    
    // Repository selection
    let mut selected_repo_name = state.selected_repo.clone().unwrap_or_default();
    egui::ComboBox::from_label("Repository")
        .selected_text(&selected_repo_name)
        .show_ui(ui, |ui| {
            for repo in &state.repositories {
                ui.selectable_value(&mut selected_repo_name, repo.name.clone(), &repo.name);
            }
        });
    
    if selected_repo_name != state.selected_repo.clone().unwrap_or_default() {
        state.selected_repo = Some(selected_repo_name.clone());
        // Trigger branch list refresh
        refresh_branch_list(state, repo_manager, &selected_repo_name);
    }
    
    if let Some(repo_name) = &state.selected_repo {
        ui.separator();
        
        // Current branch display
        if let Some(repo) = state.repositories.iter().find(|r| r.name == *repo_name) {
            ui.horizontal(|ui| {
                ui.label("Current branch:");
                ui.label(repo.branch.as_ref().unwrap_or(&"unknown".to_string()));
            });
        }
        
        // Available branches
        ui.label("Available branches:");
        
        // Branch list (populated by refresh_branch_list)
        if let Some(branches) = state.available_branches.get(repo_name) {
            for branch in branches {
                ui.horizontal(|ui| {
                    ui.label(&format!("  {}", branch));
                    
                    if ui.button("Switch").clicked() {
                        // Trigger branch switch
                        switch_to_branch(repo_manager, repo_name, branch);
                    }
                });
            }
        }
        
        ui.separator();
        
        // Create new branch
        ui.horizontal(|ui| {
            ui.label("New branch name:");
            ui.text_edit_singleline(&mut state.new_branch_name);
            
            if ui.button("Create").clicked() && !state.new_branch_name.is_empty() {
                create_new_branch(repo_manager, repo_name, &state.new_branch_name);
                state.new_branch_name.clear();
            }
        });
    }
}

fn refresh_branch_list(
    state: &mut RepoPanelState,
    repo_manager: &Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
) {
    let repo_manager_clone = Arc::clone(repo_manager);
    let repo_name_clone = repo_name.to_string();
    
    tokio::spawn(async move {
        let manager = repo_manager_clone.lock().await;
        if let Ok(branches) = manager.list_branches(&repo_name_clone).await {
            // Update state with branches (would need proper state management)
            log::info!("Available branches for {}: {:?}", repo_name_clone, branches);
        }
    });
}

fn switch_to_branch(
    repo_manager: &Arc<Mutex<RepositoryManager>>,
    repo_name: &str,
    branch_name: &str,
) {
    let repo_manager_clone = Arc::clone(repo_manager);
    let repo_name_clone = repo_name.to_string();
    let branch_name_clone = branch_name.to_string();
    
    tokio::spawn(async move {
        let mut manager = repo_manager_clone.lock().await;
        match manager.switch_branch(&repo_name_clone, &branch_name_clone).await {
            Ok(_) => log::info!("Successfully switched to branch: {}", branch_name_clone),
            Err(e) => log::error!("Failed to switch branch: {}", e),
        }
    });
}
```

#### Step 5: Update repository tools

**File:** `crates/sagitta-code/src/tools/repository/switch_branch.rs`

**Create new branch switching tool:**
```rust
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

#[derive(Debug, Deserialize, Serialize)]
pub struct SwitchBranchParams {
    pub repository_name: String,
    pub branch_name: String,
    pub force: Option<bool>,
}

#[derive(Debug)]
pub struct SwitchBranchTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl SwitchBranchTool {
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self { repo_manager }
    }
}

#[async_trait]
impl Tool for SwitchBranchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "switch_repository_branch".to_string(),
            description: "Switch to a different branch in a repository with automatic resync".to_string(),
            category: ToolCategory::Repository,
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Name of the repository"
                    },
                    "branch_name": {
                        "type": "string", 
                        "description": "Name of the branch to switch to"
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Force switch even with uncommitted changes",
                        "default": false
                    }
                },
                "required": ["repository_name", "branch_name"]
            }),
        }
    }
    
    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        let params: SwitchBranchParams = serde_json::from_value(parameters)
            .map_err(|e| SagittaCodeError::ToolError(format!("Invalid parameters: {}", e)))?;
        
        let mut repo_manager = self.repo_manager.lock().await;
        
        match repo_manager.switch_branch(&params.repository_name, &params.branch_name).await {
            Ok(_) => {
                Ok(ToolResult::Success(serde_json::json!({
                    "message": format!("Successfully switched to branch '{}' in repository '{}'", 
                        params.branch_name, params.repository_name),
                    "repository": params.repository_name,
                    "new_branch": params.branch_name
                })))
            },
            Err(e) => {
                Err(SagittaCodeError::ToolError(format!(
                    "Failed to switch branch in repository '{}': {}", 
                    params.repository_name, e
                )))
            }
        }
    }
}
```

## 4. Performance Optimization

### Benchmarking

Create performance benchmarks to ensure the new implementation meets or exceeds current performance:

**File:** `crates/git-manager/benches/migration_performance.rs`
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use git_manager::GitManager;
use std::path::PathBuf;
use tempfile::TempDir;

fn benchmark_branch_switching(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("branch_switch_with_resync", |b| {
        b.to_async(&rt).iter(|| async {
            let mut manager = GitManager::new();
            let repo_path = PathBuf::from("test_repo");
            
            // Benchmark the switch operation
            let result = manager.switch_branch(black_box(&repo_path), black_box("main")).await;
            black_box(result)
        });
    });
}

fn benchmark_sync_detection(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("sync_requirement_calculation", |b| {
        b.to_async(&rt).iter(|| async {
            let mut manager = GitManager::new();
            let repo_path = PathBuf::from("test_repo");
            
            let result = manager.calculate_sync_requirements(black_box(&repo_path), black_box("develop")).await;
            black_box(result)
        });
    });
}

criterion_group!(benches, benchmark_branch_switching, benchmark_sync_detection);
criterion_main!(benches);
```

### Memory Usage Optimization

Monitor memory usage during migration:

```rust
// Add to GitManager
impl GitManager {
    /// Get memory usage statistics
    pub fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            state_manager_size: std::mem::size_of_val(&self.state_manager),
            merkle_manager_size: std::mem::size_of_val(&self.merkle_manager),
            branch_switcher_size: std::mem::size_of_val(&self.branch_switcher),
        }
    }
}
```

## 5. Testing Strategy

### Integration Tests

**File:** `crates/git-manager/tests/migration_integration.rs`
```rust
use git_manager::GitManager;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_cli_migration_compatibility() {
    // Test that git-manager provides same functionality as old CLI
    let temp_dir = TempDir::new().unwrap();
    let repo_path = create_test_repo(&temp_dir);
    
    let mut manager = GitManager::new();
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Test branch switching
    let result = manager.switch_branch(&repo_path, "develop").await.unwrap();
    assert!(result.success);
    assert_eq!(result.new_branch, "develop");
}

#[tokio::test]
async fn test_mcp_migration_compatibility() {
    // Test that git-manager provides same functionality as old MCP
    // ... similar test structure
}

#[tokio::test]
async fn test_sagitta_code_migration_compatibility() {
    // Test that git-manager provides same functionality as old sagitta-code
    // ... similar test structure
}
```

### Backward Compatibility Tests

Ensure that existing configurations and workflows continue to work:

```rust
#[tokio::test]
async fn test_existing_config_compatibility() {
    // Load existing sagitta configuration
    // Verify git-manager can work with existing repository configs
    // Test that no data is lost during migration
}
```

## 6. Rollback Strategy

### Git Branches for Each Migration

1. Create feature branches for each tool migration:
   - `feature/migrate-cli-to-git-manager`
   - `feature/migrate-mcp-to-git-manager`
   - `feature/migrate-sagitta-code-to-git-manager`

2. Incremental commits for easy rollback:
   ```bash
   git commit -m "cli: Add git-manager dependency"
   git commit -m "cli: Update use_branch.rs to use git-manager"
   git commit -m "cli: Add enhanced sync detection"
   git commit -m "cli: Add new branch management commands"
   ```

### Configuration Backup

Before migration, backup existing configurations:

```rust
// Add to migration scripts
fn backup_configuration() -> Result<PathBuf> {
    let config_path = get_config_path_or_default(None)?;
    let backup_path = config_path.with_extension("toml.backup");
    std::fs::copy(&config_path, &backup_path)?;
    Ok(backup_path)
}
```

## 7. Migration Checklist

### Pre-Migration
- [ ] All git-manager tests passing (33/33)
- [ ] Performance benchmarks established
- [ ] Configuration backup created
- [ ] Migration branches created

### CLI Migration
- [ ] Add git-manager dependency
- [ ] Update use_branch.rs
- [ ] Update sync.rs with enhanced detection
- [ ] Add new branch management commands
- [ ] Update tests
- [ ] Performance validation

### MCP Migration  
- [ ] Add git-manager dependency
- [ ] Add new MCP endpoints
- [ ] Update repository handlers
- [ ] Update server request handling
- [ ] Update tool definitions
- [ ] Test MCP protocol compatibility

### Sagitta-Code Migration
- [ ] Add git-manager dependency
- [ ] Update RepositoryManager
- [ ] Update sync operations
- [ ] Add branch switching UI
- [ ] Update repository tools
- [ ] Test GUI functionality

### Post-Migration
- [ ] All integration tests passing
- [ ] Performance meets or exceeds baseline
- [ ] No data loss verified
- [ ] Documentation updated
- [ ] Old code cleanup

## 8. Success Metrics

### Functionality
- [ ] All existing git operations work
- [ ] New branch management features available
- [ ] Automatic resync working correctly
- [ ] Merkle tree optimization active

### Performance
- [ ] Branch switching ≤ current performance
- [ ] Memory usage ≤ current usage
- [ ] Sync detection < 100ms for typical repos

### Quality
- [ ] Test coverage ≥ 90%
- [ ] No critical bugs
- [ ] Clean API design
- [ ] Comprehensive documentation

This migration guide provides a comprehensive roadmap for transitioning all sagitta tools to use the new git-manager crate while maintaining backward compatibility and improving functionality. 