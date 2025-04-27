# Relay Development Plan: Comprehensive Workflow

This document outlines the planned workflow and capabilities for the Relay agent. It focuses on a task-oriented approach integrating context management, environment preparation, core execution with validation, and finalization steps.

## Supported Languages

Relay aims to provide robust support for languages recognized by `vectordb-core`:

*   **Rust** (`rs`): `cargo` for init, build, test, format.
*   **Python** (`py`): `python -m venv`, `pip` for deps, `python` for run, `unittest`/`pytest` for test.
*   **Go** (`go`): `go mod init`, `go run`, `go build`, `go test`, `gofmt`.
*   **JavaScript** (`js`, `jsx`): `npm`/`yarn` for init/deps, `node` for run, framework-specific test commands (e.g., `npm test`). Requires `prettier` for formatting.
*   **TypeScript** (`ts`, `tsx`): As JS, plus `tsc` for build/check, `ts-node` for run. Requires `prettier` for formatting.
*   **Ruby** (`rb`): `bundle gem` or manual setup, `ruby` for run, `rake test` or framework tests.
*   **Markdown** (`md`) / **YAML** (`yaml`, `yml`): File creation/editing. Potential future linting.

*(Fallback parsing exists for other file types)*

## Core Workflow

Relay follows these phases for handling user requests:

### Phase 1: Context Initialization & Target Identification (In Progress)

*   **Goal:** Understand the user's intent and identify the target codebase.
*   **Steps:**
    1.  **Parse Request:** Analyze prompt for goal (create, modify, query) and target repo name.
    2.  **Identify Target:**
        *   **Explicit Target:** If name provided, use `list_repositories`.
            *   Found: `use_repository <target_name>`. Proceed.
            *   Not Found: Inform user, suggest `add_repository`. Halt or await input.
        *   **Implicit Target:** Check for existing active repo. Confirm usage with user or ask for target.
        *   **New Project Request:** If prompt is "create new...", proceed to *Phase 2: New Project Path*.
        *   **No Target:** If no target identified or confirmed, ask user to specify.

### Phase 2: Environment Preparation

This phase has two paths depending on whether we are working on a new or existing project.

#### Path A: Existing Repository (In Progress)

*   **Goal:** Prepare the identified existing repository for work.
*   **Steps:**
    1.  **Initial Sync:** `repo_sync` (Ensures index matches file state *before* changes).
    2.  **Status Check (Recommended):** `run_command git status` (Requires user approval).
    3.  **Initial Lint/Check (Optional):** `run_command [cargo check|tsc|...]` (User approval).

#### Path B: New Project Creation (Planned)

*   **Goal:** Create a new project based on user request and prepare it.
*   **Steps:**
    1.  **Standard Initialization:**
        *   Determine language.
        *   Formulate standard init command (`cargo new`, `npm init -y`, `go mod init`, etc.).
        *   `run_command <init_command>` (User approval).
    2.  **Initial Commit:**
        *   `run_command git add .` (in new dir).
        *   `run_command git commit -m "Initial project setup..."` (in new dir).
    3.  **Add & Use Repository:**
        *   `add_repository <path_to_new_project>`.
        *   `use_repository <new_project_name>`.
    4.  **Initial Sync:** `repo_sync` on the new repository.

### Phase 3: Core Task Execution Loop (Partially Implemented, Needs Enhancement)

*   **Goal:** Iteratively perform the user's requested task using appropriate tools and validation.
*   **Steps (Loop):**
    1.  **Understand Sub-Task:** Analyze the current specific instruction (e.g., "add function X", "find Y", "refactor Z").
    2.  **Information Gathering:**
        *   `semantic_search` (Preferred for finding relevant code).
        *   `read_file` (For detail, often using search results).
        *   `grep_search` (For specific patterns).
    3.  **Code Modification & Validation:**
        *   **Choose Edit Tool:**
            *   Prefer `semantic_edit` (for intent-based changes).
            *   Use `line_edit` (for precise line changes).
            *   Use `write_file` (for new files / replacements).
        *   **Perform Edit:** Execute chosen action.
        *   **Format Code (Planned):** `run_command [cargo fmt|prettier|gofmt|...]` (User approval).
        *   **Build/Test (Planned):**
            *   Identify build/test command (`cargo test`, `npm test`, `go test`, etc.).
            *   `run_command <build/test_command>` (User approval).
            *   **Analyze:** Check exit code, stdout/stderr.
                *   Success: Continue loop or proceed to Phase 4.
                *   Failure: Feed error back to agent. Initiate debug sub-loop (back to Info Gathering -> Modify -> Validate). Report failure to user if unfixable.
    4.  **Iteration:** Repeat for next sub-task or until goal achieved.

