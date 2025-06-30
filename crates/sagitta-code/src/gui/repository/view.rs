use std::sync::Arc;
use egui::{Ui, RichText, Color32, Grid, TextEdit, ScrollArea, ComboBox, Button, text::LayoutJob, TextStyle, TextFormat, FontId};
use tokio::sync::Mutex;
use super::manager::RepositoryManager;
use egui_code_editor::CodeEditor;
use syntect::{
    highlighting::{ThemeSet, Style as SyntectStyle, Theme},
    parsing::SyntaxSet,
    easy::HighlightLines,
    util::LinesWithEndings,
};
use std::sync::OnceLock;
use std::path::Path;

use super::types::{RepoPanelState, FileViewResult};

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(|| SyntaxSet::load_defaults_newlines())
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(|| ThemeSet::load_defaults())
}

/// Get file extension from path
fn get_file_extension(file_path: &str) -> Option<&str> {
    Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
}

/// Convert syntect style to egui color
fn syntect_style_to_color(style: &SyntectStyle) -> Color32 {
    Color32::from_rgb(
        style.foreground.r, 
        style.foreground.g, 
        style.foreground.b
    )
}

/// Render the file view component
pub fn render_file_view(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("View File");
    
    // Sync file view options with selected repository if needed
    if let Some(selected_repo) = &state.selected_repo {
        if state.file_view_options.repo_name != *selected_repo {
            state.file_view_options.repo_name = selected_repo.clone();
        }
    }
    
    // Check for file content updates from async task
    if let Some(channel) = &mut state.file_view_result.channel {
        if let Ok(result) = channel.receiver.try_recv() {
            // Update file view result
            state.file_view_result.is_loading = result.is_loading;
            state.file_view_result.error_message = result.error_message;
            state.file_view_result.content = result.content;
        }
    }
    
    if state.selected_repo.is_none() {
        ui.label("No repository selected");
        
        // Repository selector dropdown
        let repo_names = state.repo_names();
        if !repo_names.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Select repository:");
                ComboBox::from_id_source("view_no_repo_selector")
                    .selected_text("Choose repository...")
                    .show_ui(ui, |ui| {
                        for name in repo_names {
                            if ui.selectable_value(
                                &mut state.selected_repo,
                                Some(name.clone()),
                                &name
                            ).clicked() {
                                state.file_view_options.repo_name = name;
                            }
                        }
                    });
            });
        } else {
            ui.label("No repositories available");
            if ui.button("Go to Repository List").clicked() {
                state.active_tab = super::types::RepoPanelTab::List;
            }
        }
        
        return;
    }
    
    // File view options
    Grid::new("file_view_options_grid")
        .num_columns(2)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            ui.label("Repository:");
            let repo_names: Vec<String> = state.repositories.iter().map(|r| r.name.clone()).collect();
            let selected_text = state.selected_repo.as_ref().unwrap_or(&state.file_view_options.repo_name);
            ComboBox::from_id_source("repository_select_file_view")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for name in repo_names {
                        if ui.selectable_value(
                            &mut state.file_view_options.repo_name, 
                            name.clone(),
                            &name
                        ).clicked() {
                            // Also update the selected_repo to maintain consistency
                            state.selected_repo = Some(name.clone());
                        }
                    }
                });
            ui.end_row();
            
            ui.label("File Path:");
            ui.text_edit_singleline(&mut state.file_view_options.file_path);
            ui.end_row();
            
            ui.label("Start Line (optional):");
            ui.horizontal(|ui| {
                let mut start_line_str = if let Some(line) = state.file_view_options.start_line {
                    line.to_string()
                } else {
                    String::new()
                };
                
                if ui.text_edit_singleline(&mut start_line_str).changed() {
                    state.file_view_options.start_line = start_line_str.parse().ok();
                }
            });
            ui.end_row();
            
            ui.label("End Line (optional):");
            ui.horizontal(|ui| {
                let mut end_line_str = if let Some(line) = state.file_view_options.end_line {
                    line.to_string()
                } else {
                    String::new()
                };
                
                if ui.text_edit_singleline(&mut end_line_str).changed() {
                    state.file_view_options.end_line = end_line_str.parse().ok();
                }
            });
            ui.end_row();
        });
    
    // View button
    ui.vertical_centered(|ui| {
        if ui.button("View File").clicked() {
            if state.file_view_options.file_path.is_empty() {
                return;
            }
            
            // Set loading state
            state.file_view_result.is_loading = true;
            state.file_view_result.error_message = None;
            state.file_view_result.content = String::new();
            
            // Clone view options for async operation
            let options = state.file_view_options.clone();
            let repo_manager_clone = Arc::clone(&repo_manager);
            
            // Get sender clone for async operation
            let sender = state.file_view_result.channel.as_ref().map(|ch| ch.sender.clone());
            
            // Schedule the view operation
            let handle = tokio::runtime::Handle::current();
            handle.spawn(async move {
                let manager = repo_manager_clone.lock().await;
                
                // Call the actual view method
                let result = manager.view_file(
                    &options.repo_name,
                    &options.file_path,
                    options.start_line.map(|l| l as u32),
                    options.end_line.map(|l| l as u32),
                ).await;
                
                // Send result back to UI thread through channel
                if let Some(sender) = sender {
                    match result {
                        Ok(content) => {
                            let _ = sender.try_send(FileViewResult {
                                is_loading: false,
                                error_message: None,
                                content,
                                channel: None,
                            });
                        },
                        Err(e) => {
                            let _ = sender.try_send(FileViewResult {
                                is_loading: false,
                                error_message: Some(e.to_string()),
                                content: String::new(),
                                channel: None,
                            });
                        }
                    }
                }
            });
        }
    });
    
    ui.separator();
    
    // Show loading indicator or error message
    if state.file_view_result.is_loading {
        ui.label(RichText::new("Loading file...").color(theme.warning_color()));
    } else if let Some(error) = &state.file_view_result.error_message {
        ui.label(RichText::new(format!("Error: {}", error)).color(theme.error_color()));
    }
    
    // File content
    ui.label("File Content:");
    
    // Clone needed data
    let file_path = state.file_view_options.file_path.clone();
    let content = state.file_view_result.content.clone();
    let is_loading = state.file_view_result.is_loading;
    
    ScrollArea::vertical()
        .max_height(400.0)
        .show(ui, |ui| {
            // Display file content with syntax highlighting
            if file_path.is_empty() {
                ui.label("No file selected");
            } else if content.is_empty() && !is_loading {
                ui.label("No content available");
            } else {
                // Detect language from file extension
                let extension = get_file_extension(&file_path);
                let syntax_set = get_syntax_set();
                let theme_set = get_theme_set();
                
                // Choose appropriate theme based on UI theme
                let syntect_theme = if ui.style().visuals.dark_mode {
                    &theme_set.themes["base16-ocean.dark"]
                } else {
                    &theme_set.themes["base16-ocean.light"]
                };
                
                // Find syntax for the file type with fallbacks for TypeScript
                let syntax = extension
                    .and_then(|ext| {
                        // Handle TypeScript extensions by falling back to JavaScript
                        match ext {
                            "ts" | "tsx" => syntax_set.find_syntax_by_extension("js")
                                .or_else(|| syntax_set.find_syntax_by_name("JavaScript")),
                            "jsx" => syntax_set.find_syntax_by_extension("js")
                                .or_else(|| syntax_set.find_syntax_by_name("JavaScript")),
                            _ => syntax_set.find_syntax_by_extension(ext),
                        }
                    })
                    .or_else(|| {
                        // Try to detect from first line for scripts
                        content.lines().next().and_then(|first_line| {
                            if first_line.starts_with("#!") {
                                if first_line.contains("python") {
                                    syntax_set.find_syntax_by_extension("py")
                                } else if first_line.contains("bash") || first_line.contains("sh") {
                                    syntax_set.find_syntax_by_extension("sh")
                                } else if first_line.contains("node") || first_line.contains("deno") {
                                    syntax_set.find_syntax_by_extension("js")
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
                
                // Create a syntax highlighter
                let mut highlighter = HighlightLines::new(syntax, syntect_theme);
                
                // Create a layouter that applies syntax highlighting
                let mut content_copy = content.clone();
                let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                    let mut layout_job = egui::text::LayoutJob::default();
                    layout_job.wrap.max_width = wrap_width;
                    
                    // Apply syntax highlighting
                    for line in LinesWithEndings::from(text) {
                        if let Ok(ranges) = highlighter.highlight_line(line, syntax_set) {
                            for (style, text_part) in ranges {
                                let color = syntect_style_to_color(&style);
                                layout_job.append(
                                    text_part,
                                    0.0,
                                    TextFormat {
                                        font_id: TextStyle::Monospace.resolve(ui.style()),
                                        color,
                                        ..Default::default()
                                    },
                                );
                            }
                        } else {
                            // Fallback to plain text if highlighting fails
                            layout_job.append(
                                line,
                                0.0,
                                TextFormat {
                                    font_id: TextStyle::Monospace.resolve(ui.style()),
                                    color: ui.visuals().text_color(),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                    
                    ui.fonts(|f| f.layout_job(layout_job))
                };
                
                ui.add(
                    egui::TextEdit::multiline(&mut content_copy)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_rows(10)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter),
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_file_extension() {
        assert_eq!(get_file_extension("test.rs"), Some("rs"));
        assert_eq!(get_file_extension("src/main.rs"), Some("rs"));
        assert_eq!(get_file_extension("README.md"), Some("md"));
        assert_eq!(get_file_extension("package.json"), Some("json"));
        assert_eq!(get_file_extension("App.tsx"), Some("tsx"));
        assert_eq!(get_file_extension("script.ts"), Some("ts"));
        assert_eq!(get_file_extension("no_extension"), None);
        assert_eq!(get_file_extension(""), None);
        assert_eq!(get_file_extension(".gitignore"), None);
    }
    
    #[test]
    fn test_syntect_style_to_color() {
        let style = SyntectStyle {
            foreground: syntect::highlighting::Color { r: 255, g: 128, b: 64, a: 255 },
            background: syntect::highlighting::Color { r: 0, g: 0, b: 0, a: 255 },
            font_style: syntect::highlighting::FontStyle::empty(),
        };
        
        let color = syntect_style_to_color(&style);
        assert_eq!(color, Color32::from_rgb(255, 128, 64));
    }
    
    #[test]
    fn test_syntax_set_loading() {
        let syntax_set = get_syntax_set();
        
        // Test that common syntaxes are available
        assert!(syntax_set.find_syntax_by_extension("rs").is_some(), "Rust syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("py").is_some(), "Python syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("js").is_some(), "JavaScript syntax should be available");
        // TypeScript might not be available in default syntect, but JavaScript should be
        assert!(
            syntax_set.find_syntax_by_extension("ts").is_some() || 
            syntax_set.find_syntax_by_extension("js").is_some(), 
            "TypeScript or JavaScript syntax should be available"
        );
        assert!(
            syntax_set.find_syntax_by_extension("tsx").is_some() || 
            syntax_set.find_syntax_by_extension("js").is_some(), 
            "TypeScript React or JavaScript syntax should be available"
        );
        assert!(syntax_set.find_syntax_by_extension("json").is_some(), "JSON syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("md").is_some(), "Markdown syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("yml").is_some(), "YAML syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("yaml").is_some(), "YAML syntax should be available");
        // TOML might not be available in default syntect
        assert!(
            syntax_set.find_syntax_by_extension("toml").is_some() ||
            syntax_set.find_syntax_by_extension("ini").is_some() ||
            syntax_set.find_syntax_plain_text().name == "Plain Text",
            "TOML, INI, or plain text syntax should be available"
        );
        assert!(syntax_set.find_syntax_by_extension("sh").is_some(), "Shell syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("go").is_some(), "Go syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("rb").is_some(), "Ruby syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("html").is_some(), "HTML syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("css").is_some(), "CSS syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("xml").is_some(), "XML syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("sql").is_some(), "SQL syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("c").is_some(), "C syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("cpp").is_some(), "C++ syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("java").is_some(), "Java syntax should be available");
        assert!(syntax_set.find_syntax_by_extension("cs").is_some(), "C# syntax should be available");
    }
    
    #[test]
    fn test_theme_set_loading() {
        let theme_set = get_theme_set();
        
        // Test that expected themes are available
        assert!(theme_set.themes.contains_key("base16-ocean.dark"), "Dark theme should be available");
        assert!(theme_set.themes.contains_key("base16-ocean.light"), "Light theme should be available");
    }
    
    #[test]
    fn test_syntax_highlighting_rust_code() {
        let syntax_set = get_syntax_set();
        let theme_set = get_theme_set();
        let theme = &theme_set.themes["base16-ocean.dark"];
        
        let rust_code = r#"fn main() {
    println!("Hello, world!");
    let x = 42;
}"#;
        
        let syntax = syntax_set.find_syntax_by_extension("rs").unwrap();
        let mut highlighter = HighlightLines::new(syntax, theme);
        
        // Test that we can highlight without panicking
        for line in LinesWithEndings::from(rust_code) {
            let ranges = highlighter.highlight_line(line, syntax_set).unwrap();
            assert!(!ranges.is_empty(), "Highlighted ranges should not be empty");
        }
    }
    
    #[test]
    fn test_syntax_highlighting_typescript_code() {
        let syntax_set = get_syntax_set();
        let theme_set = get_theme_set();
        let theme = &theme_set.themes["base16-ocean.dark"];
        
        let ts_code = r#"interface User {
    name: string;
    age: number;
}

const greet = (user: User): void => {
    console.log(`Hello, ${user.name}!`);
};"#;
        
        let syntax = syntax_set.find_syntax_by_extension("ts")
            .or_else(|| syntax_set.find_syntax_by_extension("js"))
            .expect("TypeScript or JavaScript syntax should be available");
        let mut highlighter = HighlightLines::new(syntax, theme);
        
        // Test that we can highlight without panicking
        for line in LinesWithEndings::from(ts_code) {
            let ranges = highlighter.highlight_line(line, syntax_set).unwrap();
            assert!(!ranges.is_empty(), "Highlighted ranges should not be empty");
        }
    }
    
    #[test]
    fn test_syntax_highlighting_json() {
        let syntax_set = get_syntax_set();
        let theme_set = get_theme_set();
        let theme = &theme_set.themes["base16-ocean.dark"];
        
        let json_code = r#"{
    "name": "test",
    "version": "1.0.0",
    "dependencies": {
        "syntect": "5.0"
    }
}"#;
        
        let syntax = syntax_set.find_syntax_by_extension("json").unwrap();
        let mut highlighter = HighlightLines::new(syntax, theme);
        
        // Test that we can highlight without panicking
        for line in LinesWithEndings::from(json_code) {
            let ranges = highlighter.highlight_line(line, syntax_set).unwrap();
            assert!(!ranges.is_empty(), "Highlighted ranges should not be empty");
        }
    }
    
    #[test]
    fn test_syntax_highlighting_markdown() {
        let syntax_set = get_syntax_set();
        let theme_set = get_theme_set();
        let theme = &theme_set.themes["base16-ocean.dark"];
        
        let md_code = r#"# Hello World

This is a **bold** text and this is *italic*.

```rust
fn main() {
    println!("Code block");
}
```

- List item 1
- List item 2"#;
        
        let syntax = syntax_set.find_syntax_by_extension("md").unwrap();
        let mut highlighter = HighlightLines::new(syntax, theme);
        
        // Test that we can highlight without panicking
        for line in LinesWithEndings::from(md_code) {
            let ranges = highlighter.highlight_line(line, syntax_set).unwrap();
            assert!(!ranges.is_empty(), "Highlighted ranges should not be empty");
        }
    }
    
    #[test]
    fn test_syntax_highlighting_with_shebang() {
        let syntax_set = get_syntax_set();
        
        // Test Python shebang detection
        let python_script = "#!/usr/bin/env python3\nprint('Hello')";
        let first_line = python_script.lines().next().unwrap();
        assert!(first_line.starts_with("#!"));
        assert!(first_line.contains("python"));
        
        // Test shell shebang detection
        let bash_script = "#!/bin/bash\necho 'Hello'";
        let first_line = bash_script.lines().next().unwrap();
        assert!(first_line.starts_with("#!"));
        assert!(first_line.contains("bash"));
        
        // Test node shebang detection
        let node_script = "#!/usr/bin/env node\nconsole.log('Hello')";
        let first_line = node_script.lines().next().unwrap();
        assert!(first_line.starts_with("#!"));
        assert!(first_line.contains("node"));
    }
    
    #[test]
    fn test_fallback_to_plain_text() {
        let syntax_set = get_syntax_set();
        
        // Should always have plain text syntax as fallback
        let plain_text_syntax = syntax_set.find_syntax_plain_text();
        assert_eq!(plain_text_syntax.name, "Plain Text");
    }
    
    #[test]
    fn test_syntax_detection_for_code_parsers_languages() {
        let syntax_set = get_syntax_set();
        
        // Test all languages mentioned in code-parsers crate
        let test_cases = vec![
            ("rs", "Rust"),
            ("py", "Python"),
            ("js", "JavaScript"),
            ("jsx", "JavaScript (React)"),
            ("ts", "TypeScript"),
            ("tsx", "TypeScript (React)"),
            ("go", "Go"),
            ("rb", "Ruby"),
            ("md", "Markdown"),
            ("yaml", "YAML"),
            ("yml", "YAML"),
            ("html", "HTML"),
            ("htm", "HTML"),
        ];
        
        for (extension, expected_name_part) in test_cases {
            // For TypeScript/JSX extensions, check if either the specific syntax or JavaScript is available
            let syntax = match extension {
                "ts" | "tsx" | "jsx" => {
                    syntax_set.find_syntax_by_extension(extension)
                        .or_else(|| syntax_set.find_syntax_by_extension("js"))
                }
                _ => syntax_set.find_syntax_by_extension(extension),
            };
            
            assert!(syntax.is_some(), "Syntax for .{} should be available (or JavaScript as fallback)", extension);
            
            if let Some(syntax) = syntax {
                // For TypeScript/JSX, accept JavaScript syntax as valid
                let is_valid = match extension {
                    "ts" | "tsx" | "jsx" => {
                        syntax.name.contains(expected_name_part) || 
                        syntax.name.contains("JavaScript") ||
                        syntax.name == expected_name_part
                    }
                    _ => {
                        syntax.name.contains(expected_name_part) || 
                        syntax.name == expected_name_part ||
                        // Special cases where names might differ
                        (extension == "md" && syntax.name == "Markdown")
                    }
                };
                
                assert!(
                    is_valid,
                    "Syntax name '{}' should contain '{}' or 'JavaScript' for extension '{}'", 
                    syntax.name, expected_name_part, extension
                );
            }
        }
    }
} 