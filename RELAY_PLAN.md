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

17. **TUI Preparation** (Planned)
    *   Design interface abstractions
    *   Create rendering framework
    *   Implement event handling system

18. **Testing & Optimization** (In Progress)
    *   Write unit and integration tests (In Progress)
    *   Optimize performance (In Progress)
    *   Address edge cases (In Progress)
    *   Address compiler warnings (In Progress)

19. **Extension System** (Planned)
    *   Create plugin architecture
    *   Design extension points
    *   Document extension development

20. **Safety & Security** (In Progress)
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
