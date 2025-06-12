// Add repository tool will go here

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use std::path::Path;

use crate::gui::repository::manager::RepositoryManager;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;

/// Parameters for adding a repository
#[derive(Debug, Deserialize, Serialize)]
pub struct AddExistingRepositoryParams {
    /// Name for the repository
    pub name: String,
    /// Git URL for the repository
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Branch to use (optional, defaults to main/master)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Local path to an existing repository (alternative to URL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
}

/// Tool for adding existing repositories to the management system
#[derive(Debug)]
pub struct AddExistingRepositoryTool {
    repo_manager: Arc<Mutex<RepositoryManager>>,
}

impl AddExistingRepositoryTool {
    /// Create a new add existing repository tool
    pub fn new(repo_manager: Arc<Mutex<RepositoryManager>>) -> Self {
        Self {
            repo_manager,
        }
    }
    
    /// Validate parameters with comprehensive error messages
    fn validate_parameters(&self, params: &AddExistingRepositoryParams) -> Result<(), SagittaCodeError> {
        // Validate name
        if params.name.trim().is_empty() {
            return Err(SagittaCodeError::ToolError(
                "Repository name cannot be empty. Please provide a meaningful name for the repository.".to_string()
            ));
        }

        if params.name.len() > 100 {
            return Err(SagittaCodeError::ToolError(
                "Repository name is too long (max 100 characters). Please use a shorter name.".to_string()
            ));
        }

        // Check for invalid characters in name
        if params.name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
            return Err(SagittaCodeError::ToolError(
                "Repository name contains invalid characters. Please use only alphanumeric characters, hyphens, and underscores.".to_string()
            ));
        }

        // Validate that either URL or local_path is provided
        match (&params.url, &params.local_path) {
            (None, None) => {
                return Err(SagittaCodeError::ToolError(
                    "Either 'url' or 'local_path' must be provided. \n\n\
                    Use 'url' for remote Git repositories (e.g., 'https://github.com/user/repo.git')\n\
                    Use 'local_path' for existing local directories (e.g., '/home/user/projects/myproject')\n\n\
                    Example with URL:\n\
                    {\n  \"name\": \"my-repo\",\n  \"url\": \"https://github.com/user/repo.git\"\n}\n\n\
                    Example with local path:\n\
                    {\n  \"name\": \"my-local-repo\",\n  \"local_path\": \"/path/to/existing/directory\"\n}".to_string()
                ));
            }
            (Some(_), Some(_)) => {
                return Err(SagittaCodeError::ToolError(
                    "Cannot specify both 'url' and 'local_path'. Please choose one:\n\n\
                    - Use 'url' for cloning a remote Git repository\n\
                    - Use 'local_path' for registering an existing local directory".to_string()
                ));
            }
            _ => {} // Valid: exactly one is provided
        }

        // Validate URL format if provided
        if let Some(url) = &params.url {
            if url.trim().is_empty() {
                return Err(SagittaCodeError::ToolError(
                    "URL cannot be empty. Please provide a valid Git repository URL (HTTPS or SSH format).".to_string()
                ));
            }

            // Basic URL validation
            if !url.contains("://") && !url.starts_with("git@") {
                return Err(SagittaCodeError::ToolError(
                    format!("Invalid URL format: '{}'\n\n\
                    Please use one of these formats:\n\
                    - HTTPS: https://github.com/user/repo.git\n\
                    - SSH: git@github.com:user/repo.git\n\
                    - GitLab HTTPS: https://gitlab.com/user/repo.git", url)
                ));
            }

            // Validate branch parameter when URL is provided
            if let Some(branch) = &params.branch {
                if branch.trim().is_empty() {
                    return Err(SagittaCodeError::ToolError(
                        "Branch name cannot be empty. Either omit the 'branch' parameter to use the default branch, or provide a valid branch name.".to_string()
                    ));
                }

                // Check for invalid branch characters
                if branch.contains("..") || 
                   branch.contains([' ', '\t', '\n', '\r', '^', '~', ':', '?', '*', '[', '\\']) {
                    return Err(SagittaCodeError::ToolError(
                        format!("Invalid branch name: '{}'. Branch names cannot contain spaces, special characters like '..', '^', '~', ':', etc.", branch)
                    ));
                }
            }
        }

