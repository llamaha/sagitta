use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info, warn};
use crate::context::AppContext;
use std::io::{self, BufReader, BufRead};
use std::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use std::path::Path;

// --- Read File Action ---

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileParams {
    pub path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

#[derive(Debug)]
pub struct ReadFileAction {
    params: ReadFileParams,
}

impl ReadFileAction {
    pub fn new(path: String, start_line: Option<usize>, end_line: Option<usize>) -> Self {
        Self { params: ReadFileParams { path, start_line, end_line } }
    }
}

#[async_trait]
impl Action for ReadFileAction {
    fn name(&self) -> &'static str {
        "read_file"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let path_str = &self.params.path;
        let start_line_1_based = self.params.start_line;
        let end_line_1_based = self.params.end_line;
        let file_path = PathBuf::from(path_str);

        let resolved_path = if file_path.is_relative() {
            if let Some(cwd) = &state.current_directory {
                let base_path = PathBuf::from(cwd);
                base_path.join(file_path)
            } else {
                warn!(path = %path_str, "Cannot resolve relative path: current_directory not set in ChainState.");
                // Return an error instead of falling back, as relative paths need a base.
                return Err(RelayError::ToolError("Cannot resolve relative path: current_directory not set.".to_string()));
            }
        } else {
            file_path // Path is absolute, use it directly
        };

        debug!(path = %resolved_path.display(), ?start_line_1_based, ?end_line_1_based, "Executing ReadFileAction");

        // --- Line Reading Logic ---
        let file = File::open(&resolved_path)
            .map_err(|e| RelayError::ToolError(format!("Failed to open file '{}': {}", resolved_path.display(), e)))?;
        let reader = BufReader::new(file);
        let mut lines_content = Vec::new();
        let mut current_line_num = 0;

        // Determine the range (0-based for internal logic)
        let start_0_based = start_line_1_based.map(|n| n.saturating_sub(1)); // Convert 1-based to 0-based
        let end_0_based = end_line_1_based.map(|n| n.saturating_sub(1));

        for line_result in reader.lines() {
            let line = line_result.map_err(|e| RelayError::ToolError(format!("Failed to read line from '{}': {}", resolved_path.display(), e)))?;
            
            let process_line = match (start_0_based, end_0_based) {
                (Some(start), Some(end)) => current_line_num >= start && current_line_num <= end,
                (Some(start), None) => current_line_num >= start,
                (None, Some(end)) => current_line_num <= end,
                (None, None) => true, // No range specified, read all lines
            };

            if process_line {
                lines_content.push(line);
            }

            current_line_num += 1;

            // Optimization: Stop reading if we are past the desired end line
            if let Some(end) = end_0_based {
                if current_line_num > end {
                    break;
                }
            }
        }

        // Validate range if provided
        if let (Some(start_1), Some(end_1)) = (start_line_1_based, end_line_1_based) {
            if start_1 > end_1 {
                return Err(RelayError::ToolError(format!("Invalid line range: start_line ({}) > end_line ({})", start_1, end_1)));
            }
            if start_1 == 0 {
                return Err(RelayError::ToolError("Invalid line range: start_line must be >= 1".to_string()));
            }
            // We don't need to check end_1 against total lines here, as it simply won't return lines that don't exist.
        } else if let Some(start_1) = start_line_1_based {
             if start_1 == 0 {
                return Err(RelayError::ToolError("Invalid line range: start_line must be >= 1".to_string()));
            }
        } else if let Some(end_1) = end_line_1_based {
             if end_1 == 0 {
                // Technically line 0 doesn't exist, but end_line=0 might be intended to mean "no lines"
                 warn!(path = %resolved_path.display(), "end_line was 0, returning empty content.");
                lines_content.clear(); // Ensure content is empty if end_line is 0
            }
        }

        let final_content = lines_content.join("\n");
        // --- End Line Reading Logic ---

        // Store the content in the context
        state.set_context(format!("file_content_{}", path_str), final_content)
             .map_err(|e| RelayError::ToolError(format!("Failed to set context for file '{}': {}", path_str, e)))?;

        Ok(())
    }
}

// --- Write File Action ---

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileParams {
    pub path: String,
    pub content: String,
    pub create_dirs: Option<bool>,
}

#[derive(Debug)]
pub struct WriteFileAction {
    params: WriteFileParams,
}

impl WriteFileAction {
    pub fn new(path: String, content: String, create_dirs: Option<bool>) -> Self {
        Self { params: WriteFileParams { path, content, create_dirs } }
    }
}

#[async_trait]
impl Action for WriteFileAction {
    fn name(&self) -> &'static str {
        "write_file"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let path_str = &self.params.path;
        let file_path = PathBuf::from(path_str);

        let resolved_path = if file_path.is_relative() {
            if let Some(cwd) = &state.current_directory {
                let base_path = PathBuf::from(cwd);
                base_path.join(file_path)
            } else {
                warn!(path = %path_str, "Cannot resolve relative path: current_directory not set in ChainState.");
                file_path // Fallback to using the path as is
            }
        } else {
            file_path // Path is absolute, use it directly
        };

        debug!(path = %resolved_path.display(), "Executing WriteFileAction");

