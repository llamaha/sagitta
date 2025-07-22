# Tool Rendering Simplification Plan

## Overview
Remove collapsing headers from tool results and replace with a simpler fixed-height preview system with scrollbars.

## Current Problems
1. Collapsing headers add visual complexity and spacing issues
2. Inconsistent spacing between different tool types
3. Complex state management for collapsed/expanded states
4. Different rendering paths causing inconsistencies

## Proposed Solution
- Fixed height previews (400px or 15 lines max)
- Automatic scrollbars for content exceeding the limit
- Consistent rendering for ALL tool types
- No collapsing/expanding - always show content

## Files to Modify/Create

### New Files
1. `crates/sagitta-code/src/gui/chat/simplified_tool_renderer.rs`
   - Contains the new `SimplifiedToolRenderer` struct and implementation
   - All tool-specific rendering logic in one place
   - Clear separation of concerns

### Files to Modify
1. `crates/sagitta-code/src/gui/chat/view.rs`
   - Add import for `SimplifiedToolRenderer`
   - Add feature flag: `use_simplified_tool_rendering`
   - Modify `render_single_tool_call()` to conditionally use new renderer
   - Modify `render_tool_card()` to conditionally use new renderer

2. `crates/sagitta-code/src/gui/chat/mod.rs`
   - Add `pub mod simplified_tool_renderer;`

### Files to Eventually Remove
1. `crates/sagitta-code/src/gui/chat/view/collapsing_header_helper.rs`
2. Individual render functions in `view.rs`:
   - `render_search_output()`
   - `render_search_output_legacy()`
   - `render_file_read_output()`
   - `render_file_write_output()`
   - `render_repository_output()`
   - `render_todo_output()`
   - `render_ping_output()`

## Implementation Status

### ✅ Phase 1: Create New Simplified Renderer - COMPLETED
- Created `SimplifiedToolRenderer` struct in `simplified_tool_renderer.rs`
- Fixed height of 400px with scroll areas
- Line limit of 15 lines for text content
- No collapsing header logic
- Consistent padding/margins for all tools

### ✅ Phase 2: Implement Core Rendering Logic - COMPLETED
- Implemented main `render()` method
- Created `render_header()` with tool name and inline params
- Created `render_content_area()` with scroll area
- Created `render_actions()` for action buttons
- Helper methods: `get_inline_params()`, `get_status_indicator()`, `should_show_actions()`

### ✅ Phase 3: Tool-Specific Renderers - COMPLETED
- `render_file_content()` - File read with syntax highlighting and font size controls
- `render_write_result()` - File write confirmations
- `render_edit_result()` - Edit operation results
- `render_shell_output()` - Terminal output in monospace
- `render_search_results()` - Clickable search results
- `render_todo_list()` - Todo items with status icons
- `render_repository_info()` - Repository lists
- `render_ping_result()` - Ping responses
- `render_generic_content()` - Fallback for unknown tools

### ✅ Phase 4: Update Rendering Pipeline - COMPLETED
- Added `use_simplified_tool_rendering` to `AppState`
- Updated function signatures to pass the flag
- Created conditional wrapper functions
- Added settings toggle in UI preferences

### ✅ Phase 5: Side-by-Side Testing - COMPLETED
- Feature flag works correctly
- Toggle in settings enables/disables simplified rendering
- Both rendering modes work without errors

### ✅ Phase 5.5: UI Improvements - COMPLETED

Based on user feedback, implemented the following improvements:

1. **Fixed tool card width** - Limited to 900px max width instead of stretching full screen
2. **Removed all icons/symbols** - Removed emoji and special characters that don't render correctly
3. **Fixed adaptive height** - Cards now only use the height needed up to 800px max
4. **Improved text alignment** - Added explicit left alignment for tool cards
5. **Cleaned up status indicators** - Using text instead of symbols (e.g., "Success" instead of "✓")

### 🚧 Phase 6: Cleanup Legacy Code - READY TO START

#### Code to Remove (when fully migrated):
1. **Collapsing Header Helper**
   - [ ] Remove `crates/sagitta-code/src/gui/chat/view/collapsing_header_helper.rs`
   - [ ] Remove imports and usage of `create_controlled_collapsing_header`
   - [ ] Remove imports and usage of `get_tool_card_state`