        // Validate local_path if provided
        if let Some(local_path) = &params.local_path {
            if local_path.trim().is_empty() {
                return Err(SagittaCodeError::ToolError(
                    "Local path cannot be empty. Please provide the absolute path to an existing directory.".to_string()
                ));
            }

            // Check if path is absolute
            let path = Path::new(local_path);
            if !path.is_absolute() {
                return Err(SagittaCodeError::ToolError(
                    format!("Local path must be absolute: '{}'\n\n\
                    Please provide the full path starting from the root directory (e.g., '/home/user/projects/myproject' on Linux/Mac or 'C:\\Users\\user\\projects\\myproject' on Windows).", local_path)
                ));
            }

            // Check if path exists
            if !path.exists() {
                return Err(SagittaCodeError::ToolError(
                    format!("Local path does not exist: '{}'\n\n\
                    Please ensure the directory exists before registering it. You can:\n\
                    1. Create the directory first using shell commands\n\
                    2. Check the path for typos\n\
                    3. Verify you have read permissions to the directory", local_path)
                ));
            }

            // Check if it's actually a directory
            if !path.is_dir() {
                return Err(SagittaCodeError::ToolError(
                    format!("Local path is not a directory: '{}'\n\n\
                    Please provide a path to a directory, not a file.", local_path)
                ));
            }

            // Warn about branch parameter with local path
            if params.branch.is_some() {
                return Err(SagittaCodeError::ToolError(
                    "The 'branch' parameter cannot be used with 'local_path'. Branch selection is only for remote repositories specified with 'url'.".to_string()
                ));
            }
        }

        Ok(())
    }
    
    /// Add an existing repository
    async fn add_existing_repository(&self, params: &AddExistingRepositoryParams) -> Result<String, SagittaCodeError> {
        // Validate parameters first
        self.validate_parameters(params)?;

        let mut repo_manager = self.repo_manager.lock().await;
        
        if let Some(local_path) = &params.local_path {
            // Add local repository
            match repo_manager.add_local_repository(&params.name, local_path).await {
                Ok(_) => Ok(format!("Successfully added local repository '{}' from path: {}", params.name, local_path)),
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("already exists in configuration") {
                        // Treat "already exists" as success since the repository is available
                        Ok(format!("Repository '{}' already exists and is available for use", params.name))
                    } else {
                        // Enhanced error message with suggestions
                        Err(SagittaCodeError::ToolError(format!(
                            "Failed to add local repository '{}' from path '{}': {}\n\n\
                            Possible solutions:\n\
                            1. Check if you have read permissions to the directory\n\
                            2. Verify the path exists and contains code files\n\
                            3. Try using a different repository name\n\
                            4. Check if the directory is already registered under a different name", 
                            params.name, local_path, e
                        )))
                    }
                }
            }
        } else if let Some(url) = &params.url {
            // Add remote repository
            let branch = params.branch.as_deref();
            match repo_manager.add_repository(&params.name, url, branch).await {
                Ok(_) => {
            let branch_msg = if let Some(b) = branch {
                format!(" (branch: {})", b)
            } else {
                String::new()
            };
            Ok(format!("Successfully added repository '{}' from URL: {}{}", params.name, url, branch_msg))
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("already exists in configuration") {
                        // Treat "already exists" as success since the repository is available
                        Ok(format!("Repository '{}' already exists and is available for use", params.name))
                    } else {
                        // Enhanced error message with suggestions
                        Err(SagittaCodeError::ToolError(format!(
                            "Failed to add repository '{}' from URL '{}': {}\n\n\
                            Possible solutions:\n\
                            1. Check if the URL is correct and accessible\n\
                            2. Verify you have network connectivity\n\
                            3. For private repositories, ensure you have proper authentication\n\
                            4. Try using a different branch name if specified\n\
                            5. Check if the repository name is already in use", 
                            params.name, url, e
                        )))
                    }
                }
            }
        } else {
            // This should never happen due to validation, but include for safety
            Err(SagittaCodeError::ToolError("Either URL or local_path must be provided".to_string()))
        }
    }
}

