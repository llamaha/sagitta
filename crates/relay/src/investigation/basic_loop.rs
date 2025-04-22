// src/investigation/basic_loop.rs

use crate::chain::{ChainExecutor, ChainState, parse_and_create_action};
use crate::context::AppContext;
use crate::llm::message::{AnthropicContent, AnthropicMessage, Role};
use crate::utils::error::{Result, RelayError};
use anyhow::Context;
use futures::StreamExt;
use std::io::{self, Write};
use tracing::{error, info, warn};
use serde_json::Value; // For accessing context results
use termcolor::Color;

const SYSTEM_PROMPT: &str = r#"You are Relay, an AI coding assistant. Your goal is to help users with their coding tasks.

You have access to a set of tools (actions) to interact with the user's environment. When you need to use a tool, you MUST respond ONLY with a JSON object matching the following structure:

{
  "action": "<action_name>",
  "params": { ... parameters specific to the action ... }
}

Available actions:

*   **read_file**: Reads the content of a file.
    Params: {"path": "<path_to_file>", "start_line": <optional_number>, "end_line": <optional_number>}
*   **write_file**: Writes content to a file, overwriting if it exists.
    Params: {"path": "<path_to_file>", "content": "<content_to_write>", "create_dirs": <optional_boolean>}
*   **line_edit**: Replaces a range of lines in a file with new content.
    Params: {"path": "<path_to_file>", "start_line": <line_number>, "end_line": <line_number>, "content": "<new_content>"}
*   **create_directory**: Creates a directory (including parent directories).
    Params: {"path": "<path_to_directory>"}
*   **git_status**: Gets the status of a git repository.
    Params: {"repo_path": "<optional_path_to_repo>"} (Defaults to current directory)
*   **git_add**: Stages changes in a git repository.
    Params: {"paths": ["<path1>", "<path2>", ...], "repo_path": "<optional_path_to_repo>"}
*   **git_commit**: Creates a commit in a git repository.
    Params: {"message": "<commit_message>", "repo_path": "<optional_path_to_repo>", "author_name": "<optional_name>", "author_email": "<optional_email>"}
*   **init_repo**: Initializes a new git repository in a directory.
    Params: {"path": "<path_to_directory>"}
*   **add_repository**: Adds a repository configuration for vectordb_lib.
    Params: {"name": "<repo_name>", "url": "<optional_git_url>", "local_path": "<optional_local_path>", "branch": "<optional_branch>", "extensions": [<optional_list_of_extensions>]}
*   **use_repository**: Sets the active repository for subsequent vectordb_lib operations.
    Params: {"name": "<repo_name>"}
*   **list_repositories**: Lists configured vectordb_lib repositories. Use this instead of `run_command` for listing repositories.
    Params: {}
*   **remove_repository**: Removes a configured vectordb_lib repository. Use this instead of `run_command` for removing repositories.
    Params: {"name": "<repo_name>"}
*   **sync_repository**: Syncs a vectordb_lib repository (fetches updates, updates index). Use this instead of `run_command` for syncing repositories.
    Params: {"name": "<optional_repo_name>"} (Defaults to active repository)
*   **semantic_search**: Performs semantic search over indexed code.
    Params: {"query": "<search_query>", "limit": <optional_number>, "repo_name": "<optional_repo>", "lang": "<optional_language>", "element_type": "<optional_type>"}
*   **semantic_edit**: Applies a semantic edit to a code element.
    Params: {"file": "<path_to_file>", "element": "<element_query>", "content": "<new_content>", "fuzzy": <optional_boolean>, "confirm": <optional_boolean>}
*   **run_command**: Executes a shell command NOT COVERED by the actions above. You MUST ask the user for permission via plain text before using this action.
    Params: {"command": "<shell_command>", "cwd": "<optional_working_directory>", "timeout_secs": <optional_number>}

If you need to ask a clarifying question or provide information, respond in plain text WITHOUT the JSON structure. Do not include explanations or conversational text when responding with a JSON action request.

After you request an action, the system will execute it and provide the result back to you in the next user message (prefixed with "System:"). Use this result to plan your next step.
"#;