### Phase 4: Finalization & Synchronization (Partially Implemented, Needs Enhancement)

*   **Goal:** Ensure final code state is valid, committed, and synced.
*   **Steps:**
    1.  **Final Testing (Planned):** `run_command [cargo test|npm test|...]` one last time. Address failures via Phase 3 loop.
    2.  **Staging:** `run_command git add .` (or specific files).
    3.  **Commit:** `run_command git commit -m "Summary of changes..."`.
    4.  **Final Sync:** `repo_sync` on the active repository.
    5.  **Report Completion:** Inform user.

### Phase 5: Ongoing Refinement (Meta)

*   **Goal:** Continuously improve Relay based on usage and testing.
*   **Areas:**
    *   **Prompt Engineering:** Refine system prompts for better reasoning, tool use, and language-specific knowledge.
    *   **Tool Integration:** Improve how tools are chosen and chained. Enhance error handling and recovery logic.
    *   **Language Support:** Deepen understanding of idioms, standard libraries, and common frameworks for each supported language.
    *   **Testing & Optimization:** Add comprehensive unit/integration tests for Relay itself. Optimize performance. Address warnings.
    *   **User Experience:** Improve CLI output, progress indication, error reporting.
    *   **Documentation:** Maintain this plan, document architecture, provide user examples.
    *   **Safety & Security:** Containerization ("yolo mode"), security scanning, permissions.
    *   **Extension System:** (Future) Plugin architecture.

---

*(Original Phase structure and implementation details below are kept for historical reference but should be considered superseded by the workflow above)*

---

## Original Plan (Historical Reference)

### Phase 1: Core Infrastructure
*(Testing should be added for implemented core infrastructure)*

1.  **Project Setup** (Completed)
    *   Set up Rust workspace and crates
    *   Configure dependencies (clap, tokio, etc.)
    *   Establish basic module structure

2.  **Configuration** (Completed)
    *   Implement configuration loading (file/env)
    *   Define configuration structure (API keys, model settings)
    *   Set up default configuration values

3.  **Basic Prompt Handling** (Completed)
    *   Set up CLI argument parsing
    *   Accept initial user prompt
    *   Initialize basic chain state

4.  **LLM Integration & Streaming** (Completed)
    *   Connect to Anthropic API
    *   Implement streaming chat completions
    *   Handle SSE events correctly
    *   Set up basic error handling for API calls

### Phase 2: File & Repository Operations
*(Testing should be added for implemented actions)*

5.  **File Operations** (Completed)
    *   Implement file reading/viewing with pagination
    *   Add file writing/editing capabilities
    *   Create directory and file creation tools
    *   Implement line editing action

    **Testing Milestone: Basic File Interaction** (Completed)
    *   **Capabilities:** Agent able to read, write, and create files/directories relative to its starting CWD. It handles "file not found" errors gracefully and attempts basic commands (like `ls`) to gather context (pending user confirmation). Handles conversational text around action JSON.
    *   **Example Test:** `relay "Create a file named test.txt with the content 'Hello Relay!' and then show me its content."` (Expect it to use `write_file` then `read_file`).

6.  **Repository Integration** (Completed)
    *   Add repository initialization
    *   Implement repository adding (URL/local path)
    *   Implement repository listing
    *   Implement repository removal
    *   Implement repository sync
    *   Set up context switching between repositories
    *   Refactor repo operations to use vectordb-core functions directly

7.  **VectorDB Integration** (Completed)
    *   Connect to `vectordb-core`'s search functionality
    *   Set up semantic search for code exploration
    *   Implement semantic editing capabilities

8.  **Git Integration** (Completed)
    *   Add git status checking
    *   Implement git history browsing
    *   Create git operations (commit, branch, etc.)

    **Testing Milestone: Repo & Search Integration** (Completed)
    *   **Capabilities:** Agent able to add a local git repo, perform semantic searches within it using `vectordb-core`, retrieve relevant code snippets, and use `git status` (pending user confirmation). Context switches to the active repository.
    *   **Example Test:** `relay "Add the repository at ./my-local-repo. Then find functions related to 'user authentication' in that repository."` (Expect `add_repo`, `use_repo`, then `semantic_search`).

