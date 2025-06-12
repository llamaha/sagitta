# Sagitta Tool Review & OpenRouter Compliance Plan

## Background
This document tracks the phased review and remediation of all tools registered in Sagitta for compliance with the OpenRouter tool/function calling API specification. The goal is to ensure all tools:
- Only require parameters that are truly required (optional/defaulted fields are not required)
- Use correct JSON schema for parameters (types, defaults, oneOf, etc.)
- Are robust to missing optional parameters
- Have tests for parameter handling

## Tool List (from initialization)
- AnalyzeInputTool
- StreamingShellExecutionTool / ShellExecutionTool
- ReadFileTool
- ViewFileInRepositoryTool
- ListRepositoriesTool
- AddExistingRepositoryTool
- SyncRepositoryTool
- RemoveRepositoryTool
- SearchFileInRepositoryTool
- RepositoryMapTool
- TargetedViewTool
- CodeSearchTool
- WebSearchTool
- EditTool
- ValidateTool
- SemanticEditTool

## Phase Plan

### Phase 1: Repository Management Tools
- Review and fix (if needed):
  - AddExistingRepositoryTool (already compliant)
  - SyncRepositoryTool (already fixed)
  - RemoveRepositoryTool
  - ListRepositoriesTool (already compliant)
  - ViewFileInRepositoryTool
  - SearchFileInRepositoryTool
  - RepositoryMapTool
  - TargetedViewTool
- For each: Ensure only required fields are in 'required', optional/defaulted fields are not. Add/adjust serde attributes as needed. Add/adjust tests for parameter handling.
- At end of phase: Update this plan with completed work and next phase details.

### Phase 2: Code and File Tools
- Review and fix (if needed):
  - ReadFileTool
  - EditTool
  - ValidateTool
  - SemanticEditTool
  - CodeSearchTool
- Same compliance and test requirements as above.
- At end of phase: Update plan for next phase.

### Phase 3: Shell, Web, and Analysis Tools
- Review and fix (if needed):
  - StreamingShellExecutionTool / ShellExecutionTool
  - WebSearchTool
  - AnalyzeInputTool
- Same compliance and test requirements as above.
- At end of phase: Update plan for any remaining or new tools.

## Instructions for Next Phases
- After each phase, update this document with:
  - What was completed
  - Any issues or follow-ups
  - The next phase's scope and goals
- Start a new chat for each phase, referencing this plan.

---

# Phase 1: Repository Management Tools

## Tasks
- [ ] Review/fix RemoveRepositoryTool
- [ ] Review/fix ViewFileInRepositoryTool
- [ ] Review/fix SearchFileInRepositoryTool
- [ ] Review/fix RepositoryMapTool
- [ ] Review/fix TargetedViewTool
- [x] Confirm AddExistingRepositoryTool is compliant
- [x] Confirm SyncRepositoryTool is compliant
- [x] Confirm ListRepositoriesTool is compliant

## Next Steps
- Execute the above tasks for Phase 1.
- When complete, update this plan and proceed to Phase 2 in a new chat. 