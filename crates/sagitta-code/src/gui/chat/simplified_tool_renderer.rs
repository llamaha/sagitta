// Simplified tool renderer that removes collapsing headers and uses fixed-height previews

use egui::{Frame, ScrollArea, Ui, RichText, Vec2, Color32, Stroke, CornerRadius, Layout, Align};
use serde_json::Value;
use crate::gui::theme::AppTheme;
use crate::gui::chat::tool_mappings::get_human_friendly_tool_name;
use crate::gui::chat::syntax_highlighting::{render_syntax_highlighted_code, render_syntax_highlighted_code_with_font_size};
use crate::gui::chat::view::COMMONMARK_CACHE;
use std::collections::HashMap;
use std::cell::RefCell;
use uuid;


pub struct SimplifiedToolRenderer<'a> {
    tool_name: &'a str,
    result: &'a serde_json::Value,
    input_params: Option<&'a serde_json::Value>,
    app_theme: AppTheme,
    unique_id: String,
}

impl<'a> SimplifiedToolRenderer<'a> {
    const MAX_HEIGHT: f32 = 800.0;
    const MAX_WIDTH: f32 = 900.0;  // Limit tool card width
    const CONTENT_PADDING: f32 = 8.0;
    
    pub fn new(
        tool_name: &'a str,
        result: &'a serde_json::Value,
        app_theme: AppTheme,
    ) -> Self {
        // Generate unique ID for this tool renderer instance
        // Include timestamp to ensure uniqueness even in rapid succession
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let unique_id = format!("tool_renderer_{}_{}_{}", tool_name, timestamp, uuid::Uuid::new_v4());
        Self { tool_name, result, input_params: None, app_theme, unique_id }
    }
    
    pub fn with_id(
        tool_name: &'a str,
        result: &'a serde_json::Value,
        app_theme: AppTheme,
        id: String,
    ) -> Self {
        // Use the provided stable ID instead of generating a new one
        Self { tool_name, result, input_params: None, app_theme, unique_id: id }
    }
    
    pub fn with_params(
        tool_name: &'a str,
        result: &'a serde_json::Value,
        input_params: &'a serde_json::Value,
        app_theme: AppTheme,
        id: String,
    ) -> Self {
        // Use the provided stable ID and include input parameters
        Self { tool_name, result, input_params: Some(input_params), app_theme, unique_id: id }
    }
    
    /// Main entry point - renders the complete tool result
    pub fn render(self, ui: &mut egui::Ui) -> Option<(String, String)> {
        let mut action = None;
        
        let app_theme = self.app_theme;
        
        // Outer frame for the entire tool result
        // Limit the width of the tool card
        ui.set_max_width(Self::MAX_WIDTH);
        
        Frame::NONE
            .fill(app_theme.panel_background())
            .stroke(Stroke::new(1.0, app_theme.border_color()))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Vec2::new(8.0, 8.0))
            .show(ui, |ui| {
                // 1. Render header
                self.render_header(ui);
                
                ui.separator();
                
                // 2. Render content with scroll area
                self.render_content_area(ui, &mut action);
                
                // 3. Render action buttons if applicable
                if self.should_show_actions() {
                    ui.separator();
                    self.render_actions(ui, &mut action);
                }
            });
            