### Phase 3: Agent Capabilities
*(Testing should be added for implemented agent capabilities and actions)*

9.  **Investigation Loops** (Completed)
    *   Design investigation patterns
    *   Create loops for code understanding
    *   Build software generation workflows
    *   Implement loop detection system to prevent infinite loops

10. **Context Management** (Completed)
    *   Implement repository context for reference
    *   Add multi-repository context capabilities
    *   Create context windowing for large codebases
    *   Develop Context Advisor for optimization

11. **Prompt Engineering** (Completed)
    *   Design system prompts for various tasks
    *   Create templates for different operations
    *   Implement prompt construction logic

12. **Code Understanding** (Completed)
    *   Implement code explanation system
    *   Add refactoring assistance
    *   Create bug detection capabilities

    **Testing Milestone: Basic Code Assistance** (Completed)
    *   **Capabilities:** Agent able to answer simple questions about code found via semantic search, explain snippets, and perform basic edits (like `line_edit` or `semantic_edit`) based on user requests within the context of an active repository.
    *   **Example Test:** `relay "In the active repository, find the 'login' function. Read its content and then replace line 10 with 'log.info(\\"User login attempted\\")'."` (Expect `semantic_search`, `read_file`, `line_edit`).

### Phase 4: User Experience
*(Testing should be added for CLI interactions and output)*

13. **CLI Refinement** (In Progress)
    *   Build user-friendly command structure (Completed)
    *   Add progress indicators (In Progress)
    *   Implement error handling and recovery (Completed)
    *   Implement User Confirmation for Dangerous Actions (Completed)

14. **Output Formatting** (Completed)
    *   Implement streaming text formatting
    *   Add syntax highlighting
    *   Create summary views for results

--- **MVP Milestone (Achieved)** ---
*(Completion of core infrastructure, essential file/repo/search/command tools, basic investigation loop, basic context, basic CLI/output)*

15. **Documentation** (In Progress)
    *   Write user documentation (In Progress)
    *   Create examples (In Progress)
    *   Document internal architecture (In Progress)

16. **Technical Debt Tooling** (In Progress)
    *   Implement technical debt detection algorithms
    *   Create reporting system for issues
    *   Add architectural suggestion capabilities

### Phase 5: Advanced Features

17. **Testing & Optimization** (In Progress)
    *   Write unit and integration tests (In Progress)
    *   Optimize performance (In Progress)
    *   Address edge cases (In Progress)
    *   Address compiler warnings (In Progress)

18. **Extension System** (Planned)
    *   Create plugin architecture
    *   Design extension points
    *   Document extension development

19. **Safety & Security** (In Progress)
    *   Implement "yolo mode" with containerization (Planned)
    *   Add security scanning for generated code (Planned)
    *   Create permission management system (Completed for `run_command`)

## Implementation Details

### Module Structure

```
relay/
├── src/
│   ├── main.rs                # Entry point
│   ├── cli.rs                 # CLI interface
│   ├── config.rs              # Configuration management
│   ├── chain/                 # Chain system
│   │   ├── mod.rs
│   │   ├── executor.rs        # Chain execution
│   │   ├── state.rs           # Chain state
│   │   └── action.rs          # Action trait
│   ├── tools/                 # Action implementations
│   │   ├── mod.rs
│   │   ├── file.rs            # File operations
│   │   ├── repo.rs            # Repo operations
│   │   ├── search.rs          # Search operations
│   │   ├── edit.rs            # Edit operations
│   │   ├── command.rs         # Command execution
│   │   └── git.rs             # Git operations
│   ├── llm/                   # LLM integration
│   │   ├── mod.rs
│   │   ├── anthropic.rs       # Anthropic API client
│   │   ├── message.rs         # Message formatting
│   │   └── stream.rs          # Stream handling
│   ├── investigation/         # Investigation loops
│   │   ├── mod.rs
│   │   ├── explore.rs         # Code exploration
│   │   ├── modify.rs          # Code modification
│   │   ├── generate.rs        # Code generation
│   │   └── loop_detection.rs  # Loop detection
│   ├── advisors/              # Advisory systems
│   │   ├── mod.rs
│   │   ├── context.rs         # Context optimization
│   │   ├── technical_debt.rs  # Technical debt detection
│   │   └── bug_tracker.rs     # Bug tracking
│   └── utils/                 # Utility functions
│       ├── mod.rs
│       ├── formatting.rs      # Text formatting
│       └── error.rs           # Error handling
└── Cargo.toml
```