2. **Tool Card State Management**
   - [ ] Remove `tool_cards_collapsed` from `AppState`
   - [ ] Remove `tool_card_individual_states` from `AppState`
   - [ ] Remove these parameters from all function signatures

3. **Legacy Rendering Functions** (in view.rs)
   - [ ] Remove the old `render_single_tool_call` implementation (keep wrapper)
   - [ ] Remove the old `render_tool_card` implementation (keep wrapper)
   - [ ] Remove `render_file_read_output`
   - [ ] Remove `render_file_write_output` 
   - [ ] Remove `render_search_output`
   - [ ] Remove `render_search_output_legacy`
   - [ ] Remove `render_repository_output`
   - [ ] Remove `render_todo_output`
   - [ ] Remove `render_ping_output`
   - [ ] Remove `render_terminal_output`
   - [ ] Remove `render_diff_output`

4. **ToolResultRenderer Struct**
   - [ ] Remove the old `ToolResultRenderer` struct and implementation
   - [ ] Remove its usage in tool rendering

5. **Helper Functions**
   - [ ] Remove tool type checking functions if not used elsewhere
   - [ ] Remove `FILE_READ_FONT_SIZES` thread local (moved to simplified renderer)

#### Code to Keep:
- The wrapper functions that switch between old/new rendering
- The feature flag in `AppState`
- The settings toggle
- Tool mappings and syntax highlighting (used by both)

#### Migration Strategy:
1. Keep both implementations side-by-side temporarily
2. Default to old rendering for stability
3. Allow users to opt-in to simplified rendering
4. After testing period, switch default to simplified
5. Eventually remove old implementation entirely

## Implementation Plan

### Phase 1: Create New Simplified Renderer
1. Create `SimplifiedToolRenderer` struct alongside existing `ToolResultRenderer`
2. Key features:
   - Fixed max height of 400px
   - Line limit of 15 lines for text content
   - Automatic vertical scrollbar when content exceeds limits
   - No collapsing header logic
   - Consistent padding/margins for all tools

### Phase 2: Implement Core Rendering Logic

#### Design Principles for Maintainability
1. **Single Responsibility**: Each method does one thing well
2. **Clear Naming**: Methods and variables have descriptive names
3. **Consistent Patterns**: All tool types follow the same rendering pattern
4. **Minimal Dependencies**: Reduce coupling with other parts of the codebase
5. **Easy to Extend**: Adding new tool types should be straightforward

```rust
// In simplified_tool_renderer.rs
use egui::{Frame, ScrollArea, Ui, RichText, Vec2};
use serde_json::Value;
use crate::gui::theme::AppTheme;
use crate::gui::chat::tool_mappings::{get_human_friendly_tool_name, get_tool_icon};

pub struct SimplifiedToolRenderer<'a> {
    ui: &'a mut egui::Ui,
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
        ui: &'a mut egui::Ui,
        tool_name: &'a str,
        result: &'a serde_json::Value,
        app_theme: AppTheme,
    ) -> Self {
        Self { ui, tool_name, result, app_theme }
    }
    
    /// Main entry point - renders the complete tool result
    pub fn render(self) -> Option<(String, String)> {
        let mut action = None;
        
        // Outer frame for the entire tool result
        Frame::NONE
            .fill(self.app_theme.panel_background())
            .stroke(egui::Stroke::new(1.0, self.app_theme.border_color()))
            .corner_radius(4.0)
            .show(self.ui, |ui| {
                // 1. Render header
                self.render_header(ui);
                
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
                ui.label(RichText::new(params).small());
            }
            
            // Status indicator on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                    name if name.contains("read_file") => self.render_file_content(ui),
                    name if name.contains("write_file") => self.render_write_result(ui),
                    name if name.contains("search") => self.render_search_results(ui, action),
                    name if name.contains("shell") => self.render_shell_output(ui),
                    name if name.contains("todo") => self.render_todo_list(ui),
                    name if name.contains("repository") => self.render_repository_info(ui),
                    _ => self.render_generic_content(ui),
                }
                
                ui.add_space(Self::CONTENT_PADDING);
            });
    }
}
```

### Phase 3: Tool-Specific Renderers

#### Key Implementation Details