#[async_trait]
impl Tool for AddExistingRepositoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "add_existing_repository".to_string(),
            description: "Register an ALREADY EXISTING repository with Sagitta for code analysis and search. This tool is ONLY for repositories that already exist - either local directories with code or remote Git repositories. DO NOT use this tool to create new projects. For creating new projects, use shell commands like 'cargo new', 'npm init', 'go mod init', etc. You must provide either 'url' (for remote Git repos) OR 'local_path' (for existing local directories).".to_string(),
            category: ToolCategory::Repository,
            is_required: false,
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name"],
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Unique name for the repository. Must be meaningful and not empty. Cannot contain special characters like /, \\, :, *, ?, \", <, >, |",
                        "examples": ["my-project", "awesome-library", "company-api"]
                    },
                    "url": {
                        "type": ["string", "null"],
                        "description": "Git URL for remote repositories (e.g., 'https://github.com/user/repo.git' or 'git@github.com:user/repo.git'). Provide either 'url' OR 'local_path', not both.",
                        "examples": [
                            "https://github.com/tokio-rs/tokio.git",
                            "git@github.com:rust-lang/rust.git",
                            "https://gitlab.com/user/project.git"
                        ]
                    },
                    "branch": {
                        "type": ["string", "null"],
                        "description": "Git branch to checkout (e.g., 'main', 'master', 'develop', 'v1.0'). Only used with 'url'. Defaults to repository's default branch if not specified.",
                        "examples": ["main", "master", "develop", "feature/new-api", "v1.0.0"]
                    },
                    "local_path": {
                        "type": ["string", "null"],
                        "description": "Absolute path to an existing local directory containing code (e.g., '/home/user/projects/myproject'). Use this for repositories that already exist on the local filesystem. Provide either 'local_path' OR 'url', not both.",
                        "examples": [
                            "/home/user/projects/myproject",
                            "/Users/user/code/project",
                            "C:\\Users\\user\\projects\\myproject"
                        ]
                    }
                },
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["local_path"] }
                ]
            }),
            metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        let params: AddExistingRepositoryParams = match serde_json::from_value(parameters.clone()) {
            Ok(params) => params,
            Err(e) => {
                let detailed_error = format!(
                    "Invalid parameters: {}\n\n\
                    Expected format:\n\
                    For remote repository:\n\
                    {{\n  \"name\": \"my-repo\",\n  \"url\": \"https://github.com/user/repo.git\",\n  \"branch\": \"main\" // optional\n}}\n\n\
                    For local directory:\n\
                    {{\n  \"name\": \"my-local-repo\",\n  \"local_path\": \"/absolute/path/to/directory\"\n}}\n\n\
                    Received parameters: {}", 
                    e, 
                    serde_json::to_string_pretty(&parameters).unwrap_or_else(|_| "Invalid JSON".to_string())
                );
                return Ok(ToolResult::Error { error: detailed_error });
            }
        };
        
        match self.add_existing_repository(&params).await {
            Ok(message) => Ok(ToolResult::Success(serde_json::json!({
                "repository_name": params.name,
                "repository_url": params.url,
                "local_path": params.local_path,
                "branch": params.branch,
                "status": "added",
                "message": message
            }))),
            Err(SagittaCodeError::ToolError(msg)) => Ok(ToolResult::Error { error: msg }),
            Err(e) => Err(e) // Only propagate non-tool errors
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::manager::RepositoryManager;
    use sagitta_search::AppConfig as SagittaAppConfig;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use serde_json::json;

    fn create_test_repo_manager() -> Arc<Mutex<RepositoryManager>> {
        let config = SagittaAppConfig::default();
        let repo_manager = RepositoryManager::new_for_test(Arc::new(Mutex::new(config)));
        Arc::new(Mutex::new(repo_manager))
    }

    #[tokio::test]
    async fn test_add_existing_repository_tool_creation() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        assert!(format!("{:?}", tool).contains("AddExistingRepositoryTool"));
    }

    #[tokio::test]
    async fn test_tool_definition() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        let definition = tool.definition();
        
        assert_eq!(definition.name, "add_existing_repository");
        assert!(definition.description.contains("ALREADY EXISTING repository"));
        assert!(definition.description.contains("DO NOT use this tool to create new projects"));
        assert_eq!(definition.category, ToolCategory::Repository);
        assert!(!definition.is_required);
        
        // Check parameters structure
        let params = definition.parameters;
        assert!(params.get("type").is_some());
        assert!(params.get("properties").is_some());
        assert!(params.get("required").is_some());
    }

    #[tokio::test]
    async fn test_execute_with_invalid_parameters() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test with empty parameters
        let result = tool.execute(json!({})).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Invalid parameters"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_execute_with_missing_url_and_path() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo"
        });
        
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Either 'url' or 'local_path' must be provided"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_execute_with_valid_local_path_params() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo",
            "local_path": "/tmp/test-repo"
        });
        
        // This will fail because the repository manager isn't fully initialized
        // but we can test that the parameters are parsed correctly
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                // Should fail at the repository manager level, not parameter parsing
                assert!(!error.contains("Invalid parameters"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_execute_with_valid_url_params() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "test-repo",
            "url": "https://github.com/test/repo.git",
            "branch": "main"
        });
        
        // This will fail because the repository manager isn't fully initialized
        // but we can test that the parameters are parsed correctly
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                // Should fail at the repository manager level, not parameter parsing
                assert!(!error.contains("Invalid parameters"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_add_existing_repository_params_serialization() {
        let params = AddExistingRepositoryParams {
            name: "test-repo".to_string(),
            url: Some("https://github.com/test/repo.git".to_string()),
            branch: Some("main".to_string()),
            local_path: None,
        };
        
        let serialized = serde_json::to_string(&params).unwrap();
        let deserialized: AddExistingRepositoryParams = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(params.name, deserialized.name);
        assert_eq!(params.url, deserialized.url);
        assert_eq!(params.branch, deserialized.branch);
        assert_eq!(params.local_path, deserialized.local_path);
    }

    #[tokio::test]
    async fn test_add_existing_repository_params_with_local_path() {
        let params = AddExistingRepositoryParams {
            name: "local-repo".to_string(),
            url: None,
            branch: None,
            local_path: Some("/path/to/repo".to_string()),
        };
        
        let json_value = serde_json::to_value(&params).unwrap();
        let parsed: AddExistingRepositoryParams = serde_json::from_value(json_value).unwrap();
        
        assert_eq!(params.name, parsed.name);
        assert_eq!(params.local_path, parsed.local_path);
        assert!(parsed.url.is_none());
        assert!(parsed.branch.is_none());
    }

    #[tokio::test]
    async fn test_parameter_validation_edge_cases() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test with both url and local_path provided
        let params = json!({
            "name": "test-repo",
            "url": "https://github.com/test/repo.git",
            "local_path": "/tmp/repo"
        });
        
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Cannot specify both 'url' and 'local_path'"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_empty_name_parameter() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        let params = json!({
            "name": "",
            "url": "https://github.com/test/repo.git"
        });
        
        let result = tool.execute(params).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Repository name cannot be empty"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_name_validation() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test empty name
        let params = AddExistingRepositoryParams {
            name: "".to_string(),
            url: Some("https://github.com/test/repo.git".to_string()),
            branch: None,
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository name cannot be empty"));

        // Test name with whitespace only
        let params = AddExistingRepositoryParams {
            name: "   ".to_string(),
            url: Some("https://github.com/test/repo.git".to_string()),
            branch: None,
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository name cannot be empty"));

        // Test name too long
        let params = AddExistingRepositoryParams {
            name: "a".repeat(101),
            url: Some("https://github.com/test/repo.git".to_string()),
            branch: None,
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Repository name is too long"));

        // Test name with invalid characters
        let invalid_chars = vec!['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
        for invalid_char in invalid_chars {
            let params = AddExistingRepositoryParams {
                name: format!("repo{}", invalid_char),
                url: Some("https://github.com/test/repo.git".to_string()),
                branch: None,
                local_path: None,
            };
            let result = tool.validate_parameters(&params);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Repository name contains invalid characters"));
        }

        // Test valid names
        let valid_names = vec!["repo", "my-repo", "my_repo", "repo123", "a-b_c-123"];
        for valid_name in valid_names {
            let params = AddExistingRepositoryParams {
                name: valid_name.to_string(),
                url: Some("https://github.com/test/repo.git".to_string()),
                branch: None,
                local_path: None,
            };
            let result = tool.validate_parameters(&params);
            assert!(result.is_ok(), "Valid name '{}' should pass validation", valid_name);
        }
    }

    #[tokio::test]
    async fn test_url_validation() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test empty URL
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: Some("".to_string()),
            branch: None,
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("URL cannot be empty"));

        // Test URL with only whitespace
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: Some("   ".to_string()),
            branch: None,
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("URL cannot be empty"));

        // Test invalid URL format
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: Some("invalid-url".to_string()),
            branch: None,
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid URL format"));

        // Test valid URLs
        let valid_urls = vec![
            "https://github.com/user/repo.git",
            "git@github.com:user/repo.git",
            "https://gitlab.com/user/repo.git",
            "ssh://git@server.com/repo.git",
        ];
        for valid_url in valid_urls {
            let params = AddExistingRepositoryParams {
                name: "repo".to_string(),
                url: Some(valid_url.to_string()),
                branch: None,
                local_path: None,
            };
            let result = tool.validate_parameters(&params);
            assert!(result.is_ok(), "Valid URL '{}' should pass validation", valid_url);
        }
    }

    #[tokio::test]
    async fn test_branch_validation() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test empty branch name
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: Some("https://github.com/test/repo.git".to_string()),
            branch: Some("".to_string()),
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Branch name cannot be empty"));

        // Test branch name with invalid characters
        let invalid_branches = vec!["branch with spaces", "branch..name", "branch^name", "branch~name", "branch:name", "branch?name", "branch*name", "branch[name", "branch\\name"];
        for invalid_branch in invalid_branches {
            let params = AddExistingRepositoryParams {
                name: "repo".to_string(),
                url: Some("https://github.com/test/repo.git".to_string()),
                branch: Some(invalid_branch.to_string()),
                local_path: None,
            };
            let result = tool.validate_parameters(&params);
            assert!(result.is_err(), "Invalid branch '{}' should fail validation", invalid_branch);
            assert!(result.unwrap_err().to_string().contains("Invalid branch name"));
        }

        // Test valid branch names
        let valid_branches = vec!["main", "develop", "feature-123", "feature_name", "v1.0.0"];
        for valid_branch in valid_branches {
            let params = AddExistingRepositoryParams {
                name: "repo".to_string(),
                url: Some("https://github.com/test/repo.git".to_string()),
                branch: Some(valid_branch.to_string()),
                local_path: None,
            };
            let result = tool.validate_parameters(&params);
            assert!(result.is_ok(), "Valid branch '{}' should pass validation", valid_branch);
        }

        // Test branch with local_path (should fail)
        // First create a temporary directory for the test
        std::fs::create_dir_all("/tmp/test_repo").ok();
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: None,
            branch: Some("main".to_string()),
            local_path: Some("/tmp/test_repo".to_string()),
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("The 'branch' parameter cannot be used with 'local_path'"));
        // Clean up
        std::fs::remove_dir_all("/tmp/test_repo").ok();
    }

    #[tokio::test]
    async fn test_local_path_validation() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test empty local path
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: None,
            branch: None,
            local_path: Some("".to_string()),
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Local path cannot be empty"));

        // Test relative path (should fail)
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: None,
            branch: None,
            local_path: Some("relative/path".to_string()),
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Local path must be absolute"));

        // Test non-existent path (should fail)
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: None,
            branch: None,
            local_path: Some("/this/path/does/not/exist".to_string()),
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Local path does not exist"));

        // Test path pointing to file instead of directory
        // We'll use a common file that should exist on most systems
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: None,
            branch: None,
            local_path: Some("/etc/passwd".to_string()),
        };
        let result = tool.validate_parameters(&params);
        // This test might not work on all systems, so we'll check if the file exists first
        if std::path::Path::new("/etc/passwd").exists() {
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Local path is not a directory"));
        }
    }

    #[tokio::test]
    async fn test_mutual_exclusion_validation() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test neither URL nor local_path provided
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: None,
            branch: None,
            local_path: None,
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Either 'url' or 'local_path' must be provided"));

        // Test both URL and local_path provided
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: Some("https://github.com/test/repo.git".to_string()),
            branch: None,
            local_path: Some("/tmp/repo".to_string()),
        };
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot specify both 'url' and 'local_path'"));
    }

    #[tokio::test]
    async fn test_enhanced_error_messages_in_execute() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test detailed error message for invalid JSON
        let invalid_json = json!({
            "name": 123, // Should be string
            "url": "https://github.com/test/repo.git"
        });
        
        let result = tool.execute(invalid_json).await.unwrap();
        match result {
            ToolResult::Error { error } => {
                assert!(error.contains("Invalid parameters"));
                assert!(error.contains("Expected format"));
                assert!(error.contains("Received parameters"));
            }
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_validation_examples_in_error_messages() {
        let repo_manager = create_test_repo_manager();
        let tool = AddExistingRepositoryTool::new(repo_manager);
        
        // Test that error message includes examples
        let params = AddExistingRepositoryParams {
            name: "repo".to_string(),
            url: None,
            branch: None,
            local_path: None,
        };
        
        let result = tool.validate_parameters(&params);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Example with URL"));
        assert!(error_msg.contains("Example with local path"));
        assert!(error_msg.contains("\"name\": \"my-repo\""));
    }
}