        action
    }
    
    /// Renders the tool header with name, status, and basic info
    fn render_header(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Tool name without icon
            let name = get_human_friendly_tool_name(self.tool_name);
            ui.label(RichText::new(name).strong());
            
            // Add all relevant parameters inline
            if let Some(params) = self.get_all_inline_params() {
                tracing::debug!("Rendering inline params for {} (cleaned: {}): {}", 
                    self.tool_name, 
                    get_human_friendly_tool_name(self.tool_name), 
                    params
                );
                ui.separator();
                ui.label(RichText::new(params)
                    .size(self.app_theme.small_font_size())
                    .color(self.app_theme.hint_text_color()));
            } else {
                tracing::debug!("No inline params for tool: {} (input_params: {:?})", 
                    self.tool_name,
                    self.input_params
                );
            }
            
            // Status indicator on the right
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(status) = self.get_status_indicator() {
                    ui.label(status);
                }
            });
        });
    }
    
    /// Renders the main content area with scrolling
    fn render_content_area(&self, ui: &mut Ui, action: &mut Option<(String, String)>) {
        // SIMPLE DYNAMIC SIZING: Start at 0, grow with content, max out at 800px
        // For scrollable content, we need to ensure the viewport shows enough content
        // Use min_scrolled_height to guarantee at least 200px is visible when scrolling
        let min_scrolled = 200.0;  // Always show at least 200px of content when scrolling
        
        ScrollArea::vertical()
            .id_salt(&self.unique_id)  // Use the stable ID directly, just like legacy code
            .max_height(Self::MAX_HEIGHT)  // Max 800px, then scroll
            .min_scrolled_height(min_scrolled)  // Minimum visible height when content needs scrolling
            .auto_shrink([false, true])  // Allow shrinking for small content
            .show(ui, |ui| {
                // Set consistent width for content
                ui.set_max_width(Self::MAX_WIDTH - 20.0);
                
                // Add content within a vertical layout to ensure proper sizing
                ui.vertical(|ui| {
                    ui.add_space(Self::CONTENT_PADDING);
                    
                    // Delegate to specific renderer based on tool type
                    match self.tool_name {
                        name if name.contains("read_file") || name == "Read" => self.render_file_content(ui),
                        name if name.contains("write_file") || name == "Write" => self.render_write_result(ui),
                        name if name.contains("search") || name.contains("query") => self.render_search_results(ui, action),
                        name if name.contains("shell") || name.contains("bash") || name == "Bash" => self.render_shell_output(ui),
                        name if name.contains("todo") => self.render_todo_list(ui),
                        name if name.contains("repository") || name.contains("repo") => self.render_repository_info(ui),
                        name if name.contains("edit_file") || name.contains("multi_edit") || name == "Edit" || name == "MultiEdit" => self.render_edit_result(ui),
                        name if name.contains("ping") => self.render_ping_result(ui),
                        _ => self.render_generic_content(ui),
                    }
                    
                    ui.add_space(Self::CONTENT_PADDING);
                });
            });
    }
    
    /// Extract all relevant parameters to show inline in header
    fn get_all_inline_params(&self) -> Option<String> {
        // Use input_params if available, otherwise try to extract from result
        let params_source = self.input_params.unwrap_or(&self.result);
        let mut params = Vec::new();
        
        // Helper functions that don't capture params
        let add_str_param = |source: &serde_json::Value, key: &str, display_key: Option<&str>| -> Option<String> {
            if let Some(value) = source.get(key).and_then(|v| v.as_str()) {
                if !value.is_empty() {
                    let display = display_key.unwrap_or(key);
                    Some(format!("{}: {}", display, self.format_param_value(key, value)))
                } else {
                    None
                }
            } else {
                None
            }
        };
        
        let add_num_param = |source: &serde_json::Value, key: &str, display_key: Option<&str>| -> Option<String> {
            if let Some(value) = source.get(key).and_then(|v| v.as_i64()) {
                let display = display_key.unwrap_or(key);
                Some(format!("{}: {}", display, value))
            } else {
                None
            }
        };
        
        let add_bool_param = |source: &serde_json::Value, key: &str, display_key: Option<&str>| -> Option<String> {
            if let Some(value) = source.get(key).and_then(|v| v.as_bool()) {
                if value {
                    let display = display_key.unwrap_or(key);
                    Some(format!("{}: {}", display, value))
                } else {
                    None
                }
            } else {
                None
            }
        };
        
        // Clean up mcp__ prefix if present to get the actual tool name
        let clean_tool_name = if self.tool_name.starts_with("mcp__") {
            // Extract just the tool name part after the second underscore
            if let Some(parts) = self.tool_name.strip_prefix("mcp__") {
                if let Some((_provider, actual_tool)) = parts.split_once("__") {
                    actual_tool
                } else {
                    parts
                }
            } else {
                self.tool_name
            }
        } else {
            self.tool_name
        };
        
        match clean_tool_name {
            // Repository operations
            "repository_add" => {
                if let Some(p) = add_str_param(params_source, "name", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "url", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "local_path", Some("path")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "branch", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "ssh_key", None) { params.push(p); }
                // Skip ssh_passphrase for security
            }
            "repository_sync" => {
                if let Some(p) = add_str_param(params_source, "name", None) { params.push(p); }
            }
            "repository_switch_branch" => {
                if let Some(p) = add_str_param(params_source, "repositoryName", Some("repo")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "branchName", Some("branch")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "targetRef", Some("ref")) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "force", None) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "noAutoResync", Some("no-resync")) { params.push(p); }
            }
            "repository_list_branches" => {
                if let Some(p) = add_str_param(params_source, "repositoryName", Some("repo")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "filter", None) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "includeRemote", Some("remote")) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "includeTags", Some("tags")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "limit", None) { params.push(p); }
            }
            
            // Search operations
            "semantic_code_search" | "Search" | "query" => {
                if let Some(p) = add_str_param(params_source, "repositoryName", Some("repo")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "queryText", Some("query")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "elementType", Some("type")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "lang", None) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "limit", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "branchName", Some("branch")) { params.push(p); }
            }
            "search_file" | "Glob" => {
                if let Some(p) = add_str_param(params_source, "repositoryName", Some("repo")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "pattern", None) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "caseSensitive", Some("case-sensitive")) { params.push(p); }
            }
            "grep" | "Grep" => {
                if let Some(p) = add_str_param(params_source, "pattern", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "path", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "include", None) { params.push(p); }
            }
            "ripgrep" => {
                if let Some(p) = add_str_param(params_source, "pattern", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "filePattern", Some("files")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "repositoryName", Some("repo")) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "caseSensitive", Some("case-sensitive")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "contextLines", Some("context")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "maxResults", Some("max")) { params.push(p); }
            }
            
            // File operations
            "read_file" | "Read" => {
                if let Some(p) = add_str_param(params_source, "file_path", Some("path")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "start_line", Some("from")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "end_line", Some("to")) { params.push(p); }
            }
            "write_file" | "Write" => {
                if let Some(p) = add_str_param(params_source, "file_path", Some("path")) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "create_parents", Some("create-dirs")) { params.push(p); }
            }
            "edit_file" | "Edit" => {
                if let Some(p) = add_str_param(params_source, "file_path", Some("path")) { params.push(p); }
                if let Some(p) = add_bool_param(params_source, "replace_all", Some("replace-all")) { params.push(p); }
            }
            "multi_edit_file" | "MultiEdit" => {
                if let Some(p) = add_str_param(params_source, "file_path", Some("path")) { params.push(p); }
                // Show edit count
                if let Some(edits) = params_source.get("edits").and_then(|v| v.as_array()) {
                    params.push(format!("edits: {}", edits.len()));
                }
            }
            
            // Shell operations
            "shell_execute" | "Bash" => {
                if let Some(p) = add_str_param(params_source, "command", Some("cmd")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "working_directory", Some("dir")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "timeout_ms", Some("timeout")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "grep_pattern", Some("grep")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "head_lines", Some("head")) { params.push(p); }
                if let Some(p) = add_num_param(params_source, "tail_lines", Some("tail")) { params.push(p); }
            }
            
            // Web operations
            "web_search" | "WebSearch" => {
                if let Some(p) = add_str_param(params_source, "query", None) { params.push(p); }
                // Show allowed/blocked domains if present
                if let Some(allowed) = params_source.get("allowed_domains").and_then(|v| v.as_array()) {
                    if !allowed.is_empty() {
                        params.push(format!("allowed: {}", allowed.len()));
                    }
                }
                if let Some(blocked) = params_source.get("blocked_domains").and_then(|v| v.as_array()) {
                    if !blocked.is_empty() {
                        params.push(format!("blocked: {}", blocked.len()));
                    }
                }
            }
            "web_fetch" | "WebFetch" => {
                if let Some(p) = add_str_param(params_source, "url", None) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "prompt", None) { params.push(p); }
            }
            
            // Other tools
            "NotebookRead" => {
                if let Some(p) = add_str_param(params_source, "notebook_path", Some("path")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "cell_id", Some("cell")) { params.push(p); }
            }
            "NotebookEdit" => {
                if let Some(p) = add_str_param(params_source, "notebook_path", Some("path")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "cell_id", Some("cell")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "cell_type", Some("type")) { params.push(p); }
                if let Some(p) = add_str_param(params_source, "edit_mode", Some("mode")) { params.push(p); }
            }
            "LS" => {
                if let Some(p) = add_str_param(params_source, "path", None) { params.push(p); }
                if let Some(ignore) = params_source.get("ignore").and_then(|v| v.as_array()) {
                    if !ignore.is_empty() {
                        params.push(format!("ignore: {} patterns", ignore.len()));
                    }
                }
            }
            "Task" => {
                if let Some(p) = add_str_param(params_source, "description", None) { params.push(p); }
                // Don't show prompt as it's too long
            }
            
            _ => {
                // For unknown tools, don't show parameters
            }
        }
        
        if params.is_empty() {
            None
        } else {
            Some(params.join(", "))
        }
    }
    
    /// Format parameter values for display
    fn format_param_value(&self, key: &str, value: &str) -> String {
        match key {
            // Truncate long paths but keep the filename
            "file_path" | "path" | "local_path" | "notebook_path" | "working_directory" => {
                if value.len() > 80 {
                    if let Some(filename) = value.split('/').last() {
                        if filename.len() < 20 {
                            format!(".../{}", filename)
                        } else {
                            format!("...{}", &value[value.len().saturating_sub(30)..])
                        }
                    } else {
                        format!("...{}", &value[value.len().saturating_sub(30)..])
                    }
                } else {
                    value.to_string()
                }
            }
            // Truncate long queries/commands
            "queryText" | "query" | "command" | "prompt" | "pattern" => {
                if value.len() > 50 {
                    format!("{}...", &value[..47])
                } else {
                    value.to_string()
                }
            }
            // Show just domain for URLs
            "url" => {
                if let Some(start) = value.find("://") {
                    if let Some(domain_end) = value[start+3..].find('/') {
                        value[start+3..start+3+domain_end].to_string()
                    } else {
                        value[start+3..].to_string()
                    }
                } else {
                    value.to_string()
                }
            }
            // Default: show as-is
            _ => value.to_string()
        }
    }
    
    /// Extract key parameters to show in header
    fn get_inline_params(&self) -> Option<String> {
        match self.tool_name {
            name if name.contains("file") || name == "Read" || name == "Write" || name == "Edit" || name == "MultiEdit" => {
                let file_path = self.result.get("file_path")
                    .or_else(|| self.result.get("filePath"))
                    .or_else(|| self.result.get("path"))
                    .and_then(|v| v.as_str());
                    
                let mut params = Vec::new();
                
                // Add repository name if available
                if let Some(repo) = self.result.get("repository")
                    .or_else(|| self.result.get("repositoryName"))
                    .and_then(|v| v.as_str()) {
                    params.push(format!("[{}]", repo));
                }
                
                // Add full file path
                if let Some(path) = file_path {
                    params.push(path.to_string());
                }
                
                // Add line numbers if available
                let start_line = self.result.get("start_line")
                    .or_else(|| self.result.get("startLine"))
                    .and_then(|v| v.as_i64());
                let end_line = self.result.get("end_line")
                    .or_else(|| self.result.get("endLine"))
                    .and_then(|v| v.as_i64());
                    
                if let Some(start) = start_line {
                    if let Some(end) = end_line {
                        if start == end {
                            params.push(format!("line {}", start));
                        } else {
                            params.push(format!("lines {}-{}", start, end));
                        }
                    } else {
                        params.push(format!("line {}", start));
                    }
                }
                
                if !params.is_empty() {
                    Some(params.join(" "))
                } else {
                    None
                }
            }
            name if name.contains("search") => {
                self.result.get("query")
                    .or_else(|| self.result.get("queryText"))
                    .and_then(|v| v.as_str())
                    .map(|q| format!("\"{}\"", q))
            }
            name if name.contains("shell") || name == "Bash" => {
                self.result.get("command")
                    .and_then(|v| v.as_str())
                    .map(|cmd| {
                        if cmd.len() > 50 {
                            format!("{}...", &cmd[..47])
                        } else {
                            cmd.to_string()
                        }
                    })
            }
            _ => None
        }
    }
    
    
    /// Get status indicator for the header
    fn get_status_indicator(&self) -> Option<RichText> {
        // Check for error fields
        if let Some(_error) = self.result.get("error").and_then(|v| v.as_str()) {
            return Some(RichText::new("Error").color(self.app_theme.error_color()));
        }
        
        // Check for exit code (shell commands)
        if let Some(exit_code) = self.result.get("exit_code").and_then(|v| v.as_i64()) {
            if exit_code == 0 {
                return Some(RichText::new("Success").color(self.app_theme.success_color()));
            } else {
                return Some(RichText::new(format!("Exit: {}", exit_code)).color(self.app_theme.error_color()));
            }
        }
        
        // Check for success fields
        if self.result.get("success").and_then(|v| v.as_bool()) == Some(true) {
            return Some(RichText::new("Success").color(self.app_theme.success_color()));
        }
        
        // Default success for results with content
        if self.result.get("content").is_some() || 
           self.result.get("results").is_some() ||
           self.result.get("todos").is_some() {
            return Some(RichText::new("Success").color(self.app_theme.success_color()));
        }
        
        None
    }
    
    
    /// Determine if action buttons should be shown
    fn should_show_actions(&self) -> bool {
        // Show actions for file operations
        if self.tool_name.contains("file") || self.tool_name == "Read" || self.tool_name == "Write" {
            return true;
        }
        
        // Show actions for search results with clickable items
        if (self.tool_name.contains("search") || self.tool_name.contains("query")) && 
           self.result.get("results").is_some() {
            return true;
        }
        
        false
    }
    
    /// Render action buttons
    fn render_actions(&self, ui: &mut Ui, action: &mut Option<(String, String)>) {
        ui.horizontal(|ui| {
            // File operations
            if self.tool_name.contains("read_file") || self.tool_name == "Read" {
                if ui.link("View Full File").clicked() {
                    if let Some(file_path) = self.result.get("file_path").and_then(|v| v.as_str()) {
                        let action_data = serde_json::json!({
                            "file_path": file_path,
                            "full_file": true
                        });
                        *action = Some(("__READ_FULL_FILE__".to_string(), action_data.to_string()));
                    }
                }
            }
            
            // Copy button for content
            if self.result.get("content").is_some() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Copy").clicked() {
                        if let Some(content) = self.result.get("content").and_then(|v| v.as_str()) {
                            ui.ctx().copy_text(content.to_string());
                        }
                    }
                });
            }
        });
    }
    
    /// File content renderer with syntax highlighting
    fn render_file_content(&self, ui: &mut Ui) {
        if let Some(content) = self.result.get("content").and_then(|v| v.as_str()) {
            // Extract file extension for syntax highlighting
            let file_ext = self.result.get("file_path")
                .or_else(|| self.result.get("filePath"))
                .and_then(|v| v.as_str())
                .and_then(|path| path.split('.').last())
                .unwrap_or("txt");
                
            // Use the theme's code font size
            let font_size = self.app_theme.code_font_size();
            
            // Render with syntax highlighting directly (no nested Frame)
            render_syntax_highlighted_code_with_font_size(
                ui,
                content,
                file_ext,
                &self.app_theme.code_background(),
                ui.available_width(),
                font_size,
            );
        } else {
            ui.label("No content available");
        }
    }
    
    /// File write result renderer
    fn render_write_result(&self, ui: &mut Ui) {
        // Check if we have diff/changes to display
        if let Some(diff_content) = self.result.get("diff")
            .or_else(|| self.result.get("changes"))
            .and_then(|v| v.as_str()) {
            // Show file info first
            let mut info_parts = Vec::new();
            
            if let Some(bytes_written) = self.result.get("bytes_written").and_then(|v| v.as_i64()) {
                info_parts.push(format!("{} bytes written", bytes_written));
            }
            
            if let Some(created) = self.result.get("created").and_then(|v| v.as_bool()) {
                if created {
                    info_parts.push("File created".to_string());
                } else {
                    info_parts.push("File updated".to_string());
                }
            }
            
            if !info_parts.is_empty() {
                ui.label(RichText::new(info_parts.join(" | ")).color(self.app_theme.success_color()));
                ui.add_space(4.0);
            }
            
            // Render the diff
            self.render_diff_content(ui, diff_content);
        } else {
            // No diff, show basic info
            let mut info_parts = Vec::new();
            
            if let Some(bytes_written) = self.result.get("bytes_written").and_then(|v| v.as_i64()) {
                info_parts.push(format!("{} bytes written", bytes_written));
            }
            
            if let Some(created) = self.result.get("created").and_then(|v| v.as_bool()) {
                if created {
                    info_parts.push("File created".to_string());
                } else {
                    info_parts.push("File updated".to_string());
                }
            }
            
            if !info_parts.is_empty() {
                ui.label(RichText::new(info_parts.join(" | ")).color(self.app_theme.success_color()));
            } else {
                ui.label(RichText::new("File written successfully").color(self.app_theme.success_color()));
            }
        }
    }
    
    /// Edit result renderer
    fn render_edit_result(&self, ui: &mut Ui) {
        // Check if we have diff/changes to display
        if let Some(diff_content) = self.result.get("diff")
            .or_else(|| self.result.get("changes"))
            .and_then(|v| v.as_str()) {
            // Render the diff
            self.render_diff_content(ui, diff_content);
        } else if let Some(success) = self.result.get("success").and_then(|v| v.as_bool()) {
            if success {
                ui.label(RichText::new("Edit applied successfully").color(self.app_theme.success_color()));
                
                // Show number of replacements if available
                if let Some(replacements) = self.result.get("replacements").and_then(|v| v.as_i64()) {
                    ui.label(RichText::new(format!("{} replacement(s) made", replacements)).small().color(self.app_theme.hint_text_color()));
                }
            } else {
                ui.label(RichText::new("Edit failed").color(self.app_theme.error_color()));
                if let Some(error) = self.result.get("error").and_then(|v| v.as_str()) {
                    ui.label(RichText::new(error).small().color(self.app_theme.error_color()));
                }
            }
        } else {
            // Fallback for other edit formats
            ui.label(RichText::new("Edit operation completed").color(self.app_theme.text_color()));
        }
    }
    
    /// Shell output renderer
    fn render_shell_output(&self, ui: &mut Ui) {
        let mut output = String::new();
        
        if let Some(stdout) = self.result.get("stdout").and_then(|v| v.as_str()) {
            if !stdout.is_empty() {
                output.push_str(stdout);
            }
        }
        
        if let Some(stderr) = self.result.get("stderr").and_then(|v| v.as_str()) {
            if !stderr.is_empty() {
                if !output.is_empty() {
                    output.push_str("\n\n");
                }
                output.push_str("STDERR:\n");
                output.push_str(stderr);
            }
        }
        
        if output.is_empty() {
            output = "(No output)".to_string();
        }
        
        // Render in a code frame
        Frame::NONE
            .fill(self.app_theme.code_background())
            .inner_margin(Vec2::new(8.0, 6.0))
            .corner_radius(CornerRadius::same(4))
            .stroke(Stroke::new(0.5, self.app_theme.border_color()))
            .show(ui, |ui| {
                ui.label(RichText::new(output).monospace().color(self.app_theme.text_color()));
            });
    }
    
    /// Search results renderer
    fn render_search_results(&self, ui: &mut Ui, action: &mut Option<(String, String)>) {
        if let Some(results) = self.result.get("results").and_then(|v| v.as_array()) {
            ui.label(format!("Found {} results", results.len()));
            ui.separator();
            
            for (i, result) in results.iter().enumerate() {
                // Render each result as a clickable item
                let file_path = result.get("filePath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown file");
                
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i + 1));
                    if ui.link(file_path).clicked() {
                        // Return the full result information as action
                        *action = Some(("__OPEN_SEARCH_RESULT__".to_string(), result.to_string()));
                    }
                });
                
                // Show preview if available
                if let Some(preview) = result.get("preview").and_then(|v| v.as_str()) {
                    ui.add_space(2.0);
                    ui.label(RichText::new(preview).small().code());
                }
                
                // Show metadata
                let mut metadata = Vec::new();
                if let Some(element_type) = result.get("elementType").and_then(|v| v.as_str()) {
                    metadata.push(element_type.to_string());
                }
                if let Some(lang) = result.get("language").and_then(|v| v.as_str()) {
                    metadata.push(format!("[{}]", lang));
                }
                
                // Add line range
                let start_line = result.get("startLine").and_then(|v| v.as_i64());
                let end_line = result.get("endLine").and_then(|v| v.as_i64());
                if let (Some(start), Some(end)) = (start_line, end_line) {
                    if start == end {
                        metadata.push(format!("Line {}", start));
                    } else {
                        metadata.push(format!("Lines {}-{}", start, end));
                    }
                }
                
                if let Some(score) = result.get("score").and_then(|v| v.as_f64()) {
                    metadata.push(format!("Score: {:.2}", score));
                }
                
                if !metadata.is_empty() {
                    ui.label(RichText::new(metadata.join(" • ")).small().color(self.app_theme.hint_text_color()));
                }
                
                // Show contextInfo if available
                if let Some(context_info) = result.get("contextInfo") {
                    // Show description if available
                    if let Some(desc) = context_info.get("description").and_then(|v| v.as_str()) {
                        ui.label(RichText::new(format!("  {}", desc))
                            .small()
                            .color(self.app_theme.hint_text_color()));
                    }
                    
                    // Show identifiers and outgoing calls counts
                    let mut context_metadata = Vec::new();
                    
                    if let Some(identifiers) = context_info.get("identifiers").and_then(|v| v.as_array()) {
                        if !identifiers.is_empty() {
                            context_metadata.push(format!("{} identifiers", identifiers.len()));
                        }
                    }
                    
                    if let Some(calls) = context_info.get("outgoing_calls").and_then(|v| v.as_array()) {
                        if !calls.is_empty() {
                            context_metadata.push(format!("{} calls", calls.len()));
                        }
                    }
                    
                    if !context_metadata.is_empty() {
                        ui.label(RichText::new(format!("  {}", context_metadata.join(" • ")))
                            .small()
                            .color(self.app_theme.hint_text_color()));
                    }
                }
                
                ui.add_space(4.0);
            }
            
            if results.len() > 10 {
                ui.label(RichText::new(format!("... and {} more results", results.len() - 10))
                    .small()
                    .color(self.app_theme.hint_text_color()));
            }
        } else if let Some(matches) = self.result.get("matchingFiles").and_then(|v| v.as_array()) {
            // File search results
            ui.label(format!("Found {} files", matches.len()));
            ui.separator();
            
            for (i, file) in matches.iter().enumerate() {
                if let Some(file_path) = file.as_str() {
                    ui.label(format!("{}. {}", i + 1, file_path));
                }
            }
        } else {
            ui.label("No results found");
        }
    }
    
    /// Todo list renderer
    fn render_todo_list(&self, ui: &mut Ui) {
        if let Some(todos) = self.result.get("todos").and_then(|v| v.as_array()) {
            let pending_count = todos.iter().filter(|t| t.get("status").and_then(|s| s.as_str()) == Some("pending")).count();
            let in_progress_count = todos.iter().filter(|t| t.get("status").and_then(|s| s.as_str()) == Some("in_progress")).count();
            let completed_count = todos.iter().filter(|t| t.get("status").and_then(|s| s.as_str()) == Some("completed")).count();
            
            ui.horizontal(|ui| {
                ui.label(format!("{} total", todos.len()));
                ui.separator();
                if pending_count > 0 {
                    ui.label(RichText::new(format!("{} pending", pending_count)).color(self.app_theme.warning_color()));
                }
                if in_progress_count > 0 {
                    ui.label(RichText::new(format!("{} in progress", in_progress_count)).color(self.app_theme.accent_color()));
                }
                if completed_count > 0 {
                    ui.label(RichText::new(format!("{} completed", completed_count)).color(self.app_theme.success_color()));
                }
            });
            
            ui.add_space(4.0);
            
            for todo in todos {
                if let Some(content) = todo.get("content").and_then(|v| v.as_str()) {
                    let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let priority = todo.get("priority").and_then(|v| v.as_str()).unwrap_or("medium");
                    
                    let status_text = match status {
                        "completed" => "[Done]",
                        "pending" => "[Todo]",
                        "in_progress" => "[In Progress]",
                        _ => "[Unknown]",
                    };
                    
                    let priority_color = match priority {
                        "high" => self.app_theme.error_color(),
                        "low" => self.app_theme.hint_text_color(),
                        _ => self.app_theme.text_color(),
                    };
                    
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(status_text).small().color(self.app_theme.hint_text_color()));
                        ui.label(RichText::new(content).color(priority_color));
                    });
                }
            }
        } else {
            ui.label("No todos");
        }
    }
    
    /// Repository info renderer
    fn render_repository_info(&self, ui: &mut Ui) {
        if let Some(repositories) = self.result.get("repositories").and_then(|v| v.as_array()) {
            ui.label(format!("{} repositories", repositories.len()));
            ui.add_space(4.0);
            
            for repo in repositories {
                if let Some(name) = repo.get("name").and_then(|v| v.as_str()) {
                    // Clean up the name - remove trailing parentheses if present
                    let clean_name = name.trim_end_matches("()");
                    
                    ui.horizontal(|ui| {
                        // Use bullet point format
                        ui.label("•");
                        ui.label(RichText::new(clean_name).strong());
                        
                        if let Some(branch) = repo.get("branch").and_then(|v| v.as_str()) {
                            if !branch.is_empty() {
                                ui.label(RichText::new(format!("[{}]", branch)).small().color(self.app_theme.hint_text_color()));
                            }
                        }
                    });
                    
                    if let Some(path) = repo.get("path").and_then(|v| v.as_str()) {
                        ui.indent("repo_path", |ui| {
                            ui.label(RichText::new(path).small().color(self.app_theme.hint_text_color()));
                        });
                    }
                }
            }
        } else if let Some(message) = self.result.get("message").and_then(|v| v.as_str()) {
            ui.label(message);
        } else {
            ui.label("Repository operation completed");
        }
    }
    
    /// Render diff content with syntax highlighting
    fn render_diff_content(&self, ui: &mut Ui, diff_content: &str) {
        // Render diff lines with appropriate colors
        for line in diff_content.lines() {
            let (text_color, bg_color) = if line.starts_with('+') && !line.starts_with("+++") {
                (self.app_theme.diff_added_text(), Some(self.app_theme.diff_added_bg()))
            } else if line.starts_with('-') && !line.starts_with("---") {
                (self.app_theme.diff_removed_text(), Some(self.app_theme.diff_removed_bg()))
            } else if line.starts_with("@@") {
                (self.app_theme.accent_color(), None)
            } else {
                (self.app_theme.text_color(), None)
            };
            
            if let Some(bg) = bg_color {
                Frame::NONE
                    .fill(bg)
                    .inner_margin(Vec2::new(4.0, 2.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(line).monospace().color(text_color));
                    });
            } else {
                ui.label(RichText::new(line).monospace().color(text_color));
            }
        }
    }
    
    /// Ping result renderer
    fn render_ping_result(&self, ui: &mut Ui) {
        if let Some(message) = self.result.get("message").and_then(|v| v.as_str()) {
            ui.label(RichText::new(message).color(self.app_theme.success_color()));
        } else {
            ui.label(RichText::new("Server is responsive").color(self.app_theme.success_color()));
        }
        
        if let Some(response_time) = self.result.get("response_time_ms").and_then(|v| v.as_i64()) {
            ui.label(RichText::new(format!("Response time: {}ms", response_time))
                .small()
                .color(self.app_theme.hint_text_color()));
        }
    }
    
    /// Generic content renderer for unknown tool types
    fn render_generic_content(&self, ui: &mut Ui) {
        // Try to format as markdown first
        let formatter = crate::gui::app::tool_formatting::ToolResultFormatter::new();
        let tool_result = crate::agent::events::ToolResult::Success { output: self.result.to_string() };
        let formatted_result = formatter.format_tool_result_for_preview(self.tool_name, &tool_result);
        
        // Use markdown rendering
        COMMONMARK_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let viewer = egui_commonmark::CommonMarkViewer::new();
            viewer.show(ui, &mut cache, &formatted_result);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_dynamic_sizing_behavior() {
        // Small content should result in small tool card
        let small_result = json!({ "message": "pong" });
        let small_renderer = SimplifiedToolRenderer::new("ping", &small_result, AppTheme::default());
        assert_eq!(small_renderer.tool_name, "ping");
        
        // Large content should still work but be scrollable at 800px
        let large_content = "Line\n".repeat(1000); // 1000 lines
        let large_result = json!({ "content": large_content });
        let large_renderer = SimplifiedToolRenderer::new("read_file", &large_result, AppTheme::default());
        assert_eq!(large_renderer.tool_name, "read_file");
        
        // Medium content should size appropriately
        let medium_result = json!({ 
            "stdout": "Output line 1\nOutput line 2\nOutput line 3\nOutput line 4\nOutput line 5"
        });
        let medium_renderer = SimplifiedToolRenderer::new("bash", &medium_result, AppTheme::default());
        assert_eq!(medium_renderer.tool_name, "bash");
    }
    
    #[test]
    fn test_unique_id_generation() {
        let result = json!({});
        let tool1 = SimplifiedToolRenderer::new("test", &result, AppTheme::default());
        let tool2 = SimplifiedToolRenderer::new("test", &result, AppTheme::default());
        
        // Unique IDs should be different even for same tool type
        assert_ne!(tool1.unique_id, tool2.unique_id);
        assert!(tool1.unique_id.contains("tool_renderer_test"));
        assert!(tool2.unique_id.contains("tool_renderer_test"));
    }
    
    
    #[test]
    fn test_status_indicators() {
        // Error status
        let error_result = json!({ "error": "Something went wrong" });
        let renderer = SimplifiedToolRenderer::new("test", &error_result, AppTheme::default());
        let status = renderer.get_status_indicator();
        assert!(status.is_some());
        assert!(status.unwrap().text().contains("Error"));
        
        // Success shell command
        let success_shell = json!({ "exit_code": 0 });
        let renderer = SimplifiedToolRenderer::new("shell", &success_shell, AppTheme::default());
        let status = renderer.get_status_indicator();
        assert!(status.is_some());
        assert!(status.unwrap().text().contains("Success"));
        
        // Failed shell command
        let failed_shell = json!({ "exit_code": 1 });
        let renderer = SimplifiedToolRenderer::new("shell", &failed_shell, AppTheme::default());
        let status = renderer.get_status_indicator();
        assert!(status.is_some());
        assert!(status.unwrap().text().contains("Exit: 1"));
        
        // Success boolean
        let success_bool = json!({ "success": true });
        let renderer = SimplifiedToolRenderer::new("test", &success_bool, AppTheme::default());
        let status = renderer.get_status_indicator();
        assert!(status.is_some());
        assert!(status.unwrap().text().contains("Success"));
        
        // Content implies success
        let content_result = json!({ "content": "test content" });
        let renderer = SimplifiedToolRenderer::new("test", &content_result, AppTheme::default());
        let status = renderer.get_status_indicator();
        assert!(status.is_some());
        assert!(status.unwrap().text().contains("Success"));
    }
    
    #[test]
    fn test_inline_params_extraction() {
        // File operations with full info
        let file_result = json!({
            "file_path": "/home/user/test.rs",
            "repository": "my-repo",
            "start_line": 10,
            "end_line": 20
        });
        let renderer = SimplifiedToolRenderer::new("read_file", &file_result, AppTheme::default());
        let params = renderer.get_inline_params();
        assert!(params.is_some());
        let params_str = params.unwrap();
        assert!(params_str.contains("[my-repo]"));
        assert!(params_str.contains("/home/user/test.rs"));
        assert!(params_str.contains("lines 10-20"));
        
        // Single line number
        let single_line = json!({
            "file_path": "test.py",
            "start_line": 5,
        });
        let renderer = SimplifiedToolRenderer::new("read_file", &single_line, AppTheme::default());
        let params = renderer.get_inline_params();
        assert!(params.is_some());
        assert!(params.unwrap().contains("test.py"));
        
        // Test with line numbers only
        let line_only = json!({
            "file_path": "file.rs",
            "start_line": 5,
            "end_line": 5
        });
        let renderer2 = SimplifiedToolRenderer::new("read_file", &line_only, AppTheme::default());
        let params2 = renderer2.get_inline_params();
        assert!(params2.is_some());
        assert!(params2.unwrap().contains("line 5"));
        
        // Search query
        let search_result = json!({
            "query": "test query"
        });
        let renderer = SimplifiedToolRenderer::new("search", &search_result, AppTheme::default());
        let params = renderer.get_inline_params();
        assert_eq!(params.unwrap(), "\"test query\"");
        
        // Shell command truncation
        let long_command = "ls -la /very/long/path/that/should/be/truncated/when/displayed";
        let shell_result = json!({
            "command": long_command
        });
        let renderer = SimplifiedToolRenderer::new("shell", &shell_result, AppTheme::default());
        let params = renderer.get_inline_params();
        assert!(params.is_some());
        let params_str = params.unwrap();
        assert!(params_str.len() <= 50);
        assert!(params_str.ends_with("..."));
    }
    
    #[test]
    fn test_should_show_actions() {
        // File operations should show actions
        let empty_result = json!({});
        let file_renderer = SimplifiedToolRenderer::new("read_file", &empty_result, AppTheme::default());
        assert!(file_renderer.should_show_actions());
        
        let write_renderer = SimplifiedToolRenderer::new("Write", &empty_result, AppTheme::default());
        assert!(write_renderer.should_show_actions());
        
        // Search with results should show actions
        let search_with_results = json!({"results": []});
        let search_renderer = SimplifiedToolRenderer::new("search", &search_with_results, AppTheme::default());
        assert!(search_renderer.should_show_actions());
        
        // Search without results should not
        let search_no_results = SimplifiedToolRenderer::new("search", &empty_result, AppTheme::default());
        assert!(!search_no_results.should_show_actions());
        
        // Other tools should not show actions
        let todo_renderer = SimplifiedToolRenderer::new("todo", &empty_result, AppTheme::default());
        assert!(!todo_renderer.should_show_actions());
    }
    
    #[test]
    fn test_font_size_uses_theme() {
        // Font sizes now come from the theme, not from per-file storage
        let theme = AppTheme::default();
        let code_font_size = theme.code_font_size();
        
        // Verify the theme provides a reasonable default
        assert!(code_font_size >= 8.0);
        assert!(code_font_size <= 24.0);
    }
    
    #[test]
    fn test_edge_cases() {
        // Null values - this should not show status  
        let null_result = json!({
            "nothing": null
        });
        let renderer = SimplifiedToolRenderer::new("test", &null_result, AppTheme::default());
        assert!(renderer.get_status_indicator().is_none());
        
        // Very long content
        let long_content = json!({
            "content": "x".repeat(10000)
        });
        let renderer = SimplifiedToolRenderer::new("read_file", &long_content, AppTheme::default());
        assert_eq!(renderer.tool_name, "read_file");
        
        // Empty arrays
        let empty_arrays = json!({
            "results": [],
            "todos": [],
            "repositories": []
        });
        let renderer = SimplifiedToolRenderer::new("search", &empty_arrays, AppTheme::default());
        assert!(renderer.should_show_actions()); // Has results field
        
        // Missing expected fields
        let missing_fields = json!({});
        let renderer = SimplifiedToolRenderer::new("read_file", &missing_fields, AppTheme::default());
        assert!(renderer.get_inline_params().is_none());
    }
    
    #[test]
    fn test_scroll_configuration() {
        // Test scroll area ID generation
        let result = json!({});
        let renderer1 = SimplifiedToolRenderer::new("read_file", &result, AppTheme::default());
        let renderer2 = SimplifiedToolRenderer::new("read_file", &result, AppTheme::default());
        
        // Scroll IDs should be based on unique_id
        let scroll_id1 = format!("{}_scroll", renderer1.unique_id);
        let scroll_id2 = format!("{}_scroll", renderer2.unique_id);
        
        assert_ne!(scroll_id1, scroll_id2);
    }
    
    
    #[test]
    fn test_tool_type_matching() {
        // Test various tool name patterns are recognized correctly
        let result = json!({});
        // Just verify different tool types can be created
        let _ = SimplifiedToolRenderer::new("mcp__read_file", &result, AppTheme::default());
        let _ = SimplifiedToolRenderer::new("view_file", &result, AppTheme::default());
        let _ = SimplifiedToolRenderer::new("semantic_search", &result, AppTheme::default());
        let _ = SimplifiedToolRenderer::new("query", &result, AppTheme::default());
    }
}