use crate::chain::action::Action;
use crate::tools::command::{RunCommandAction, RunCommandParams};
use crate::tools::file::{CreateDirectoryAction, CreateDirectoryParams, ReadFileAction, ReadFileParams, WriteFileAction, WriteFileParams, LineEditAction, LineEditParams};
use crate::tools::git::{GitStatusAction, GitStatusParams, GitAddAction, GitAddParams, GitCommitAction, GitCommitParams};
use crate::tools::repo::actions::{AddRepoAction, AddRepoParams, InitRepoAction, InitRepoParams, UseRepoAction, UseRepoParams, ListRepositoriesAction, ListRepositoriesParams, RemoveRepositoryAction, RemoveRepositoryParams, SyncRepositoryAction, SyncRepositoryParams};
use crate::tools::search::{SemanticSearchAction, SemanticSearchParams};
use crate::tools::edit::{SemanticEditAction, SemanticEditParams};
use crate::utils::error::{RelayError, Result};
use serde::Deserialize;
use serde_json::Value;
use tracing::error;
use tracing::warn;

/// Represents the expected structure of an action request from the LLM.
#[derive(Deserialize, Debug)]
struct ActionRequest {
    action: String,
    params: Value, // Params are kept as raw JSON Value initially
}

/// Parses a JSON string representing an action request and creates the corresponding Action trait object.
/// Attempts to extract a JSON object even if it's surrounded by other text.
pub fn parse_and_create_action(llm_response: &str) -> Result<Box<dyn Action>> {
    // Find the first '{' and the last '}'
    let json_start = llm_response.find('{');
    let json_end = llm_response.rfind('}');

    let potential_json_str = match (json_start, json_end) {
        (Some(start), Some(end)) if end > start => {
            &llm_response[start..=end] // Extract the potential JSON block
        },
        (Some(_), Some(_)) => {
             // Found '{' and '}' but '}' is before '{'
             warn!(response = %llm_response, "Found '}}' before '{{' in LLM response, attempting parse on full string.");
             llm_response
        },
        (Some(_), None) => {
             // Found '{' but no '}'
             warn!(response = %llm_response, "Found '{{' but no '}}' in LLM response, attempting parse on full string.");
             llm_response
        },
        (None, _) => {
             // No '{' found
             warn!(response = %llm_response, "No '{{' found in LLM response, attempting parse on full string.");
             llm_response
        }
    };

    // Try parsing the extracted/potential JSON first
    match serde_json::from_str::<ActionRequest>(potential_json_str) {
        Ok(request) => create_action_from_request(request), // If success, create action
        Err(e_extract) => {
            // If extracted parse failed, AND we actually extracted something different than the original response
            if potential_json_str != llm_response {
                 warn!(response = %llm_response, extracted = %potential_json_str, error = %e_extract, "Failed to parse extracted JSON, attempting parse on full response.");
                 // Try parsing the original full response string as a fallback
                 match serde_json::from_str::<ActionRequest>(llm_response) {
                     Ok(request) => create_action_from_request(request),
                     Err(e_full) => {
                         error!(
                             original_response = %llm_response,
                             extraction_error = %e_extract,
                             full_parse_error = %e_full,
                             "Failed to parse both extracted JSON and original response."
                         );
                         Err(RelayError::ChainError(format!(
                             "Invalid action request format. Failed initial parse ({}) and fallback parse ({}).", e_extract, e_full
                         )))
                     }
                 }
            } else {
                 // If we didn't extract anything different (e.g., no {} found), the first error is the only one.
                 error!(
                     response = %llm_response,
                     error = %e_extract,
                     "Failed to deserialize action request JSON (no valid extraction attempted)."
                 );
                 Err(RelayError::ChainError(format!(
                     "Invalid action request JSON: {}. Original response: '{}'", e_extract, llm_response
                 )))
            }
        }
    }
}

