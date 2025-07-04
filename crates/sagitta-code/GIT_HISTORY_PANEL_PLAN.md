# Git History Panel Implementation Plan

## Overview
This document outlines the implementation plan for adding a git history panel to Sagitta Code. The panel will display git log history with graph visualization and provide revert functionality.

## Implementation Approach: Modal Window

### Why Modal Window?
1. Git history visualization benefits from more screen space
2. It's an occasional-use feature that doesn't need to be always visible  
3. Consistent with the events panel pattern (floating window)
4. Allows for richer interaction with the graph

## Dependencies

### Required Crates
- `egui_graphs = "0.25.1"` - For commit graph visualization (compatible with egui 0.31)
- `petgraph = "0.8"` - Graph data structure (included by egui_graphs)
- `git2 = "0.18"` - Already in dependencies for git operations

## File Structure

```
src/gui/git_history/
├── mod.rs          # Module definition and exports
├── modal.rs        # Main modal implementation
├── graph.rs        # Graph visualization logic
├── types.rs        # Data structures (CommitNode, etc.)
└── tests.rs        # Unit tests
```

## Key Components

### 1. GitHistoryModal Structure
```rust
pub struct GitHistoryModal {
    visible: bool,
    repository_path: Option<PathBuf>,
    commits: Vec<CommitInfo>,
    graph: Option<Graph<CommitNode, EdgeWeight>>,
    selected_commit: Option<String>,
    show_revert_confirmation: bool,
    search_query: String,
    branch_filter: Option<String>,
}
```

### 2. Core Features
- **Commit Graph Visualization**: Using egui_graphs to display branch/merge history
- **Commit Details Panel**: Show author, date, message, files changed
- **Branch Visualization**: Display branches and their relationships
- **Revert Functionality**: Revert to specific commit with confirmation
- **Search/Filter**: Filter commits by message, author, or branch
- **Theme Integration**: Respect current theme colors

### 3. Integration Points

#### Panel System Integration
- Add `GitHistory` variant to `ActivePanel` enum
- Add case in `render_panels()` function
- Add toggle logic in `toggle_panel()` method

#### Hotkey Integration
- Add `Ctrl+G` hotkey handler in main event loop
- Add to hotkeys modal documentation

#### Repository Integration
- Connect to current repository selection from dropdown
- Update when repository changes
- Handle case when no repository is selected

## Implementation Steps

### Phase 1: Basic Structure (Priority: High)
1. ✅ Create implementation plan file
2. ✅ Update Cargo.toml dependencies
3. ✅ Create module structure
4. ✅ Implement basic GitHistoryModal struct
5. ✅ Add to panel system
6. ✅ Implement hotkey

### Phase 2: Git Integration (Priority: High)
7. ✅ Implement git log fetching with git2
8. ✅ Parse commit information
9. ✅ Build commit graph structure
10. ✅ Handle branch information

### Phase 3: Visualization (Priority: Medium)
11. ✅ Integrate egui_graphs
12. ✅ Implement graph layout
13. ✅ Add commit details panel
14. ✅ Apply theme styling

### Phase 4: Interactive Features (Priority: Medium)
15. ✅ Implement commit selection
16. ✅ Add revert functionality
17. ✅ Add confirmation dialog
18. ✅ Handle revert errors

### Phase 5: Polish (Priority: Low)
19. Add search/filter UI
20. Implement search logic
21. Add loading states
22. Performance optimization

## Testing Strategy

### Unit Tests
- Git history fetching functions
- Commit parsing logic
- Graph building algorithms
- Search/filter functions

### Integration Tests
- Modal open/close behavior
- Repository switching
- Revert operation flow
- Hotkey functionality

### Manual Testing
- Visual appearance across themes
- Graph layout with complex histories
- Performance with large repositories
- Error handling scenarios

## API Design

### Public Interface
```rust
impl GitHistoryModal {
    pub fn new() -> Self;
    pub fn set_repository(&mut self, path: PathBuf);
    pub fn toggle(&mut self);
    pub fn render(&mut self, ctx: &Context, theme: AppTheme);
}
```

### Internal Methods
```rust
impl GitHistoryModal {
    fn fetch_commits(&mut self) -> Result<()>;
    fn build_graph(&mut self) -> Result<()>;
    fn render_graph(&mut self, ui: &mut Ui);
    fn render_details(&mut self, ui: &mut Ui);
    fn handle_revert(&mut self, commit_id: &str) -> Result<()>;
}
```

## Error Handling

- Repository not found → Show message in modal
- Git operations fail → Display error with context
- Graph too large → Implement pagination/filtering
- Revert conflicts → Show detailed error message

## Performance Considerations

- Lazy load commits (initial load of last 100)
- Cache graph layout calculations
- Debounce search input
- Use async operations for git commands

## Future Enhancements

1. Diff viewer for selected commits
2. Cherry-pick functionality
3. Interactive rebase support
4. Blame view integration
5. Statistics and insights
6. Export commit history

## Implementation Status

✅ **COMPLETED** - All major features have been implemented!

### What's Done:
- Modal window with graph visualization
- Git history fetching with git2
- Commit graph visualization (custom implementation, not using egui_graphs directly)
- Interactive commit selection
- Commit details panel
- Revert functionality with confirmation
- Theme-aware styling
- Hotkey support (Ctrl+G)
- Repository context integration

### What's Pending:
- Search/filter functionality (low priority)
- Additional documentation

## Testing

To test the git history panel:
1. Run the application with `cargo run --features gui`
2. Select a repository from the dropdown
3. Press Ctrl+G to open the git history modal
4. Click on commits to view details
5. Use the revert button to revert to a specific commit

## Status Tracking

This plan will be updated as implementation progresses. Check the todo list for current status of each task.

Last Updated: 2025-01-04