```rust
impl<'a> SimplifiedToolRenderer<'a> {
    /// Extract key parameters to show in header
    fn get_inline_params(&self) -> Option<String> {
        match self.tool_name {
            name if name.contains("file") => {
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
            name if name.contains("shell") => {
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
            
            // Render with syntax highlighting
            render_syntax_highlighted_code(
                ui,
                &truncated_content,
                file_ext,
                &self.app_theme.code_background(),
                ui.available_width(),
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
                    
                let preview = result.get("preview")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                    
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i + 1));
                    if ui.link(file_path).clicked() {
                        // Return file path and line number as action
                        if let Some(line) = result.get("startLine").and_then(|v| v.as_i64()) {
                            *action = Some(("open_file".to_string(), 
                                format!("{}:{}", file_path, line)));
                        }
                    }
                });
                
                if !preview.is_empty() {
                    ui.add_space(2.0);
                    ui.label(RichText::new(preview).small().code());
                }
                
                ui.add_space(4.0);
            }
            
            if results.len() > 10 {
                ui.label(RichText::new(format!("... and {} more results", results.len() - 10))
                    .small()
                    .color(self.app_theme.hint_text_color()));
            }
        } else {
            ui.label("No results found");
        }
    }
}
```

#### Maintainability Guidelines

1. **Helper Methods**: Each renderer has clear helper methods:
   - `get_inline_params()` - Extract display parameters
   - `get_status_indicator()` - Get success/error status
   - `should_show_actions()` - Determine if action buttons needed

2. **Consistent Error Handling**:
   ```rust
   fn get_value_or_default(result: &Value, keys: &[&str], default: &str) -> String {
       for key in keys {
           if let Some(value) = result.get(key).and_then(|v| v.as_str()) {
               return value.to_string();
           }
       }
       default.to_string()
   }
   ```

3. **Clear Constants**:
   - All magic numbers defined as constants
   - Easy to adjust limits and sizes in one place

### Phase 4: Update Rendering Pipeline
1. Create new functions:
   - `render_tool_simplified()` - replaces `render_single_tool_call()`
   - `render_tool_card_simplified()` - replaces `render_tool_card()`

2. Remove dependencies on:
   - `CollapsingHeader`
   - `collapsing_header_helper.rs`
   - Tool card state management
   - Individual collapsed states

### Phase 5: Side-by-Side Testing
1. Add feature flag or setting to toggle between old and new rendering
2. Test with various tool outputs:
   - Empty results (should be compact)
   - Small results (no scrollbar needed)
   - Large results (scrollbar appears)
   - Very large results (limited to max height)

### Phase 6: Migration
1. Replace old rendering functions with new ones
2. Remove old code:
   - `create_controlled_collapsing_header()`
   - `get_tool_card_state()`
   - Tool card individual states
   - Complex spacing logic

3. Clean up:
   - Remove unused imports
   - Delete `collapsing_header_helper.rs`
   - Remove state management code

## Benefits
1. **Simpler Code**: No collapsing logic, state management, or special cases
2. **Consistent Appearance**: All tools render the same way
3. **Predictable Spacing**: Fixed heights mean predictable layout
4. **Better Performance**: No dynamic height calculations
5. **Easier Maintenance**: One rendering path for all tools

## UI Mockup
```
┌─────────────────────────────────────────┐
│ [R] Read File - path: src/main.rs    ✓  │
├─────────────────────────────────────────┤
│ ┌─────────────────────────────────────┐ │
│ │ fn main() {                         │ │
│ │     println!("Hello, world!");      │ │
│ │     // ... (scrollable if needed)   │ │
│ │                                     │ │
│ └─────────────────────────────────────┘ │
│ View Full | Copy | Open in Editor       │
└─────────────────────────────────────────┘
```

## Implementation Order
1. Start with `SimplifiedToolRenderer` struct
2. Implement for one tool type (e.g., file read)
3. Add feature toggle
4. Test side-by-side
5. Port remaining tool types
6. Remove old code
7. Clean up

## Estimated Effort
- Initial implementation: 2-3 hours
- Testing and refinement: 1-2 hours
- Migration and cleanup: 1 hour
- Total: ~4-6 hours

## Risks and Mitigation
- **Risk**: Users might miss collapsing functionality
  - **Mitigation**: Fixed height is generous (400px), most content fits
  
- **Risk**: Some tools might need special handling
  - **Mitigation**: Build generic solution first, add minimal specialization only if needed

## Success Criteria
1. All tool results render with consistent spacing
2. No more missing icons or unicode issues
3. Simpler codebase with less state management
4. Predictable, clean UI appearance