### Key Components

#### Action Chain System

The action chain system provides a flexible way to sequence operations, with each action capable of:

- Accepting input from previous steps or shared context
- Producing output for subsequent steps or shared context
- Handling errors and recovery
- Being composed into higher-level workflows
- **Action Dispatching**: Mechanism to parse LLM requests and map them to specific `Action` implementations with parameters.

```rust
pub trait Action {
    fn execute(&self, state: &mut ChainState) -> Result<()>; // Consider adding AppContext here
    fn name(&self) -> &'static str;
}

pub struct ChainState {
    pub context: HashMap<String, Value>,
    pub history: Vec<AnthropicMessage>, // Changed from HistoryEntry
    // ... other fields ...
}

pub struct ChainExecutor {
    actions: Vec<Box<dyn Action>>,
    // ...
}
```

#### Command Execution

The command execution system:
- Prompts the user for approval before running commands
- Captures command output for the agent to process
- Provides secure execution environments (containerized in "yolo mode")

```rust
pub struct CommandExecutor {
    pub approval_mode: ApprovalMode,
    // ...
}

pub enum ApprovalMode {
    Prompt,    // Ask the user before execution
    YoloMode,  // Execute without prompting (in containerized environment)
}
```

#### Context Advisor

The Context Advisor:
- Tracks which context items are actually used by the LLM
- Suggests optimizations to reduce token usage and costs
- Provides metrics on context efficiency
- Helps prevent context window overflow

```rust
pub struct ContextAdvisor {
    pub usage_metrics: HashMap<String, UsageMetric>,
    // ...
}

pub struct UsageMetric {
    pub usage_count: usize,
    pub token_count: usize,
    pub last_used: chrono::DateTime<chrono::Utc>,
    // ...
}
```

#### Investigation Loops

Investigation loops are implemented as specialized chains that follow patterns like:

1. **Exploration Loop**: Understanding code through iterative search and reading
2. **Modification Loop**: Making changes with validation and testing
3. **Generation Loop**: Creating new code with planning and implementation steps

#### Loop Detection System

The loop detection system:
- Monitors chain execution for repetitive patterns
- Detects when the agent is stuck in a loop
- Breaks out of loops and notifies the user
- Provides diagnostics on what caused the loop

```rust
pub struct LoopDetector {
    pub history: std::collections::VecDeque<String>, // Simplified: Use action names for now
    pub threshold: usize,
    // ...
}
```

#### Technical Debt Detection

The Technical Debt system:
- Analyzes codebases for architectural issues
- Identifies files that exceed size thresholds
- Detects design patterns that need refactoring
- Prompts users when problematic patterns are about to be introduced

#### Bug Tracker

The Bug Tracker:
- Maintains a list of identified bugs
- Allows users to add, prioritize, and resolve bugs
- Integrates with the agent's workflow for systematic resolution

#### VectorDB Integration

The integration with `vectordb-core` focuses on:

- Repository management (adding, switching, context)
- Semantic search across files and repositories
- Chunking and retrieval for context building

## User Experience Flow

1. User launches Relay with a request
2. Relay analyzes the request and selects an appropriate investigation loop
3. The loop executes, chaining actions like:
   - Repository context gathering
   - Semantic searching for relevant code
   - File reading for deeper understanding
   - LLM queries for generating responses or code
   - File editing based on generated results
   - Command execution (with user approval)
   - Git operations for version control
4. Results stream to the user in real-time
5. The process continues until completion or user intervention
6. Context Advisor provides optimization suggestions
7. Bug Tracker maintains a list of issues for future resolution

## Future Extensions

- **Multiple LLM Support**: Ability to use different LLM providers
- **Custom Tools**: User-defined actions for specialized workflows
- **Persistent Memory**: Long-term learning across sessions
- **Team Collaboration**: Sharing contexts and investigation results
- **Fully Autonomous Mode**: Safe, containerized environment for autonomous execution

## Technical Considerations

- Memory efficiency for large codebases
- Streaming for responsive UX
- Error handling and recovery for robust operation
- Security for code access and API credentials
- Rate limiting for API usage
- Command execution safety
- Git integration security
- Context optimization for token efficiency
