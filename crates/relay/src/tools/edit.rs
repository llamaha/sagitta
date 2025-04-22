// src/tools/edit.rs

use crate::chain::action::Action;
use crate::chain::state::ChainState;
// use crate::chain::message::ChainMessage; // Removed unused import
use crate::context::AppContext;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

// Use direct imports from vectordb_core root
use vectordb_core::{apply_edit, EditTarget, EngineEditOptions};

// Remove imports for items no longer used directly from core
// use vectordb_core::edit::cli::{self as edit_cli, EditArgs, EditCommand}; // Import edit CLI structures
// use vectordb_core::cli::commands::CliArgs;

use std::path::PathBuf;
use std::sync::Arc;

// --- Semantic Edit Action ---
// Calls vectordb_core::apply_edit

#[derive(Debug, Serialize, Deserialize)]
pub struct SemanticEditParams {
    pub file_path: String,     // Target file path (renamed for clarity)
    pub element_query: String, // Element query (e.g., "function:my_func") (renamed)
    pub edit_content: String,  // New content to insert (renamed)
    pub update_references: Option<bool>, // Renamed from fuzzy
    // Add other relevant options if apply_edit supports them
    pub start_line: Option<usize>, // Added optional line numbers
    pub end_line: Option<usize>,
    pub no_format: Option<bool>,
    pub no_preserve_docs: Option<bool>,
}

#[derive(Debug)]
pub struct SemanticEditAction {
    params: SemanticEditParams,
}

impl SemanticEditAction {
    // Update constructor signature
    pub fn new(
        file_path: String, 
        element_query: String, 
        edit_content: String, 
        update_references: Option<bool>, 
        start_line: Option<usize>,
        end_line: Option<usize>,
        no_format: Option<bool>,
        no_preserve_docs: Option<bool>
    ) -> Self {
        Self { params: SemanticEditParams { 
            file_path, 
            element_query, 
            edit_content, 
            update_references, 
            start_line, 
            end_line, 
            no_format, 
            no_preserve_docs 
        } }
    }
}

#[async_trait]
impl Action for SemanticEditAction {
    fn name(&self) -> &'static str {
        "semantic_edit"
    }

    async fn execute(&self, context: &AppContext, state: &mut ChainState) -> Result<()> {
        debug!(params = ?self.params, "Executing SemanticEditAction using library function");

        // --- Dependencies ---
        let vdb_config = Arc::clone(&context.vdb_config);
        let qdrant_client = Arc::clone(&context.qdrant_client);

        // --- Construct EditTarget --- 
        let target = EditTarget::Semantic { 
            element_query: self.params.element_query.clone() 
        };

        // --- Construct Options (Optional) ---
        // Create options based on params if needed, otherwise pass None
        let options = EngineEditOptions {
            // Map params to options if apply_edit uses them
            format_code: self.params.no_format.map_or(true, |b| !b), // Assuming no_format=true means format_code=false
            update_references: self.params.update_references.unwrap_or(false),
            preserve_documentation: self.params.no_preserve_docs.map_or(true, |b| !b), // Default to true if not provided
        };

        // --- Call Library Function --- 
        match apply_edit(
            // Arguments must match vectordb_core::apply_edit signature:
            // pub fn apply_edit(
            //     file_path: &Path,
            //     target: &EditTarget,
            //     new_content: &str,
            //     options: Option<&EngineEditOptions>,
            // ) -> Result<()> 
            &PathBuf::from(&self.params.file_path), // Pass as &Path
            &target, // Pass the constructed target
            &self.params.edit_content, // Pass as &str
            Some(&options), // Pass options (or None)
        ) { // Remove .await as apply_edit is synchronous
            Ok(_) => {
                info!(file = %self.params.file_path, element = %self.params.element_query, "Semantic edit applied successfully.");
                let success_msg = format!("Successfully applied semantic edit to element '{}' in file '{}'.", self.params.element_query, self.params.file_path);
                state.set_context("last_action_status".to_string(), "success".to_string())
                     .map_err(RelayError::SerializationError)?;
                state.set_context("last_action_message".to_string(), success_msg)
                    .map_err(RelayError::SerializationError)?;
                Ok(())
            }
            Err(e) => {
                error!(file = %self.params.file_path, element = %self.params.element_query, error = %e, "Failed to apply semantic edit");
                let err_msg = format!("Failed to apply semantic edit to element '{}' in file '{}': {}", self.params.element_query, self.params.file_path, e);
                state.set_context("last_action_status".to_string(), "error".to_string())
                     .map_err(RelayError::SerializationError)?;
                state.set_context("last_action_error".to_string(), err_msg.clone())
                     .map_err(RelayError::SerializationError)?;
                Err(RelayError::ToolError(err_msg))
            }
        }
    }
} 