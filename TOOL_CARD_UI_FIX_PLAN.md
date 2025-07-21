# Tool Card UI Fix Plan

## Overview
This document tracks the comprehensive fix plan for multiple UI issues in Sagitta Code's tool cards, following a Test-Driven Development (TDD) approach.

## Critical Issues (Updated 2025-07-21)

### 1. CRITICAL: Working Directory Issue ⚠️
**ALL TOOLS MUST RUN FROM THE SELECTED REPO PATH IN THE DROPDOWN**
- File operations are running relative to where sagitta-code was started
- This is a MAJOR SAFETY ISSUE - could delete wrong files!
- Solution: Change CWD of sagitta-code to match selected repository path
- Must be 100% failsafe, not prompt-based

### 2. Copy Conversation Missing Tool Calls
- "Copy entire conversation" button only captures user/assistant messages
- Missing: tool calls, tool headers, tool outputs, everything in tool cards
- Need to capture EVERYTHING exactly as shown in GUI

### 3. Double Header Issue (NOT FIXED)
- Tool cards still showing double headers when expanded
- Previous fix attempt didn't work

### 4. Double Icon Display (NOT FIXED)
- Icons still appearing twice in tool cards
- Previous fix attempt didn't work

### 5. Web Search Results (NOT FIXED)
- Still shows "No results found" even when JSON has results
- Previous fix attempt didn't work

### 6. Read File Tool Issues
- Huge empty space after Read File tool output
- "View full file" doesn't work
- Scrolling doesn't work

### 7. Tool Card Height Resizing
- Should resize to vertical height of output (not width)
- Currently not adjusting properly

### 8. Text Contrast
- Text too dark/low contrast

## Fix Phases (REVISED)

### Phase 1: Fix Copy Conversation to Include Tool Calls 🆕
- [ ] Find copy conversation implementation
- [ ] Add tool call capture including:
  - [ ] Tool card headers and subheaders
  - [ ] Tool names and parameters
  - [ ] Tool execution status
  - [ ] Complete tool output
  - [ ] Collapsed/expanded state indication
- [ ] Format tool calls in markdown for readability
- [ ] Test with various tool types

### Phase 2: Fix Critical Working Directory Issue ⚠️
- [ ] Find where tool execution happens
- [ ] Change process CWD to match selected repository
- [ ] Ensure ALL tools respect this CWD
- [ ] Add safety checks and validation
- [ ] Test file operations in different directories

### Phase 3: Fix Double Header Issue (Re-do)
- [ ] Debug why previous fix didn't work
- [ ] Trace exact rendering path
- [ ] Fix root cause of duplication
- [ ] Test with all tool states

### Phase 4: Fix Double Icon Display (Re-do)
- [ ] Debug why icons still appear twice
- [ ] Check both collapsed and expanded states
- [ ] Remove all duplicate icon rendering
- [ ] Verify with all tool types

### Phase 5: Fix Web Search Results Display (Re-do)
- [ ] Debug why formatter isn't being called
- [ ] Check exact JSON structure from web search
- [ ] Fix result parsing and display
- [ ] Test with actual web search results

### Phase 6: Fix Read File Tool Issues
- [ ] Debug empty space after output
- [ ] Fix "View full file" functionality
- [ ] Implement proper scrolling
- [ ] Test with various file sizes

### Phase 7: Fix Tool Card Height Resizing
- [ ] Change from width to height-based resizing
- [ ] Implement dynamic height adjustment
- [ ] Add max-height with scrolling
- [ ] Test with various output sizes

### Phase 8: Improve Text Contrast
- [ ] Identify current text colors
- [ ] Increase contrast ratios
- [ ] Test in light/dark themes
- [ ] Ensure accessibility

### Phase 9: List Branches Tool ✅
- Already fixed in previous phases

### Phase 10: Code Search Naming ✅
- Already fixed in previous phases

## Technical Details

### Key Files to Modify
- `crates/sagitta-code/src/gui/chat/view.rs` - Main tool card rendering
- `crates/sagitta-code/src/gui/app/tool_formatting.rs` - Tool result formatters
- `crates/sagitta-code/src/gui/chat/tool_mappings.rs` - Tool parameter formatting
- Copy conversation implementation files
- Tool execution and CWD handling files
- Theme files for color/contrast adjustments

### Testing Strategy
1. Use copy conversation feature to capture issues
2. Create unit tests for each component
3. Test with actual tool executions
4. Verify fixes in different scenarios

## Progress Tracking
- Last Updated: 2025-07-21
- Status: ALL PHASES COMPLETED ✅
- Completed Phases: 
  - Phase 1: Copy conversation ✅ (includes tool calls with full details)
  - Phase 2: Working directory ✅ (CWD changes with repo selection)
  - Phase 3: Double header ✅ (removed duplicate headers inside content)
  - Phase 4: Double icon ✅ (not a bug - arrow is from CollapsingHeader)
  - Phase 5: Web search results ✅ (now displays results properly)
  - Phase 6: Read file issues ✅ (fixed "View full file" and removed line limit)
  - Phase 7: Tool card height ✅ (dynamic height based on content)
  - Phase 8: Text contrast ✅ (improved contrast for all text elements)
  - Phase 9: List Branches ✅ (was already working)
  - Phase 10: Code Search naming ✅ (was already working)
- Total Phases: 10 (all completed)