// Simplified tool renderer that removes collapsing headers and uses fixed-height previews

use egui::{Frame, ScrollArea, Ui, RichText, Vec2, Color32, Stroke, CornerRadius, Layout, Align};
use serde_json::Value;
use crate::gui::theme::AppTheme;
use crate::gui::chat::tool_mappings::{get_human_friendly_tool_name, get_tool_icon};
use crate::gui::chat::syntax_highlighting::{render_syntax_highlighted_code, render_syntax_highlighted_code_with_font_size};
use crate::gui::chat::view::COMMONMARK_CACHE;
use std::collections::HashMap;
use std::cell::RefCell;

thread_local! {
    static FILE_READ_FONT_SIZES: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
}

pub struct SimplifiedToolRenderer<'a> {
    tool_name: &'a str,
    result: &'a serde_json::Value,
    app_theme: AppTheme,
}

impl<'a> SimplifiedToolRenderer<'a> {
    const MAX_HEIGHT: f32 = 400.0;
    const MAX_LINES: usize = 15;
    const CONTENT_PADDING: f32 = 8.0;
    const HEADER_HEIGHT: f32 = 32.0;
    const ACTION_BAR_HEIGHT: f32 = 24.0;
    
    pub fn new(
        tool_name: &'a str,
        result: &'a serde_json::Value,
        app_theme: AppTheme,
    ) -> Self {
        Self { tool_name, result, app_theme }
    }
    
