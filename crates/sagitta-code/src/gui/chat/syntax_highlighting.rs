use egui::{Color32, TextFormat, Ui};
use syntect::{
    highlighting::{ThemeSet, Style as SyntectStyle},
    parsing::SyntaxSet,
    easy::HighlightLines,
    util::LinesWithEndings,
};
use std::sync::OnceLock;
use crate::gui::theme::AppTheme;
use similar::{ChangeTag, TextDiff};

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

pub fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

pub fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

/// Constants for diff display
pub const DIFF_COLLAPSING_THRESHOLD_LINES: usize = 20;
pub const EXPANDED_DIFF_SCROLL_AREA_MAX_HEIGHT: f32 = 360.0;

/// Render syntax highlighted code
pub fn render_syntax_highlighted_code(ui: &mut Ui, text: &str, language: &str, _bg_color: &Color32, max_width: f32) {
    // Default to 10.0 font size for backward compatibility
    render_syntax_highlighted_code_with_font_size(ui, text, language, _bg_color, max_width, 10.0);
}

/// Render syntax highlighted code with adjustable font size
pub fn render_syntax_highlighted_code_with_font_size(ui: &mut Ui, text: &str, language: &str, _bg_color: &Color32, max_width: f32, font_size: f32) {
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    
    let syntect_theme = &theme_set.themes["base16-ocean.dark"];
    let syntax = syntax_set.find_syntax_by_extension(language)
        .or_else(|| syntax_set.find_syntax_by_name(language))
        .or_else(|| syntax_set.find_syntax_by_extension("txt"))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    
    let mut highlighter = HighlightLines::new(syntax, syntect_theme);
    
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    
    // Use a more compact approach by building the layout job directly
    let mut layout_job = egui::text::LayoutJob::default();
    
    // Calculate appropriate line height based on font size
    let line_height = font_size * 1.2;
    
    for (_line_index, line) in LinesWithEndings::from(text).enumerate() {
        let ranges = highlighter.highlight_line(line, syntax_set).unwrap_or_default();
            
            for (style, text_part) in ranges {
                let color = syntect_style_to_color(&style);
            layout_job.append(
                text_part,
                0.0,
                TextFormat {
                    font_id: egui::FontId::monospace(font_size),
                    line_height: Some(line_height),
                    color,
                    ..Default::default()
                },
            );
            }
    }
    
    // Render the entire layout job as a single label
    ui.set_max_width(max_width);
    ui.label(layout_job);
}

fn syntect_style_to_color(style: &SyntectStyle) -> Color32 {
    Color32::from_rgb(
        style.foreground.r, 
        style.foreground.g, 
        style.foreground.b
    )
}

// New helper function to contain the core logic of preparing and rendering diff lines
pub fn render_internal_diff_display_logic(ui: &mut Ui, old_content: &str, new_content: &str, language: Option<&str>, _bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    let diff = TextDiff::from_lines(old_content, new_content);
    
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    let syntect_theme = &theme_set.themes["base16-ocean.dark"];
    
    let syntax = if let Some(lang) = language {
        syntax_set.find_syntax_by_extension(lang)
            .or_else(|| syntax_set.find_syntax_by_name(lang))
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    } else {
        syntax_set.find_syntax_by_extension("rs")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
    };

    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.set_max_width(max_width);
    ui.spacing_mut().item_spacing.y = 0.0; // Remove spacing between items
    render_diff_lines(ui, &diff, syntax, syntect_theme, syntax_set, app_theme);
}

/// Render a unified diff view of two code snippets with syntax highlighting
pub fn render_code_diff(ui: &mut Ui, old_content: &str, new_content: &str, language: Option<&str>, bg_color: &Color32, max_width: f32, app_theme: AppTheme) {
    let diff = TextDiff::from_lines(old_content, new_content);
    let total_lines = diff.iter_all_changes().count();

    if total_lines > DIFF_COLLAPSING_THRESHOLD_LINES { // Use the constant
        egui::CollapsingHeader::new(egui::RichText::new(format!("{total_lines} lines changed")).small())
            .default_open(false) // Keep it closed by default
            .show(ui, |header_ui| {
                egui::ScrollArea::vertical()
                    .max_height(EXPANDED_DIFF_SCROLL_AREA_MAX_HEIGHT) // Use the constant
                    .auto_shrink([false, true])
                    .show(header_ui, |scroll_ui| {
                        render_internal_diff_display_logic(scroll_ui, old_content, new_content, language, bg_color, scroll_ui.available_width(), app_theme);
                    });
            });
    } else {
        render_internal_diff_display_logic(ui, old_content, new_content, language, bg_color, max_width, app_theme);
    }
}

