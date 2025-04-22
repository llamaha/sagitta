// src/tools/edit.rs

use crate::chain::action::Action;
use crate::chain::state::ChainState;
// use crate::chain::message::ChainMessage; // Removed unused import
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use vectordb_lib::edit::cli::{self as edit_cli, EditArgs, EditCommand}; // Import edit CLI structures
use std::path::PathBuf;
use vectordb_lib::cli::commands::CliArgs;
use vectordb_core::config::AppConfig as VdbConfig;
use std::sync::Arc;

// --- Semantic Edit Action ---
// Corresponds roughly to `vectordb-cli edit apply`

#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticEditParams {
    pub file: String,          // Target file path
    pub element: String,       // Element query (e.g., "function:my_func")
    pub content: String,       // New content to insert
    // Add other options from EditApplyArgs if needed (e.g., context_lines, fuzzy)
    pub fuzzy: Option<bool>,
    pub confirm: Option<bool>, // Auto-confirm? Defaults to no for safety.
}

#[derive(Debug)]
pub struct SemanticEditAction {
    params: SemanticEditParams,
}

impl SemanticEditAction {
    pub fn new(file: String, element: String, content: String, fuzzy: Option<bool>, confirm: Option<bool>) -> Self {
        Self { params: SemanticEditParams { file, element, content, fuzzy, confirm } }
    }
}

#[async_trait]
impl Action for SemanticEditAction {
    fn name(&self) -> &'static str {
        "semantic_edit"
    }

    async fn execute(&self, context: &AppContext, state: &mut ChainState) -> Result<()> {
        debug!(params = ?self.params, "Executing SemanticEditAction");

        // Semantic editing requires vectordb_lib config
        let vdb_config = &context.vdb_config;

        // We need to construct the EditArgs structure that the handler expects
        let edit_args = EditArgs {
            command: EditCommand::Apply(edit_cli::ApplyArgs {
                file_path: PathBuf::from(self.params.file.clone()),
                edit_content: self.params.content.clone(),
                start_line: None,
                end_line: None,
                element_query: Some(self.params.element.clone()),
                update_references: false,
                no_format: false,
                no_preserve_docs: false,
            })
        };
        
        // Check if handler actually uses `content` or requires `content_file`
        // Based on vectordb_lib v1.6.0, it seems to prefer content_file.
        // Let's create a temporary file.
        use tempfile::NamedTempFile;
        use std::io::Write;

        let mut temp_file = NamedTempFile::new()
             .map_err(|e| RelayError::ToolError(format!("Failed to create temp file for semantic edit: {}", e)))?;
        temp_file.write_all(self.params.content.as_bytes())
             .map_err(|e| RelayError::ToolError(format!("Failed to write content to temp file: {}", e)))?;
        let temp_file_path = temp_file.path().to_path_buf();
        
        let edit_args_with_file = EditArgs {
             command: EditCommand::Apply(edit_cli::ApplyArgs {
                 file_path: PathBuf::from(self.params.file.clone()),
                 edit_content: self.params.content.clone(),
                 start_line: None,
                 end_line: None,
                 element_query: Some(self.params.element.clone()),
                 update_references: self.params.fuzzy.unwrap_or(false),
                 no_format: false,
                 no_preserve_docs: false,
             })
        };
        
        // Create CLI args needed for the handler
        let cli_args = CliArgs::default();
        let app_config = VdbConfig::default();

        // Call the main edit handler function
        // Note: handle_edit_command is async
        match edit_cli::handle_edit_command(edit_args_with_file, &cli_args, app_config, context.qdrant_client.clone()).await {
            Ok(_) => {
                info!(file = %self.params.file, element = %self.params.element, "Semantic edit applied successfully.");
                state.set_context(format!("semantic_edit_result_{}_{}", self.params.file, self.params.element), "Success".to_string())
                     .map_err(|e| RelayError::ToolError(format!("Failed to set context for semantic edit result: {}", e)))?;
                 // The temp file is automatically deleted when `temp_file` goes out of scope
                Ok(())
            }
            Err(e) => {
                error!(file = %self.params.file, element = %self.params.element, error = %e, "Failed to apply semantic edit");
                 state.set_context(format!("semantic_edit_error_{}_{}", self.params.file, self.params.element), e.to_string())
                    .map_err(|e_ctx| RelayError::ToolError(format!("Failed to set context for semantic edit error: {}", e_ctx)))?;
                 // The temp file is automatically deleted even on error
                Err(RelayError::ToolError(format!("Failed to apply semantic edit: {}", e)))
            }
        }
    }
} 