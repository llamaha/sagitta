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

5.  **File Operations** (In Progress)
    *   Implement file reading/viewing with pagination (Basic read implemented, context-aware path resolution done, needs partial read logic)
    *   Add file writing/editing capabilities (Basic write implemented, context-aware path resolution done)
    *   Create directory and file creation tools (Create directory implemented, context-aware path resolution done)
    *   Implement line editing action (Basic implementation done, context-aware path resolution done)

    **Testing Milestone: Basic File Interaction**
    *   **Capabilities:** Agent should be able to read, write, and create files/directories relative to its starting CWD. It should handle "file not found" errors gracefully and attempt basic commands (like `ls`) to gather context (pending user confirmation). Should handle conversational text around action JSON.
    *   **Example Test:** `relay "Create a file named test.txt with the content 'Hello Relay!' and then show me its content."` (Expect it to use `write_file` then `read_file`).

6.  **Repository Integration** (In Progress)
    *   Add repository initialization (Implemented - uses `git2` directly)
    *   Implement repository adding (URL/local path) (Implemented - currently shells out to `vectordb-cli`, **needs refactor to use library**) 
    *   Implement repository listing (Implemented - uses `vectordb-lib` function correctly)
    *   Implement repository removal (Implemented - currently shells out to `vectordb-cli`, **needs refactor to use library**) 
    *   Implement repository sync (Implemented - currently shells out to `vectordb-cli`, **needs refactor to use library**) 
    *   Set up context switching between repositories (Basic state added, `use_repo` action implemented - currently shells out to `vectordb-cli`, **needs refactor to use library**) 
    *   **NEXT:** Refactor add/remove/sync/use actions to call `vectordb-lib` functions directly. This may require modifying `vectordb-lib` functions to return structured data instead of only performing side effects and printing output.

7.  **VectorDB Integration**
    *   Connect to vectordb_lib's search functionality
    *   Set up semantic search for code exploration
    *   Implement semantic editing capabilities

8.  **Git Integration**
    *   Add git status checking
    *   Implement git history browsing
    *   Create git operations (commit, branch, etc.)

    **Testing Milestone: Repo & Search Integration**
    *   **Capabilities:** Agent should be able to add a local git repo, perform semantic searches within it using `vectordb_lib`, retrieve relevant code snippets, and potentially use `git status` (pending user confirmation). Context should switch to the active repository.
    *   **Example Test:** `relay "Add the repository at ./my-local-repo. Then find functions related to 'user authentication' in that repository."` (Expect `add_repo`, `use_repo`, then `semantic_search`).

### Phase 3: Agent Capabilities
*(Testing should be added for implemented agent capabilities and actions)*

9.  **Investigation Loops** (In Progress)
    *   Design investigation patterns
    *   Create loops for code understanding (MVP: Basic action loop implemented)
    *   Build software generation workflows
    *   Implement loop detection system to prevent infinite loops

10. **Context Management** (In Progress)
    *   Implement repository context for reference (MVP: Basic `current_directory` context implemented, `active_repository` added to state)
    *   Add multi-repository context capabilities
    *   Create context windowing for large codebases
    *   Develop Context Advisor for optimization

11. **Prompt Engineering** (In Progress)
    *   Design system prompts for various tasks (MVP: Basic action-request prompt implemented)
    *   Create templates for different operations
    *   Implement prompt construction logic

12. **Code Understanding**
    *   Implement code explanation system
    *   Add refactoring assistance
    *   Create bug detection capabilities

    **Testing Milestone: Basic Code Assistance**
    *   **Capabilities:** Agent should be able to answer simple questions about code found via semantic search, potentially explain snippets, and perform basic edits (like `line_edit` or `semantic_edit`) based on user requests within the context of an active repository.
    *   **Example Test:** `relay "In the active repository, find the 'login' function. Read its content and then replace line 10 with 'log.info(\\"User login attempted\\")'."` (Expect `semantic_search`, `read_file`, `line_edit`).

### Phase 4: User Experience
*(Testing should be added for CLI interactions and output)*

13. **CLI Refinement** (In Progress)
    *   Build user-friendly command structure (Basic subcommand structure via `vectordb-cli` exists, `relay` binary runnable directly)
    *   Add progress indicators
    *   Implement error handling and recovery (Basic action error reporting implemented)
    *   Implement User Confirmation for Dangerous Actions (e.g., `run_command`) (Next Step)

14. **Output Formatting** (Completed - Basic)
    *   Implement streaming text formatting (Basic streaming implemented)
    *   Add syntax highlighting
    *   Create summary views for results

--- **MVP Milestone (Target)** ---
*(Completion of core infrastructure, essential file/repo/search/command tools, basic investigation loop, basic context, basic CLI/output)*

15. **Documentation**
    *   Write user documentation
    *   Create examples
    *   Document internal architecture

16. **Technical Debt Tooling**
    *   Implement technical debt detection algorithms
    *   Create reporting system for issues
    *   Add architectural suggestion capabilities

### Phase 5: Advanced Features

17. **TUI Preparation**
    *   Design interface abstractions
    *   Create rendering framework
    *   Implement event handling system

18. **Testing & Optimization**
    *   Write unit and integration tests *(ensure comprehensive coverage for all features)* (Basic tests exist, need more coverage)
    *   Optimize performance
    *   Address edge cases
    *   Address compiler warnings

19. **Extension System**
    *   Create plugin architecture
    *   Design extension points
    *   Document extension development

20. **Safety & Security** (In Progress)
    *   Implement "yolo mode" with containerization
    *   Add security scanning for generated code
    *   Create permission management system (User confirmation for `run_command` is part of this)

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

The action chain system will provide a flexible way to sequence operations, with each action capable of:

- Accepting input from previous steps or shared context
- Producing output for subsequent steps or shared context
- Handling errors and recovery
- Being composed into higher-level workflows
- **Action Dispatching**: Needs a mechanism to parse LLM requests and map them to specific `Action` implementations with parameters.

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

The command execution system will:
- Prompt the user for approval before running commands
- Capture command output for the agent to process
- Provide secure execution environments (containerized in "yolo mode")

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

The Context Advisor will:
- Track which context items are actually used by the LLM
- Suggest optimizations to reduce token usage and costs
- Provide metrics on context efficiency
- Help prevent context window overflow

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

Investigation loops will be implemented as specialized chains that follow patterns like:

1. **Exploration Loop**: Understanding code through iterative search and reading
2. **Modification Loop**: Making changes with validation and testing
3. **Generation Loop**: Creating new code with planning and implementation steps

#### Loop Detection System

The loop detection system will:
- Monitor chain execution for repetitive patterns
- Detect when the agent is stuck in a loop
- Break out of loops and notify the user
- Provide diagnostics on what caused the loop

```rust
pub struct LoopDetector {
    pub history: std::collections::VecDeque<String>, // Simplified: Use action names for now
    pub threshold: usize,
    // ...
}
```

#### Technical Debt Detection

The Technical Debt system will:
- Analyze codebases for architectural issues
- Identify files that exceed size thresholds
- Detect design patterns that need refactoring
- Prompt users when problematic patterns are about to be introduced

#### Bug Tracker

The Bug Tracker will:
- Maintain a list of identified bugs
- Allow users to add, prioritize, and resolve bugs
- Integrate with the agent's workflow for systematic resolution

#### VectorDB Integration

The integration with vectordb_lib will focus on:

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

- **TUI Interface**: Rich terminal UI for better visualization and interaction
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
