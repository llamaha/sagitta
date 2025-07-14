use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use dirs;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub repositories: Vec<RepositoryConfig>,
    #[serde(default)]
    pub todo_items: Vec<TodoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryConfig {
    pub name: String,
    pub url: String,
    pub local_path: String,
    pub default_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_branch: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<RepositoryDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryDependency {
    pub repository_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

pub fn get_config_path_or_default() -> PathBuf {
    if let Ok(custom_path) = std::env::var("SAGITTA_CONFIG") {
        PathBuf::from(custom_path)
    } else {
        let home = dirs::home_dir().expect("Failed to get home directory");
        home.join(".config").join("sagitta").join("config.toml")
    }
}

pub fn load_config(path: Option<&Path>) -> Result<AppConfig> {
    let config_path = path.map(|p| p.to_path_buf()).unwrap_or_else(get_config_path_or_default);
    
    if !config_path.exists() {
        return Ok(AppConfig::default());
    }
    
    let content = fs::read_to_string(&config_path)?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &AppConfig, path: Option<&Path>) -> Result<()> {
    let config_path = path.map(|p| p.to_path_buf()).unwrap_or_else(get_config_path_or_default);
    
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let content = toml::to_string_pretty(config)?;
    fs::write(&config_path, content)?;
    Ok(())
}

pub fn get_repo_base_path(_config: Option<&AppConfig>) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
    Ok(home.join(".sagitta").join("repos"))
}