// src/edit/cli.rs
//! Defines the CLI arguments and handlers for the `edit` subcommand.

use anyhow::{Result, Context, bail};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use qdrant_client::Qdrant;

use crate::config::AppConfig;
use crate::cli::commands::CliArgs; // To access global args like model paths
use crate::edit::engine::{self, EditTarget, EngineValidationSeverity, EngineEditOptions}; // Import new types

#[derive(Args, Debug, Clone)]
pub struct EditArgs {
    #[command(subcommand)]
    pub command: EditCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum EditCommand {
    /// Apply a specific code edit to a file.
    Apply(ApplyArgs),
    /// Validate a potential code edit without applying it.
    Validate(ValidateArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ApplyArgs {
    /// Path to the file to edit.
    #[arg(long, required = true)]
    pub file: PathBuf,

    /// Starting line number for the edit (1-based, inclusive).
    #[arg(long)]
    pub line_start: Option<usize>,

    /// Ending line number for the edit (1-based, inclusive).
    #[arg(long)]
    pub line_end: Option<usize>,

    /// Semantic element to target (e.g., "function:my_func", "class:MyClass.method:new").
    #[arg(long)]
    pub element: Option<String>,

    /// Path to a file containing the new content.
    #[arg(long)]
    pub content_file: Option<PathBuf>,

    /// Inline content for the edit.
    #[arg(long)]
    pub content: Option<String>,

    /// Automatically format the edited code block (if supported by language).
    #[arg(long, default_value_t = false)]
    pub format: bool,

    /// Automatically update references to the edited element (requires semantic analysis).
    #[arg(long, default_value_t = false)]
    pub update_references: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ValidateArgs {
    /// Path to the file to validate against.
    #[arg(long, required = true)]
    pub file: PathBuf,

    /// Starting line number for the edit (1-based, inclusive).
    #[arg(long)]
    pub line_start: Option<usize>,

    /// Ending line number for the edit (1-based, inclusive).
    #[arg(long)]
    pub line_end: Option<usize>,

    /// Semantic element to target (e.g., "function:my_func").
    #[arg(long)]
    pub element: Option<String>,

    /// Path to a file containing the new content.
    #[arg(long)]
    pub content_file: Option<PathBuf>,

    /// Inline content for the edit.
    #[arg(long)]
    pub content: Option<String>,

    /// Automatically format the edited code block (if supported by language).
    #[arg(long, default_value_t = false)]
    pub format: bool,

    /// Automatically update references to the edited element (requires semantic analysis).
    #[arg(long, default_value_t = false)]
    pub update_references: bool,
}

/// Main handler for the `edit` command group.
pub async fn handle_edit_command(
    args: EditArgs,
    _global_args: &CliArgs, // Keep for future use (e.g., accessing model paths)
    _config: AppConfig,     // Keep for future use
    _client: Arc<Qdrant>,    // Keep for future use (semantic analysis)
) -> Result<()> {
    match args.command {
        EditCommand::Apply(apply_args) => handle_apply(apply_args).await,
        EditCommand::Validate(validate_args) => handle_validate(validate_args).await,
    }
}

/// Handler for the `edit apply` subcommand.
async fn handle_apply(args: ApplyArgs) -> Result<()> {
    println!("Applying edit to file: {:?}", args.file);

    // 1. Determine the target
    let target = determine_target(&args.line_start, &args.line_end, &args.element)?;

    // 2. Get the content
    let new_content = get_content(&args.content_file, &args.content)?;

    // 3. Construct options
    let options = EngineEditOptions {
        format_code: args.format,
        update_references: args.update_references,
    };

    // 4. Call the engine function, passing options
    engine::apply_edit(&args.file, &target, &new_content, Some(&options))
        .with_context(|| format!("Failed to apply edit to file: {:?}", args.file))?;

    println!("Edit applied successfully.");

    // Note: Reference update logic would happen here if enabled in options

    Ok(())
}

/// Handler for the `edit validate` subcommand.
async fn handle_validate(args: ValidateArgs) -> Result<()> {
    println!("Validating edit for file: {:?}", args.file);

    // 1. Determine the target
    let target = determine_target(&args.line_start, &args.line_end, &args.element)?;

    // 2. Get the content
    let new_content = get_content(&args.content_file, &args.content)?;

    // 3. Construct options
    let options = EngineEditOptions {
        format_code: args.format,
        update_references: args.update_references,
    };

    // 4. Call the engine function, passing options
    match engine::validate_edit(&args.file, &target, &new_content, Some(&options)) {
        Ok(issues) => {
            if issues.is_empty() {
                println!("Validation successful (basic checks passed).");
            } else {
                println!("Validation finished with the following issues:");
                let mut has_errors = false;
                for issue in issues {
                    let severity_str = match issue.severity {
                        EngineValidationSeverity::Error => { has_errors = true; "ERROR" },
                        EngineValidationSeverity::Warning => "WARNING",
                        EngineValidationSeverity::Info => "INFO",
                    };
                    if let Some(line_num) = issue.line_number {
                        println!("- {}: [Line {}] {}", severity_str, line_num, issue.message);
                    } else {
                        println!("- {}: {}", severity_str, issue.message);
                    }
                }
                
                if has_errors {
                    // Indicate failure via error code if there were any errors
                    anyhow::bail!("Validation failed due to one or more errors.");
                } 
            }
            Ok(())
        }
        Err(e) => {
            // Handle errors from the validation engine itself (e.g., file read error)
            eprintln!("Validation engine error: {}", e);
            anyhow::bail!("Validation failed due to an internal error: {}", e)
        }
    }
}

// Helper to determine EditTarget from CLI args
fn determine_target(line_start: &Option<usize>, line_end: &Option<usize>, element: &Option<String>) -> Result<EditTarget> {
    match (line_start, line_end, element) {
        (Some(start), Some(end), None) => {
             if *start == 0 || *end == 0 { 
                 anyhow::bail!("Line numbers must be 1-based.");
             }
             if *start > *end {
                 anyhow::bail!("Start line ({}) cannot be greater than end line ({}).", start, end);
             }
             Ok(EditTarget::LineRange { start: *start, end: *end })
        }
        (Some(start), None, None) => { 
            // Explicitly handle missing line_end when line_start is present
            anyhow::bail!("Missing --line-end argument, required when using --line-start.");
        }
        (None, None, Some(el)) => {
            if el.is_empty() {
                bail!("Semantic element query (--element) cannot be empty.");
            }
            Ok(EditTarget::Semantic { element_query: el.clone() })
        }
        (None, Some(_), None) => {
             // Handle case where only line_end is given (invalid)
             anyhow::bail!("Missing --line-start argument, required when using --line-end.");
        }
        _ => {
            // Catch other invalid combinations or if clap somehow allows None for both groups
            anyhow::bail!("Invalid target specification. Provide either (--line-start and --line-end) or --element.")
        }
    }
}

// Helper to get content from either file or inline argument
fn get_content(content_file: &Option<PathBuf>, content: &Option<String>) -> Result<String> {
    match (content_file, content) {
        (Some(path), None) => std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read content file '{:?}': {}", path, e)),
        (None, Some(text)) => Ok(text.clone()),
        _ => {
            // This case should be prevented by clap's `group` and `required` attributes.
            anyhow::bail!("Invalid content specification. Use either --content-file or --content.")
        }
    }
} 