/// Helper function to render individual diff lines
pub fn render_diff_lines<'a>(
    ui: &mut Ui, 
    diff: &TextDiff<'a, 'a, 'a, str>, 
    syntax: &syntect::parsing::SyntaxReference, 
    syntect_theme: &syntect::highlighting::Theme,
    syntax_set: &SyntaxSet,
    app_theme: AppTheme
) {
    // Build a single layout job for all diff lines to eliminate spacing issues
    let mut layout_job = egui::text::LayoutJob::default();
    
    for change in diff.iter_all_changes() {
        let (line_bg_color, prefix_color, prefix_text) = match change.tag() {
            ChangeTag::Delete => (
                app_theme.diff_removed_bg(),     // Use theme colors
                app_theme.diff_removed_text(),   // Use theme colors
                "- "
            ),
            ChangeTag::Insert => (
                app_theme.diff_added_bg(),       // Use theme colors
                app_theme.diff_added_text(),     // Use theme colors
                "+ "
            ),
            ChangeTag::Equal => (
                Color32::TRANSPARENT,            // No background for unchanged lines
                Color32::from_rgb(150, 150, 150), // Gray prefix
                "  "
            ),
        };

        let line_content = change.value();
        
        // Add the prefix (-, +, or space) with appropriate color
        layout_job.append(
            prefix_text,
            0.0,
            TextFormat {
                font_id: egui::FontId::monospace(10.0),
                line_height: Some(12.0), // Tight line height
                color: prefix_color,
                background: line_bg_color,
                ..Default::default()
            },
        );

        // Handle the line content - we need to preserve the structure including newlines
        if line_content.trim().is_empty() {
            // For empty lines, just add the newline with background
            layout_job.append(
                line_content, // This preserves the actual whitespace/newline
                0.0,
                TextFormat {
                    font_id: egui::FontId::monospace(10.0),
                    line_height: Some(12.0),
                    color: prefix_color,
                    background: line_bg_color,
                    ..Default::default()
                },
            );
        } else {
            // For lines with content, syntax highlight them
            let mut highlighter = HighlightLines::new(syntax, syntect_theme);

            if let Ok(ranges) = highlighter.highlight_line(line_content, syntax_set) {
                for (style, text_part) in ranges {
                    let text_color = match change.tag() {
                        ChangeTag::Delete => app_theme.diff_removed_text(),
                        ChangeTag::Insert => app_theme.diff_added_text(),
                        ChangeTag::Equal => syntect_style_to_color(&style),
                    };
                    
                    layout_job.append(
                        text_part,
                        0.0,
                        TextFormat {
                            font_id: egui::FontId::monospace(10.0),
                            line_height: Some(12.0), // Tight line height
                            color: text_color,
                            background: line_bg_color,
                            ..Default::default()
                        },
                    );
                }
            } else {
                // Fallback to plain text if syntax highlighting fails
                let text_color = match change.tag() {
                    ChangeTag::Delete => app_theme.diff_removed_text(),
                    ChangeTag::Insert => app_theme.diff_added_text(),
                    ChangeTag::Equal => Color32::from_rgb(200, 200, 200),
                };
                
                layout_job.append(
                    line_content,
                    0.0,
                    TextFormat {
                        font_id: egui::FontId::monospace(10.0),
                        line_height: Some(12.0), // Tight line height
                        color: text_color,
                        background: line_bg_color,
                        ..Default::default()
                    },
                );
            }
        }
    }
    
    // Render the entire diff as a single label with tight spacing
    ui.label(layout_job);
}

/// Detect if content contains a diff pattern and extract the parts
pub fn detect_diff_content(content: &str) -> Option<(String, String, Option<String>)> {
    // Look for common diff patterns in tool results or messages
    
    // Pattern 1: "old content" -> "new content" format
    if let Some(arrow_pos) = content.find(" -> ") {
        let before_arrow = content[..arrow_pos].trim();
        let after_arrow = content[arrow_pos + 4..].trim();
        
        // Try to extract quoted content
        if before_arrow.starts_with('"') && before_arrow.ends_with('"') &&
           after_arrow.starts_with('"') && after_arrow.ends_with('"') {
            let old_content = before_arrow[1..before_arrow.len()-1].to_string();
            let new_content = after_arrow[1..after_arrow.len()-1].to_string();
            return Some((old_content, new_content, None));
        }
    }
    
    // Pattern 2: File edit operations in tool results
    if content.contains("edit_file") || content.contains("file_edit") || content.contains("old_content") {
        // Try to parse JSON for file edit operations
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let Some(obj) = json.as_object() {
                if let (Some(old), Some(new)) = (
                    obj.get("old_content").and_then(|v| v.as_str()),
                    obj.get("new_content").and_then(|v| v.as_str())
                ) {
                    let language = obj.get("language")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("file_extension").and_then(|v| v.as_str()));
                    return Some((old.to_string(), new.to_string(), language.map(|s| s.to_string())));
                }
            }
        }
    }
    
    None
}