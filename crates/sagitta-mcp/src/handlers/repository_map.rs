use crate::mcp::{
    error_codes,
    types::{ErrorObject, RepositoryMapParams, RepositoryMapResult, RepositoryMapSummary},
};
use anyhow::Result;
use repo_mapper::{generate_repo_map, RepoMapOptions};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, instrument};
use sagitta_search::config::AppConfig;

/// Handles repository mapping requests using the new repo-mapper crate
#[instrument(skip(config), fields(repo_name = %params.repository_name))]
pub async fn handle_repository_map(
    params: RepositoryMapParams,
    config: Arc<RwLock<AppConfig>>,
) -> Result<RepositoryMapResult, ErrorObject> {
    debug!("Starting repository mapping for: {}", params.repository_name);

    let config_guard = config.read().await;
    let repo_config = config_guard
        .repositories
        .iter()
        .find(|r| r.name == params.repository_name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::INVALID_PARAMS,
            message: format!("Repository '{}' not found", params.repository_name),
            data: None,
        })?;

    let repo_path = repo_config.local_path.clone();
    drop(config_guard);

    // Convert MCP params to repo-mapper options
    let options = RepoMapOptions {
        verbosity: params.verbosity,
        file_extension: params.file_extension,
        content_pattern: None, // Could be added to MCP params in future
        paths: params.paths,
        max_calls_per_method: 10, // Default, could be configurable
        include_context: true, // Always include context for proper decorator detection
        include_docstrings: params.verbosity >= 1,
        ..Default::default()
    };

    // Use the new repo-mapper crate
    let result = generate_repo_map(&repo_path, options)
        .map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to generate repository map: {e}"),
            data: None,
        })?;

    // Convert repo-mapper result to MCP format
    let summary = RepositoryMapSummary {
        files_scanned: result.summary.files_scanned,
        elements_found: result.summary.total_methods,
        file_types: convert_file_types(&result.summary.file_type_counts),
        element_types: convert_element_types(&result.summary.method_type_counts),
    };

    Ok(RepositoryMapResult {
        map_content: result.map_content,
        summary,
    })
}

/// Convert repo-mapper file type counts to MCP format
fn convert_file_types(file_type_counts: &HashMap<String, usize>) -> HashMap<String, usize> {
    let mut converted = HashMap::new();
    
    for (lang, count) in file_type_counts {
        let extension = match lang.as_str() {
            "Rust" => "rs",
            "JavaScript" => "js", 
            "TypeScript" => "ts",
            "Python" => "py",
            "Go" => "go",
            "Ruby" => "rb",
            "Vue" => "vue",
            "YAML" => "yaml",
            "Markdown" => "md",
            _ => lang.as_str(),
        };
        converted.insert(extension.to_string(), *count);
    }
    
    converted
}