// Helper function to create the action from the parsed request
// (Extracted from the original match statement for clarity)
fn create_action_from_request(request: ActionRequest) -> Result<Box<dyn Action>> {
    match request.action.as_str() {
        // --- File Actions --- 
        "read_file" => {
            let params: ReadFileParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for read_file: {}", e)))?;
            let action = ReadFileAction::new(params.path, params.start_line, params.end_line);
            Ok(Box::new(action))
        }
        "write_file" => {
            let params: WriteFileParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for write_file: {}", e)))?;
            let action = WriteFileAction::new(params.path, params.content, params.create_dirs);
            Ok(Box::new(action))
        }
        "create_directory" => {
             let params: CreateDirectoryParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for create_directory: {}", e)))?;
            let action = CreateDirectoryAction::new(params.path);
            Ok(Box::new(action))
        },
        "line_edit" => {
            let params: LineEditParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for line_edit: {}", e)))?;
            let start_line = params.start_line;
            let end_line = params.end_line;
            let action = LineEditAction::new(params.path, start_line, end_line, params.content);
            Ok(Box::new(action))
        }

        // --- Command Action ---
        "run_command" => {
            let params: RunCommandParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for run_command: {}", e)))?;
            let action = RunCommandAction::new(params.command, params.cwd, params.timeout_secs);
            Ok(Box::new(action))
        }

        // --- Git Actions ---
        "git_status" => {
             let params: GitStatusParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for git_status: {}", e)))?;
            let action = GitStatusAction::new(params.repo_path);
             Ok(Box::new(action))
        },
        "git_add" => {
             let params: GitAddParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for git_add: {}", e)))?;
            let action = GitAddAction::new(params.paths, params.repo_path);
             Ok(Box::new(action))
        },
        "git_commit" => {
             let params: GitCommitParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for git_commit: {}", e)))?;
            let action = GitCommitAction::new(params.message, params.repo_path, params.author_name, params.author_email);
             Ok(Box::new(action))
        },
        // TODO: Add other git actions (log, commit, etc.)

        // --- Repo Actions ---
        "init_repo" => {
            let params: InitRepoParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for init_repo: {}", e)))?;
            let action = InitRepoAction::new(params.path);
            Ok(Box::new(action))
        },
        "add_repository" => {
            let params: AddRepoParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for add_repository: {}", e)))?;
            let action = AddRepoAction::new(
                params.name, 
                params.url, 
                params.local_path, 
                params.branch, 
                params.extensions
            );
            Ok(Box::new(action))
        },
        "use_repository" => {
            let params: UseRepoParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for use_repository: {}", e)))?;
            let action = UseRepoAction::new(params.name);
            Ok(Box::new(action))
        },
        "list_repositories" => {
            let params: ListRepositoriesParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for list_repositories: {}", e)))?;
            let action = ListRepositoriesAction::new(params);
            Ok(Box::new(action))
        },
        "remove_repository" => {
            let params: RemoveRepositoryParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for remove_repository: {}", e)))?;
            let action = RemoveRepositoryAction::new(params);
            Ok(Box::new(action))
        },
         "sync_repository" => {
            let params: SyncRepositoryParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Invalid params for sync_repository: {}", e)))?;
            let action = SyncRepositoryAction::new(params);
            Ok(Box::new(action))
        },

        // --- Search Actions ---
        "semantic_search" => {
            let params: SemanticSearchParams = serde_json::from_value(request.params)
                 .map_err(|e| RelayError::ToolError(format!("Invalid params for semantic_search: {}", e)))?;
            let action = SemanticSearchAction::new(
                params.query, 
                params.limit, 
                params.repo_name, 
                params.lang, 
                params.element_type, 
                params.branch
            );
             Ok(Box::new(action))
        },
        "semantic_edit" => {
            let params: SemanticEditParams = serde_json::from_value(request.params)
                .map_err(|e| RelayError::ToolError(format!("Failed to parse semantic_edit parameters: {}", e)))?;
            let action = SemanticEditAction::new(
                params.file_path,
                params.element_query,
                params.edit_content,
                params.update_references,
                None,
                None,
                None,
                None,
            );
            Ok(Box::new(action))
        },

        // --- Unknown Action ---
        unknown_action => {
            error!(action_name = %unknown_action, "Received request for unknown action");
            Err(RelayError::ToolError(format!(
                "Unknown action requested: {}",
                unknown_action
            )))
        }
    }
} 