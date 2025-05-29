//! Repository mapping functionality for generating code structure maps
//! 
//! This crate provides functionality to scan repositories and generate structured maps
//! of code elements like functions, classes, structs, etc. It's designed to be used
//! by both the MCP server and sagitta-code for consistent repository mapping.

pub mod error;
pub mod mapper;
pub mod scanners;
pub mod types;

pub use error::RepoMapperError;
pub use mapper::RepoMapper;
pub use types::{
    MethodInfo, MethodType, RepoMapOptions, RepoMapResult, RepoMapSummary,
};

/// Main entry point for generating a repository map
/// 
/// # Arguments
/// 
/// * `repo_path` - Path to the repository root
/// * `options` - Configuration options for the mapping
/// 
/// # Returns
/// 
/// A `RepoMapResult` containing the formatted map and summary statistics
/// 
/// # Example
/// 
/// ```rust
/// use repo_mapper::{generate_repo_map, RepoMapOptions};
/// use std::path::Path;
/// 
/// let options = RepoMapOptions::default();
/// let result = generate_repo_map(Path::new("."), options).unwrap();
/// println!("{}", result.map_content);
/// ```
pub fn generate_repo_map(
    repo_path: &std::path::Path,
    options: RepoMapOptions,
) -> Result<RepoMapResult, RepoMapperError> {
    let mut mapper = RepoMapper::new(options);
    mapper.scan_repository(repo_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_basic_rust_scanning() {
        let temp_dir = tempdir().unwrap();
        let rust_file = temp_dir.path().join("test.rs");
        
        fs::write(&rust_file, r#"
pub fn test_function(param: String) -> String {
    println!("Hello, world!");
    param
}

pub struct TestStruct {
    field: String,
}

impl TestStruct {
    pub fn new(field: String) -> Self {
        Self { field }
    }
}
"#).unwrap();

        let options = RepoMapOptions {
            file_extension: Some("rs".to_string()),
            ..Default::default()
        };

        let result = generate_repo_map(temp_dir.path(), options).unwrap();
        
        assert!(result.map_content.contains("test_function"));
        assert!(result.map_content.contains("TestStruct"));
        assert!(result.map_content.contains("new"));
        assert!(result.summary.total_methods > 0);
        assert!(result.summary.languages_found.contains(&"Rust".to_string()));
    }

    #[test]
    fn test_python_scanning() {
        let temp_dir = tempdir().unwrap();
        let python_file = temp_dir.path().join("test.py");
        
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

        let options = RepoMapOptions {
            file_extension: Some("py".to_string()),
            ..Default::default()
        };

        let result = generate_repo_map(temp_dir.path(), options).unwrap();
        

        assert!(result.map_content.contains("DatabaseConfig"));
        assert!(result.map_content.contains("__init__"));
        assert!(result.map_content.contains("get_default_port"));
        assert!(result.map_content.contains("from_url"));
        assert!(result.map_content.contains("async connect_database"));
        assert!(result.map_content.contains("process_data"));
        assert!(result.summary.total_methods > 0);
        assert!(result.summary.languages_found.contains(&"Python".to_string()));
    }
} 