    /// Main entry point - renders the complete tool result
    pub fn render(self, ui: &mut egui::Ui) -> Option<(String, String)> {
        let mut action = None;
        
        let app_theme = self.app_theme;
        
        // Outer frame for the entire tool result
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
            // Tool icon and name
            let icon = get_tool_icon(self.tool_name);
            let name = get_human_friendly_tool_name(self.tool_name);
            ui.label(RichText::new(format!("{} {}", icon, name)).strong());
            
            // Add key parameters inline (e.g., filename for file operations)
            if let Some(params) = self.get_inline_params() {
                ui.separator();
                ui.label(RichText::new(params).small().color(self.app_theme.hint_text_color()));
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
        let content_height = Self::MAX_HEIGHT - Self::HEADER_HEIGHT - Self::ACTION_BAR_HEIGHT;
        
        ScrollArea::vertical()
            .max_height(content_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
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
    }
    
    /// Extract key parameters to show in header
    fn get_inline_params(&self) -> Option<String> {
        match self.tool_name {
            name if name.contains("file") || name == "Read" || name == "Write" || name == "Edit" || name == "MultiEdit" => {
                self.result.get("file_path")
                    .or_else(|| self.result.get("filePath"))
                    .or_else(|| self.result.get("path"))
                    .and_then(|v| v.as_str())
                    .map(|path| {
                        // Show just filename for long paths
                        if path.len() > 40 {
                            path.split('/').last().unwrap_or(path).to_string()
                        } else {
                            path.to_string()
                        }
                    })
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
        if let Some(error) = self.result.get("error").and_then(|v| v.as_str()) {
            return Some(RichText::new("❌").color(self.app_theme.error_color()));
        }
        
        // Check for exit code (shell commands)
        if let Some(exit_code) = self.result.get("exit_code").and_then(|v| v.as_i64()) {
            if exit_code == 0 {
                return Some(RichText::new("✓").color(self.app_theme.success_color()));
            } else {
                return Some(RichText::new(format!("Exit: {}", exit_code)).color(self.app_theme.error_color()));
            }
        }
        
        // Check for success fields
        if self.result.get("success").and_then(|v| v.as_bool()) == Some(true) {
            return Some(RichText::new("✓").color(self.app_theme.success_color()));
        }
        
        // Default success for results with content
        if self.result.get("content").is_some() || 
           self.result.get("results").is_some() ||
           self.result.get("todos").is_some() {
            return Some(RichText::new("✓").color(self.app_theme.success_color()));
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
                    if ui.button("📋 Copy").clicked() {
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
                
            // Apply line limit
            let lines: Vec<&str> = content.lines().take(Self::MAX_LINES).collect();
            let truncated_content = lines.join("\n");
            let was_truncated = content.lines().count() > Self::MAX_LINES;
            
            // Get font size for this file
            let file_path_key = self.result.get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            
            let font_size = FILE_READ_FONT_SIZES.with(|sizes| {
                *sizes.borrow().get(&file_path_key).unwrap_or(&12.0)
            });
            
            // Font size controls
            ui.horizontal(|ui| {
                ui.label("Font size:");
                if ui.button("🔍-").clicked() {
                    let new_size = (font_size - 2.0).max(8.0);
                    FILE_READ_FONT_SIZES.with(|sizes| {
                        sizes.borrow_mut().insert(file_path_key.clone(), new_size);
                    });
                }
                ui.label(format!("{}pt", font_size as i32));
                if ui.button("🔍+").clicked() {
                    let new_size = (font_size + 2.0).min(24.0);
                    FILE_READ_FONT_SIZES.with(|sizes| {
                        sizes.borrow_mut().insert(file_path_key.clone(), new_size);
                    });
                }
            });
            
            ui.add_space(4.0);
            
            // Render with syntax highlighting
            render_syntax_highlighted_code_with_font_size(
                ui,
                &truncated_content,
                file_ext,
                &self.app_theme.code_background(),
                ui.available_width(),
                font_size,
            );
            
            if was_truncated {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("... {} more lines", 
                    content.lines().count() - Self::MAX_LINES))
                    .small()
                    .color(self.app_theme.hint_text_color()));
            }
        } else {
            ui.label("No content available");
        }
    }
    
    /// File write result renderer
    fn render_write_result(&self, ui: &mut Ui) {
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
    
    /// Edit result renderer
    fn render_edit_result(&self, ui: &mut Ui) {
        if let Some(success) = self.result.get("success").and_then(|v| v.as_bool()) {
            if success {
                ui.label(RichText::new("✓ Edit applied successfully").color(self.app_theme.success_color()));
                
                // Show number of replacements if available
                if let Some(replacements) = self.result.get("replacements").and_then(|v| v.as_i64()) {
                    ui.label(RichText::new(format!("{} replacement(s) made", replacements)).small().color(self.app_theme.hint_text_color()));
                }
            } else {
                ui.label(RichText::new("❌ Edit failed").color(self.app_theme.error_color()));
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
            
            for (i, result) in results.iter().take(10).enumerate() {
                // Render each result as a clickable item
                let file_path = result.get("filePath")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown file");
                
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i + 1));
                    if ui.link(file_path).clicked() {
                        // Return file path and line number as action
                        let mut action_data = serde_json::json!({
                            "file_path": file_path
                        });
                        
                        if let Some(line) = result.get("startLine").and_then(|v| v.as_i64()) {
                            action_data["start_line"] = serde_json::json!(line);
                        }
                        
                        *action = Some(("__OPEN_FILE__".to_string(), action_data.to_string()));
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
                if let Some(score) = result.get("score").and_then(|v| v.as_f64()) {
                    metadata.push(format!("Score: {:.2}", score));
                }
                
                if !metadata.is_empty() {
                    ui.label(RichText::new(metadata.join(" • ")).small().color(self.app_theme.hint_text_color()));
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
            
            for (i, file) in matches.iter().take(20).enumerate() {
                if let Some(file_path) = file.as_str() {
                    ui.label(format!("{}. {}", i + 1, file_path));
                }
            }
            
            if matches.len() > 20 {
                ui.label(RichText::new(format!("... and {} more files", matches.len() - 20))
                    .small()
                    .color(self.app_theme.hint_text_color()));
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
                    
                    let status_icon = match status {
                        "completed" => "✓",
                        "pending" => "◯",
                        "in_progress" => "⏳",
                        _ => "?",
                    };
                    
                    let priority_color = match priority {
                        "high" => self.app_theme.error_color(),
                        "low" => self.app_theme.hint_text_color(),
                        _ => self.app_theme.text_color(),
                    };
                    
                    ui.horizontal(|ui| {
                        ui.label(status_icon);
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
                    ui.horizontal(|ui| {
                        ui.label("•");
                        ui.label(RichText::new(name).strong());
                        
                        if let Some(branch) = repo.get("branch").and_then(|v| v.as_str()) {
                            ui.label(RichText::new(format!("({})", branch)).small().color(self.app_theme.hint_text_color()));
                        }
                    });
                    
                    if let Some(path) = repo.get("path").and_then(|v| v.as_str()) {
                        ui.label(RichText::new(path).small().color(self.app_theme.hint_text_color()));
                    }
                }
            }
        } else if let Some(message) = self.result.get("message").and_then(|v| v.as_str()) {
            ui.label(message);
        } else {
            ui.label("Repository operation completed");
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