        if self.params.create_dirs.unwrap_or(false) {
            if let Some(parent) = resolved_path.parent() {
                fs::create_dir_all(parent).await
                    .map_err(|e| RelayError::ToolError(format!("Failed to create directories for '{}': {}", resolved_path.display(), e)))?;
            }
        }

        fs::write(&resolved_path, &self.params.content).await
            .map_err(|e| RelayError::ToolError(format!("Failed to write file '{}': {}", resolved_path.display(), e)))?;

        info!(path = %resolved_path.display(), "File written successfully");
        // Optional: Update state context if needed (e.g., confirm write)
        Ok(())
    }
}

// --- Create Directory Action ---

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDirectoryParams {
    pub path: String,
}

#[derive(Debug)]
pub struct CreateDirectoryAction {
    params: CreateDirectoryParams,
}

impl CreateDirectoryAction {
    pub fn new(path: String) -> Self {
        Self { params: CreateDirectoryParams { path } }
    }
}

#[async_trait]
impl Action for CreateDirectoryAction {
    fn name(&self) -> &'static str {
        "create_directory"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let path_str = &self.params.path;
        let dir_path = PathBuf::from(path_str);

        let resolved_path = if dir_path.is_relative() {
            if let Some(cwd) = &state.current_directory {
                let base_path = PathBuf::from(cwd);
                base_path.join(dir_path)
            } else {
                warn!(path = %path_str, "Cannot resolve relative path: current_directory not set in ChainState.");
                dir_path // Fallback to using the path as is
            }
        } else {
            dir_path // Path is absolute, use it directly
        };

        debug!(path = %resolved_path.display(), "Executing CreateDirectoryAction");

        fs::create_dir_all(&resolved_path).await
            .map_err(|e| RelayError::ToolError(format!("Failed to create directory '{}': {}", resolved_path.display(), e)))?;

        info!(path = %resolved_path.display(), "Directory created successfully");
        // Optional: Update state context
        Ok(())
    }
}

// --- Line Edit Action ---

#[derive(Debug, Serialize, Deserialize)]
pub struct LineEditParams {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

#[derive(Debug)]
pub struct LineEditAction {
    params: LineEditParams,
}

impl LineEditAction {
    pub fn new(path: String, start_line: usize, end_line: usize, content: String) -> Self {
        // Basic validation
        if start_line == 0 || end_line < start_line {
             warn!(start=%start_line, end=%end_line, "Invalid line range for LineEditAction");
             // Consider returning an error during creation?
        }
        Self { params: LineEditParams { path, start_line, end_line, content } }
    }
}

#[async_trait]
impl Action for LineEditAction {
    fn name(&self) -> &'static str {
        "line_edit"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let path_str = &self.params.path;
        let file_path = PathBuf::from(path_str);
        let start_line_0 = self.params.start_line.saturating_sub(1); // Convert to 0-based
        let end_line_0 = self.params.end_line.saturating_sub(1);     // Convert to 0-based

        let resolved_path = if file_path.is_relative() {
            if let Some(cwd) = &state.current_directory {
                let base_path = PathBuf::from(cwd);
                base_path.join(file_path)
            } else {
                warn!(path = %path_str, "Cannot resolve relative path for line edit: current_directory not set in ChainState.");
                file_path // Fallback to using the path as is
            }
        } else {
            file_path // Path is absolute, use it directly
        };

        debug!(path = %resolved_path.display(), start=%self.params.start_line, end=%self.params.end_line, "Executing LineEditAction");

        if !resolved_path.exists() {
             return Err(RelayError::ToolError(format!("File not found for line edit: {}", resolved_path.display())));
        }

        // --- Read existing content line by line ---
        // Using sync read here for simplicity in line processing. Can be optimized.
        let existing_content = std::fs::read_to_string(&resolved_path)
             .map_err(|e| RelayError::ToolError(format!("Failed to read file for line edit '{}': {}", resolved_path.display(), e)))?;
        let lines: Vec<&str> = existing_content.lines().collect();

        // --- Construct new content ---
        let mut new_content_lines: Vec<String> = Vec::new();

        // 1. Add lines before the edit range
        new_content_lines.extend(lines.iter().take(start_line_0).map(|s| s.to_string()));

        // 2. Add the new content
        new_content_lines.extend(self.params.content.lines().map(|s| s.to_string()));

        // 3. Add lines after the edit range
        new_content_lines.extend(lines.iter().skip(end_line_0 + 1).map(|s| s.to_string()));

        let new_content = new_content_lines.join("\n");

        // --- Write new content back asynchronously ---
        // Open file for writing (truncate existing)
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&resolved_path)
            .await
            .map_err(|e| RelayError::ToolError(format!("Failed to open file '{}' for writing edit: {}", resolved_path.display(), e)))?;

        file.write_all(new_content.as_bytes()).await
            .map_err(|e| RelayError::ToolError(format!("Failed to write edited content to '{}': {}", resolved_path.display(), e)))?;

        file.flush().await
             .map_err(|e| RelayError::ToolError(format!("Failed to flush edited content for '{}': {}", resolved_path.display(), e)))?;

        info!(path=%resolved_path.display(), start=%self.params.start_line, end=%self.params.end_line, "Line edit applied successfully.");
        state.set_context(format!("line_edit_result_{}", path_str), "Success".to_string())
            .map_err(|e| RelayError::ToolError(format!("Failed to set context for line edit result: {}", e)))?;

        Ok(())
    }
} 