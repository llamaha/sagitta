# OpenAI Compatible API Issues Fix Plan

## Issues Identified from Chat Log

### 1. Semantic Search Click Functionality Still Not Working
- **Issue**: Clicking on semantic search results does nothing
- **Root Cause**: Need to investigate the click event propagation chain

### 2. Shell Command Output Display Issues
- **Issue**: Shell output appears truncated or malformed in JSON format
- **Evidence**: The `cat` command output shows as JSON with escape sequences instead of proper text

### 3. Tool Result Formatting Issues
- **Issue**: Tool results are being displayed as JSON strings instead of rendered content
- **Evidence**: File read results show as `FILE: File Content` with empty content

### 4. Repository Path Confusion
- **Issue**: Commands are being executed with incorrect working directories
- **Evidence**: `working_directory: /home/adam/code/sagitta/` vs actual `/home/adam/repos/sagitta/`

### 5. Tool Name Display Issues  
- **Issue**: Tool names appear duplicated (e.g., "🔎 🔎 Semantic Code Search")
- **Evidence**: Icons and names are doubled in the tool headers

### 6. System Prompt Issues for Devstral
- **Issue**: The system prompt may be causing issues with tool execution and formatting
- **Evidence**: Working directory confusion and JSON formatting problems

## Fix Plan

### Phase 1: Diagnose Semantic Search Click Issue
1. Add debug logging to trace click events through the chain:
   - `render_search_result_item` → `render_search_output` → `modern_chat_view_ui` → `rendering.rs`
2. Test with a simple hardcoded action to verify the event chain works
3. Check if the issue is with the action data format or the handler

### Phase 2: Fix Shell Command Output Display
1. Investigate why shell output is being JSON-encoded
2. Check if this is related to the OpenAI-compatible streaming format
3. Ensure raw stdout/stderr are properly displayed without JSON wrapping
4. Test with various shell commands (cat, ls, echo)

### Phase 3: Fix Tool Result Rendering
1. Identify why file read results show empty content
2. Check the tool result formatting pipeline
3. Ensure proper content extraction from tool results
4. Test with different tool types

### Phase 4: Fix Repository Path Issues
1. Investigate the working directory detection logic
2. Ensure consistent path handling across all tools
3. Add validation for working directory existence

### Phase 5: Fix Duplicate Tool Names
1. Find where tool names are being duplicated
2. Check the tool header rendering logic
3. Ensure icons appear only once

### Phase 6: Optimize Devstral System Prompt
1. Simplify the system prompt to reduce confusion
2. Remove OpenHands-specific instructions that don't apply
3. Add clear instructions about tool result formatting
4. Test with minimal prompt first, then add back necessary parts

### Phase 7: Add Tests
1. Create tests for semantic search click functionality
2. Add tests for shell output rendering
3. Test tool result formatting with various outputs
4. Create integration tests for the full flow

## Implementation Order

1. **Start with Phase 6** - Fix system prompt as it may resolve other issues
2. **Phase 1** - Debug semantic search clicks (highest user priority)
3. **Phase 2 & 3** - Fix output display issues
4. **Phase 4 & 5** - Fix UI/UX issues
5. **Phase 7** - Add comprehensive tests

## Success Criteria

- [ ] Clicking semantic search results opens a modal with code snippet
- [ ] Shell command output displays properly without JSON encoding
- [ ] File read results show actual content
- [ ] Working directories are correct
- [ ] Tool names appear only once with single icon
- [ ] All tests pass
- [ ] Devstral model works smoothly with the application