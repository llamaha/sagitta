# Tool Card UI Fix Plan V2

## Issues Identified from Screenshot

### 1. Duplicate Icons in Tool Headers
- Each tool card shows TWO icons: the collapse arrow (▶/▼) AND the tool icon (📁, 🔍, etc.)
- Some icons aren't rendering properly (showing as ☐☐ boxes)
- Need to either remove the arrow or the tool icon, or redesign the header

### 2. Horizontal Separator Line
- After removing the duplicate header text, there's still a horizontal separator line visible
- This creates unnecessary visual clutter

### 3. Copy Conversation Not Working
- "Copy entire conversation" still doesn't include tool outputs
- Only copying text messages, not tool cards with their results

### 4. Search Files Not Working
- Search Files with pattern "*.rs" returned empty results
- There's clearly a main.rs file that should have been found
- This is a functional bug, not just UI

### 5. Non-Rendering Icons
- Some tools show ☐☐ instead of proper icons
- Need to fix or replace these with working icons

## Fix Plan with TDD Approach

### Phase 1: Fix Search Files Functionality
**Priority: CRITICAL - This is a functional bug**
- [ ] Write test for search_file with "*.rs" pattern
- [ ] Debug why it's returning empty results
- [ ] Fix the search implementation
- [ ] Verify with multiple file patterns

### Phase 2: Fix Duplicate Icons
**Priority: HIGH - Visual confusion**
- [ ] Decide on design: Keep arrow only, keep tool icon only, or redesign
- [ ] Remove the tool icon from header text (keep arrow for expand/collapse)
- [ ] OR: Create custom header without default arrow
- [ ] Test all tool types to ensure consistency

### Phase 3: Remove Horizontal Separator
**Priority: HIGH - Visual clutter**
- [ ] Find where ui.separator() is called after headers
- [ ] Remove the separator line
- [ ] Ensure proper spacing without the line

### Phase 4: Fix Copy Conversation Tool Output
**Priority: HIGH - User requested multiple times**
- [ ] Debug why tool outputs aren't being captured
- [ ] Ensure tool results are included in copy
- [ ] Format tool outputs properly in the copied text
- [ ] Test with various tool types

### Phase 5: Fix Non-Rendering Icons
**Priority: MEDIUM - Visual polish**
- [ ] Identify which tools have broken icons (from screenshot: List Branches, etc.)
- [ ] Replace ☐☐ with working Unicode symbols or text
- [ ] Test all tool icons render correctly

## Technical Details

### Key Files to Investigate
- `crates/sagitta-code/src/gui/chat/view.rs` - Tool card rendering
- `crates/sagitta-code/src/gui/chat/tool_mappings.rs` - Icon definitions
- `crates/sagitta-mcp/src/handlers/search_file.rs` - Search implementation
- Copy conversation implementation
- Separator/divider rendering code

### Testing Strategy
1. Create unit tests for search_file functionality
2. Visual testing for icon display
3. Integration test for copy conversation
4. Manual verification of UI changes

## Progress Tracking
- Started: 2025-01-21
- Phase 1: IN PROGRESS
- Phase 2: PENDING
- Phase 3: PENDING  
- Phase 4: PENDING
- Phase 5: PENDING