/// Convert repo-mapper method type counts to MCP format
fn convert_element_types(method_type_counts: &HashMap<String, usize>) -> HashMap<String, usize> {
    let mut converted = HashMap::new();
    
    for (method_type, count) in method_type_counts {
        // Map detailed method types to simpler element types for MCP compatibility
        let element_type = match method_type.as_str() {
            "Rust Function" | "Python Function" | "Python Async Function" | 
            "JavaScript Function" | "TypeScript Function" | "Go Function" => "function",
            
            "Rust Implementation" => "impl",
            "Rust Trait" | "Rust Trait Method" => "trait",
            
            "Python Class" | "JavaScript Class" | "TypeScript Class" => "class",
            "Python Method" | "Python Static Method" | "Python Class Method" |
            "JavaScript Object Method" | "TypeScript Method" | "Go Method" |
            "Ruby Instance Method" | "Ruby Class Method" => "method",
            
            "TypeScript Interface" | "Go Interface" => "interface",
            "TypeScript Type" => "type",
            
            "Vue Method" | "Vue Computed Property" => "vue_method",
            "Vue Component" => "vue_component",
            "Vue Prop" => "vue_prop",
            
            "Ruby Module" => "module",
            
            "YAML Definition" | "YAML Value" | "YAML Template" => "yaml_element",
            "Markdown Header" => "header",
            
            _ => "other",
        };
        
        *converted.entry(element_type.to_string()).or_insert(0) += count;
    }
    
    converted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use sagitta_search::config::{AppConfig, RepositoryConfig};

    fn create_test_config(repo_configs: Vec<RepositoryConfig>) -> Arc<RwLock<AppConfig>> {
        let app_config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            repositories: repo_configs,
            ..Default::default()
        };
        Arc::new(RwLock::new(app_config))
    }

    fn create_test_repo_config(name: &str, repo_path: std::path::PathBuf) -> RepositoryConfig {
        RepositoryConfig {
            name: name.to_string(),
            url: "file:///tmp/test".to_string(),
            local_path: repo_path,
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            active_branch: Some("main".to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_repository_map_with_new_crate() {
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        fs::create_dir_all(&repo_path).unwrap();

        // Create a simple Rust file
        let rust_file = repo_path.join("lib.rs");
        fs::write(&rust_file, r#"
/// A test function
pub fn test_function(param: String) -> String {
    println!("Hello, world!");
    param
}

/// A test struct
pub struct TestStruct {
    field: String,
}

impl TestStruct {
    /// Creates a new instance
    pub fn new(field: String) -> Self {
        Self { field }
    }
}
"#).unwrap();

        let repo_config = create_test_repo_config("test_repo", repo_path);
        let config = create_test_config(vec![repo_config]);

        let params = RepositoryMapParams {
            repository_name: "test_repo".to_string(),
            verbosity: 2,
            paths: None,
            file_extension: Some("rs".to_string()),
        };

        let result = handle_repository_map(params, config).await;
        assert!(result.is_ok());

        let map_result = result.unwrap();
        let content = &map_result.map_content;
        
        // Check that we get proper names (no "unnamed" objects)
        assert!(content.contains("test_function"));
        assert!(content.contains("TestStruct"));
        assert!(content.contains("new"));
        
        // Check for proper icons from repo-mapper
        assert!(content.contains("⚙️")); // Rust function
        assert!(content.contains("🔨")); // Rust impl
        
        // Check summary
        assert!(map_result.summary.files_scanned > 0);
        assert!(map_result.summary.elements_found > 0);
        assert!(map_result.summary.file_types.contains_key("rs"));
        assert!(map_result.summary.element_types.contains_key("function"));
    }

    #[tokio::test]
    async fn test_repository_map_python() {
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path().join("python_repo");
        fs::create_dir_all(&repo_path).unwrap();

        let python_file = repo_path.join("main.py");
        fs::write(&python_file, r#"
"""Test Python module"""

class DatabaseConfig:
    """Configuration for database connections."""
    
    def __init__(self, host: str, port: int):
        """Initialize the config."""
        self.host = host
        self.port = port
    
    @staticmethod
    def get_default_port():
        """Get the default port."""
        return 5432
    
    @classmethod
    def from_url(cls, url: str):
        """Create config from URL."""
        return cls("localhost", 5432)

async def connect_database(config):
    """Connect to the database asynchronously."""
    print(f"Connecting to {config.host}:{config.port}")
    return True

def process_data(data):
    """Process some data."""
    result = []
    for item in data:
        result.append(item.upper())
    return result
"#).unwrap();

        let repo_config = create_test_repo_config("python_repo", repo_path);
        let config = create_test_config(vec![repo_config]);

        let params = RepositoryMapParams {
            repository_name: "python_repo".to_string(),
            verbosity: 1,
            paths: None,
            file_extension: Some("py".to_string()),
        };

        let result = handle_repository_map(params, config).await;
        assert!(result.is_ok());

        let map_result = result.unwrap();
        let content = &map_result.map_content;
        

        // Check for Python elements with proper names
        assert!(content.contains("DatabaseConfig"));
        assert!(content.contains("__init__"));
        assert!(content.contains("get_default_port"));
        assert!(content.contains("from_url"));
        assert!(content.contains("async connect_database"));
        assert!(content.contains("process_data"));
        
        // Check for Python icons
        assert!(content.contains("🏛️")); // Python class
        assert!(content.contains("🔧")); // Python method
        assert!(content.contains("📌")); // Python static method
        assert!(content.contains("🏷️")); // Python class method
        assert!(content.contains("🔄")); // Python async function
        assert!(content.contains("🐍")); // Python function
        
        assert!(map_result.summary.files_scanned > 0);
        assert!(map_result.summary.elements_found > 0);
        assert!(map_result.summary.file_types.contains_key("py"));
    }

    #[tokio::test]
    async fn test_repository_map_verbosity_levels() {
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path().join("verbosity_repo");
        fs::create_dir_all(&repo_path).unwrap();

        let rust_file = repo_path.join("test.rs");
        fs::write(&rust_file, r#"
/// A documented function
pub fn documented_function() {
    println!("Hello");
    some_call();
}
"#).unwrap();

        let repo_config = create_test_repo_config("verbosity_repo", repo_path);
        let config = create_test_config(vec![repo_config]);

        // Test verbosity 0 (minimal)
        let params_0 = RepositoryMapParams {
            repository_name: "verbosity_repo".to_string(),
            verbosity: 0,
            paths: None,
            file_extension: Some("rs".to_string()),
        };

        let result_0 = handle_repository_map(params_0, config.clone()).await.unwrap();
        let content_0 = &result_0.map_content;
        
        // Minimal should not contain calls or docs
        assert!(!content_0.contains("📞 Calls:"));
        assert!(!content_0.contains("📝"));
        assert!(content_0.contains("documented_function"));

        // Test verbosity 2 (detailed)
        let params_2 = RepositoryMapParams {
            repository_name: "verbosity_repo".to_string(),
            verbosity: 2,
            paths: None,
            file_extension: Some("rs".to_string()),
        };

        let result_2 = handle_repository_map(params_2, config).await.unwrap();
        let content_2 = &result_2.map_content;
        
        // Detailed should contain both calls and docs
        assert!(content_2.contains("📞 Calls:"));
        assert!(content_2.contains("📝"));
        assert!(content_2.contains("A documented function"));
    }
} 