// Basic loop runner function
pub async fn run_basic_loop(app_context: AppContext, initial_prompt: String) -> Result<()> {
    info!("Starting basic interaction loop...");
    let mut state = ChainState::new();
    state.current_directory = Some(std::env::current_dir()
        .map_err(|e| RelayError::ToolError(format!("Failed to get current directory: {}", e)))?
        .display().to_string());

    state.initial_query = Some(initial_prompt.clone());
    state.add_history(Role::User, initial_prompt);

    // --- Loop for multiple turns --- 
    const MAX_TURNS: usize = 5; // Limit iterations for now
    for turn in 1..=MAX_TURNS {
        info!(turn, max_turns = MAX_TURNS, "Processing turn");

        // --- LLM Call --- 
        info!("Sending prompt to LLM...");
        let system_prompt = Some(SYSTEM_PROMPT);
        let mut stream = match app_context.llm_client
            .chat_completion_stream(&state.history, system_prompt)
            .await {
                Ok(s) => s,
                Err(e) => {
                    error!(error = %e, "LLM call failed");
                    // Add error to history? Or just fail?
                    return Err(RelayError::ChainError(format!("LLM API call failed during loop: {}", e)));
                }
            };

        let mut llm_response_text = String::new();
        print!("Assistant: "); 
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    print!("{}", chunk);
                    io::stdout().flush().context("Failed to flush stdout").map_err(RelayError::Other)?;
                    llm_response_text.push_str(&chunk);
                }
                Err(e) => {
                    error!(error = %e, "Error receiving stream chunk");
                    return Err(RelayError::ChainError(format!("LLM stream error: {}", e)));
                }
            }
        }
        println!(); // Newline after streaming output

        if llm_response_text.is_empty() {
             warn!("Received empty response from LLM. Ending loop.");
             state.add_history(Role::Assistant, "<empty response>".to_string());
             break; // Exit loop on empty response
        }

        // --- Action Parsing & Execution --- 
        match parse_and_create_action(&llm_response_text) {
            Ok(action) => {
                // Get the action name BEFORE moving the action
                let executed_action_name = action.name(); 
                info!(action_name = %executed_action_name, "LLM requested action");
                state.add_history(Role::Assistant, llm_response_text.clone()); // Add LLM thought (action request)

                // Move the action into the executor
                let executor = ChainExecutor::new().add_action(action); 
                let mut next_state = state.clone(); // Clone state for execution
                
                // Clone the state for execution since execute takes ownership
                let next_state_copy = next_state.clone();
                match executor.execute(&app_context, next_state_copy).await {
                    Ok(final_state) => { // Use the returned state
                        info!("Action executed successfully.");
                        
                        // --- Format Action Result --- 
                        // Pass the correct action name (already captured)
                        let result_summary = format_action_result(executed_action_name, &final_state.context);
                        println!("\nSystem: {}", result_summary); 
                        
                        // Update the main state *after* execution
                        state = final_state;
                        // Add the action *result* summary to history for the LLM
                        state.add_history(Role::User, result_summary); 
                    }
                    Err(e) => {
                        error!(error = %e, "Action execution failed");
                        // Pass the correct action name here too (already captured)
                        let error_summary = format!("Action '{}' failed. Error: {}", executed_action_name, e);
                        eprintln!("\nSystem: {}", error_summary);
                        // Update state history with error summary for the LLM
                         state.add_history(Role::User, error_summary); 
                         // Should we break the loop on action error?
                         warn!("Action failed, continuing loop may lead to unexpected behavior.");
                         // break;
                    }
                }
            }
            Err(_) => {
                info!("LLM response is plain text.");
                state.add_history(Role::Assistant, llm_response_text);
                println!("\n(LLM finished turn with text response. Loop ends.)");
                break; // Exit loop if LLM gives plain text
            }
        }
        
        // Small delay between turns? (Optional)
        // tokio::time::sleep(Duration::from_millis(500)).await;

    } // End of loop
    
    info!("Interaction loop finished.");
    Ok(())
}

/// Helper function to format action results from context for the LLM.
fn format_action_result(action_name: &str, context: &std::collections::HashMap<String, Value>) -> String {
    // Prioritize specific, structured results if available
    if action_name == "list_repositories" {
        if let Some(Ok(repo_list)) = context.get("repository_list").map(|v| serde_json::from_value::<Vec<String>>(v.clone())) {
            if repo_list.is_empty() {
                return "Action 'list_repositories' completed. No repositories configured.".to_string();
            } else {
                // Also check for active repo
                let active_repo_str = context.get("active_repository")
                    .and_then(|v| v.as_str())
                    .map(|s| format!(" (Active: {})", s))
                    .unwrap_or_else(|| "".to_string());
                return format!("Action 'list_repositories' completed. Configured repositories: {}{}", repo_list.join(", "), active_repo_str);
            }
        }
    }
    // TODO: Add specific formatters for other actions (e.g., git_status)

    // Fallback to generic formatting based on common keys
    let relevant_keys: Vec<_> = context.keys()
        .filter(|k| k.contains("stdout") || k.contains("status") || k.contains("oid") || k.contains("summary") || k.contains("result"))
        .filter(|k| !k.contains("error")) // Exclude dedicated error keys from success summary
        .collect();

    let error_keys: Vec<_> = context.keys()
        .filter(|k| k.contains("error"))
        .collect();

    // Handle error case first
    if !error_keys.is_empty() {
        let mut summary = format!("Action '{}' failed. Errors:\n", action_name);
         for key in error_keys {
            if let Some(value) = context.get(key) {
                 let value_str = truncate_value_to_string(value);
                 summary.push_str(&format!("- {}: {}\n", key, value_str));
             }
        }
        return summary.trim_end().to_string();
    }

    // Handle success case
    if relevant_keys.is_empty() {
        return format!("Action '{}' completed successfully.", action_name);
    }

    let mut summary = format!("Action '{}' completed successfully. Results:\n", action_name);
    for key in relevant_keys {
        if let Some(value) = context.get(key) {
            let value_str = truncate_value_to_string(value);
            summary.push_str(&format!("- {}: {}\n", key, value_str));
        }
    }
    summary.trim_end().to_string()
}

// Helper to truncate long strings within JSON values
fn truncate_value_to_string(value: &Value) -> String {
     match value {
        Value::String(s) => {
             if s.len() > 200 {
                 format!("{}... (truncated)", &s[..200])
             } else {
                 s.clone()
             }
        }
         _ => value.to_string(),
    }
} 