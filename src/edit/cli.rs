// src/edit/cli.rs
//! Defines the CLI arguments and handlers for the `edit` subcommand.

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use log::debug;
use std::path::PathBuf;
use std::sync::Arc;
use qdrant_client::Qdrant;
use colored::Colorize;

// Use config types from vectordb_core
use vectordb_core::AppConfig;
use crate::cli::commands::CliArgs; // To access global args like model paths
use crate::edit::engine::{self, EditTarget, EngineEditOptions}; // Removed EngineValidationSeverity
use std::fmt::Debug;
// NOTE: edit submodules haven't been moved to core yet. Commenting out imports.
// use crate::edit::editor::Editor;
// use crate::edit::validation::validate_edit;
// use crate::edit::execution::apply_edit;
// use crate::edit::semantic::{SemanticElement, SemanticQueryType};

// Use the prompt function from vectordb_core
// use vectordb_core::prompt::prompt_for_edit_confirmation; // Path seems incorrect
// Re-import prompt_for_confirmation assuming it's at crate::utils or similar
// use crate::utils::prompt_for_confirmation; // Assuming edit confirmation is also here or renamed // REMOVED UNUSED IMPORT

#[derive(Args, Debug, Clone)]
pub struct EditArgs {
    #[command(subcommand)]
    pub command: EditCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum EditCommand {
    /// Apply an edit to the specified target.
    Apply(ApplyArgs),
    /// Validate an edit without applying it.
    Validate(ValidateArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ApplyArgs {
    /// Path to the file to edit.
    #[arg(long = "file")]
    pub file_path: PathBuf,

    /// Content of the edit to apply.
    #[arg(long)]
    pub edit_content: String,

    // --- Target Specification (Mutually Exclusive) ---
    /// Starting line number for the edit (1-based).
    #[arg(long, group = "target")]
    pub start_line: Option<u32>,

    /// Ending line number for the edit (1-based, inclusive).
    #[arg(long, requires = "start_line")]
    pub end_line: Option<u32>,

    /// Semantic element query (e.g., "function MyFunc").
    #[arg(long, group = "target")]
    pub element_query: Option<String>,

    // --- Edit Options ---
    /// Attempt to update references related to the edited code.
    #[arg(long, default_value_t = false)]
    pub update_references: bool,

    /// Do not automatically format the code after applying the edit.
    #[arg(long, default_value_t = false)]
    pub no_format: bool,

    /// Do not try to preserve documentation comments during the edit.
    #[arg(long, default_value_t = false)]
    pub no_preserve_docs: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ValidateArgs {
    /// Path to the file to edit.
    #[arg(long = "file")]
    pub file_path: PathBuf,

    /// Content of the edit to validate.
    #[arg(long)]
    pub edit_content: String,

    // --- Target Specification (Mutually Exclusive) ---
    /// Starting line number for the edit (1-based).
    #[arg(long, group = "target")]
    pub start_line: Option<u32>,

    /// Ending line number for the edit (1-based, inclusive).
    #[arg(long, requires = "start_line")]
    pub end_line: Option<u32>,

    /// Semantic element query (e.g., "function MyFunc").
    #[arg(long, group = "target")]
    pub element_query: Option<String>,

    // --- Edit Options ---
    /// Attempt to update references related to the edited code.
    #[arg(long, default_value_t = false)]
    pub update_references: bool,

    /// Do not automatically format the code after applying the edit.
    #[arg(long, default_value_t = false)]
    pub no_format: bool,

    /// Do not try to preserve documentation comments during the edit.
    #[arg(long, default_value_t = false)]
    pub no_preserve_docs: bool,
}

/// Main handler for the `edit` command group.
pub async fn handle_edit_command(
    _edit_args: EditArgs,
    _cli_args: &CliArgs,
    _config: AppConfig,
    _client: Arc<Qdrant>,
) -> Result<()> {
    // Placeholder: Implement actual logic for handling edit commands
    // This might involve calling functions from the `edit` module
    // based on the specific subcommand in `edit_args.command`.
    // For now, it just logs and returns Ok.
    debug!("Handling edit command (placeholder)");
    Ok(())
}

/// Combined handler for Apply and Validate logic
async fn handle_apply_or_validate(
    args: ApplyArgs, // Use ApplyArgs as it contains all necessary fields
    _cli_args: &CliArgs, // cli_args might not be needed if engine doesn't require ONNX paths directly
    _config: AppConfig, // config might not be needed if engine doesn't require it
    validate_only: bool,
) -> Result<()> {
    // --- Target Resolution ---
    let file_path = args.file_path;
    let target = match (args.start_line, args.end_line, args.element_query) {
        (Some(start), Some(end), None) => {
            EditTarget::LineRange { start: start as usize, end: end as usize } // Convert u32 to usize
        },
        (None, None, Some(query)) => {
            EditTarget::Semantic { element_query: query }
        },
        (Some(start), None, None) => {
             EditTarget::LineRange { start: start as usize, end: start as usize } // Convert u32 to usize
        },
        _ => bail!("Invalid combination of target specifiers. Use either --start-line [--end-line] OR --element-query.")
    };

    // --- Edit Options ---
    let options = EngineEditOptions {
        update_references: args.update_references,
        format_code: !args.no_format,
        // preserve_documentation field removed
    };

    // --- Perform Action (Validate or Apply) ---
    if validate_only {
        println!("Validating edit...");
        let issues = engine::validate_edit(
            &file_path,
            &target, // Pass target by reference
            &args.edit_content,
            Some(&options), // Pass options by reference
        )?;

        if issues.is_empty() {
            println!("{}", "Validation successful. No issues found.".green());
        } else {
            println!("{}", format!("Validation found {} issues:", issues.len()).yellow());
            for issue in issues {
                println!(
                    "- [{:?}] {}:{} - {}",
                    issue.severity,
                    file_path.display(),
                    issue.line_number.unwrap_or(0),
                    issue.message
                );
            }
            return Err(anyhow!("Validation failed."));
        }
    } else {
        println!("Applying edit...");
        engine::apply_edit(
            &file_path,
            &target, // Pass target by reference
            &args.edit_content,
            Some(&options), // Pass options by reference
        ).context("Failed to apply edit")?;
        println!("{}", "Edit applied successfully.".green());
        // Note: apply_edit doesn't return the diff content currently
    }

    Ok(())
} 