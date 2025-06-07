# Sagitta Code

**Sagitta Code** is an advanced AI coding assistant built on top of [sagitta-search](../../README.md) that provides intelligent code interaction, repository management, and conversation handling capabilities. It combines the power of Google's Gemini LLM with semantic code search to deliver a superior development experience.

## 🚀 Features

### 🧠 Advanced Conversation Management
Sagitta Code features a **revolutionary conversation management system** that surpasses traditional linear chat interfaces:

- **🌳 Context-Aware Branching**: Explore alternative solutions with intelligent conversation branches
- **📍 Smart Checkpoints**: Save and restore conversation states with full context snapshots
- **🔍 Semantic Clustering**: Automatically group related conversations using vector embeddings
- **📊 Conversation Analytics**: Track success metrics, patterns, and efficiency with comprehensive insights
- **🏢 Project Workspaces**: Auto-detect project types and maintain project-contextual conversations
- **🎯 Smart Organization**: Multiple organization modes (recency, project, status, clusters, tags, success)
- **🔎 Advanced Search**: Both text-based and semantic search across conversation history

### 🛠️ Comprehensive Tool Suite

#### Repository Management
- **Add Repository**: Clone and index repositories with automatic project detection
- **Sync Repository**: Update repositories and re-index changes
- **Remove Repository**: Clean removal with data cleanup
- **List Repositories**: View all managed repositories with status
- **Search Files**: Find files across repositories with glob patterns
- **View Files**: Display file contents with syntax highlighting

#### Code Operations
- **Semantic Code Search**: Natural language queries across your codebase
- **File Operations**: Read, write, and manipulate files with context awareness
- **Code Editing**: Apply precise code changes with validation
- **Semantic Editing**: AI-powered code modifications
- **Code Validation**: Verify changes before application
- **Shell Execution**: Run commands in secure Docker containers with language-specific environments
- **Test Execution**: Execute tests in isolated containers for Python, Rust, JavaScript, Go, and more

#### Web Integration
- **Web Search**: Fast web search for real-time information, git URLs, code examples, and documentation
- **Real-time Information**: Access current data beyond training cutoffs with source attribution

### 🎨 Modern GUI Interface

#### Chat Interface
- **Streaming Responses**: Real-time message streaming with thinking indicators
- **Tool Call Visualization**: See tool executions with detailed results
- **Message History**: Persistent conversation history with search
- **Syntax Highlighting**: Code blocks with proper language detection

#### Smart Sidebar
- **Multiple Organization Modes**: 
  - Recency (today, yesterday, this week, etc.)
  - Project-based grouping
  - Status-based organization
  - Semantic clusters
  - Tag-based filtering
  - Success rate sorting
- **Advanced Filtering**: Date ranges, message counts, branches, checkpoints
- **Visual Indicators**: Status badges, branch/checkpoint icons, success scores
- **Real-time Search**: Filter conversations by title, tags, or project

#### Visual Conversation Tree
- **Interactive Visualization**: Node-based conversation flow display
- **Branch Representation**: Visual branching with success indicators
- **Checkpoint Display**: Restoration points with context snapshots
- **Configurable Styling**: Colors, fonts, animations, spacing
- **Node Interactions**: Selection, expansion, highlighting

#### Repository Panel
- **Repository Management**: Add, sync, remove repositories
- **Status Monitoring**: Real-time indexing progress and health
- **Project Detection**: Automatic project type identification
- **Branch Management**: Switch between repository branches

#### Settings Panel
- **Theme Selection**: Multiple visual themes including Catppuccin
- **Configuration Management**: Adjust all agent settings
- **Tool Configuration**: Enable/disable specific tools
- **Performance Tuning**: Adjust indexing and search parameters

### 📊 Analytics & Insights

#### Success Metrics
- **Overall Success Rate**: Track conversation completion and effectiveness
- **Project-Specific Analysis**: Success rates by programming language/framework
- **Pattern Recognition**: Identify successful conversation flows
- **Efficiency Analysis**: Resolution times, branching efficiency, resource utilization

#### Trending Topics
- **Growth Analysis**: Track emerging topics and technologies
- **Success Correlation**: Which topics lead to successful outcomes
- **Project Association**: Link topics to specific project types

#### Recommendations
- **AI-Driven Suggestions**: Actionable recommendations for improvement
- **Process Optimization**: Identify inefficient patterns
- **Content Quality**: Suggestions for better conversation outcomes

## 🏗️ Architecture

### Core Components

```
sagitta-code/
├── agent/                    # Core agent implementation
│   ├── conversation/         # Advanced conversation management
│   │   ├── types.rs         # Conversation data structures
│   │   ├── manager.rs       # Conversation CRUD operations
│   │   ├── persistence/     # Disk-based storage
│   │   ├── search/          # Text and semantic search
│   │   ├── clustering.rs    # Semantic conversation clustering
│   │   ├── analytics.rs     # Comprehensive analytics
│   │   └── branching.rs     # Branch management
│   ├── message/             # Message handling
│   ├── state/               # Agent state management
│   └── core.rs              # Main agent implementation
├── tools/                   # Tool implementations
│   ├── repository/          # Repository management tools
│   ├── code_search/         # Semantic code search
│   ├── file_operations/     # File manipulation
│   ├── code_edit/           # Code editing tools
│   └── web_search.rs        # Web search integration
├── gui/                     # User interface
│   ├── app.rs               # Main application
│   ├── chat/                # Chat interface
│   ├── conversation/        # Conversation UI components
│   ├── repository/          # Repository management UI
│   ├── settings/            # Settings panel
│   └── theme/               # Theming system
├── llm/                     # LLM integration
│   └── gemini/              # Google Gemini client
├── project/                 # Project management
│   └── workspace/           # Workspace detection and management
├── config/                  # Configuration management
└── utils/                   # Utilities and helpers
```

### Key Technologies
- **Rust**: High-performance, memory-safe implementation
- **Tokio**: Async runtime for concurrent operations
- **egui**: Immediate mode GUI framework
- **Qdrant**: Vector database for semantic search
- **ONNX Runtime**: ML model inference for embeddings
- **Git2**: Git repository integration
- **Serde**: Serialization for configuration and persistence

## 🚀 Getting Started

### Prerequisites

1. **Rust Toolchain**: Install from [rustup.rs](https://rustup.rs/)
2. **Docker**: Required for secure containerized code execution (see [Docker Installation](#docker-installation))
3. **ONNX Runtime**: GPU-enabled version recommended (see [main README](../../README.md#prerequisites))
4. **Qdrant**: Vector database (Docker recommended)
5. **Google Gemini API Key**: For LLM functionality

#### Docker Installation

Docker is **mandatory** for Sagitta Code as it provides:
- **🔒 Security**: All code execution happens in isolated containers
- **🌍 Consistency**: Same environment across all platforms
- **🛡️ Isolation**: Protects your host system from potentially unsafe code
- **📦 Dependencies**: Pre-configured language environments

##### Windows
1. Download [Docker Desktop for Windows](https://docs.docker.com/desktop/setup/install/windows-install/)
2. Requirements: Windows 10/11 64-bit (Pro, Enterprise, or Education)
3. Enable WSL 2 (recommended) or Hyper-V in BIOS
4. Install and restart your system
5. Start Docker Desktop from the Start menu

##### macOS
1. Download [Docker Desktop for Mac](https://docs.docker.com/desktop/setup/install/mac-install/)
2. Choose the correct version:
   - **Apple Silicon (M1/M2)**: Docker Desktop for Mac with Apple silicon
   - **Intel**: Docker Desktop for Mac with Intel chip
3. Install by dragging Docker.app to Applications folder
4. Start Docker from Applications

##### Linux
1. Install Docker Engine: [Installation Guide](https://docs.docker.com/engine/install/)
2. Add your user to docker group: `sudo usermod -aG docker $USER`
3. Log out and back in for group changes to take effect
4. Start Docker service: `sudo systemctl start docker`
5. Enable auto-start: `sudo systemctl enable docker`

**Quick Linux Install:**
```bash
# Ubuntu/Debian
sudo apt update && sudo apt install docker.io docker-compose
sudo usermod -aG docker $USER

# RHEL/CentOS/Fedora  
sudo dnf install docker docker-compose
sudo systemctl start docker && sudo systemctl enable docker
```

### Installation

1. **Clone the repository**:
   ```bash
   git clone <repository-url>
   cd sagitta-search/crates/sagitta-code
   ```

2. **Build the application**:
   ```bash
   # With GUI (default)
   cargo build --release
   
   # CLI only
   cargo build --release --no-default-features
   ```

3. **Set up dependencies**:
   ```bash
   # Start Qdrant
   docker run -d --name qdrant_db -p 6333:6333 -p 6334:6334 \
       -v $(pwd)/qdrant_storage:/qdrant/storage:z \
       qdrant/qdrant:latest
   
   # Set up ONNX Runtime (see main README for details)
   export LD_LIBRARY_PATH=~/onnxruntime/onnxruntime-linux-x64-1.20.0/lib:$LD_LIBRARY_PATH
   ```

### Configuration

Sagitta Code uses a unified configuration system that shares the core Sagitta namespace:

#### Configuration Files

1. **Shared Core Configuration** (`~/.config/sagitta/config.toml`):
   - Contains all sagitta-search settings (Qdrant, ONNX models, repositories, performance)
   - Shared by sagitta-cli, sagitta-mcp, and sagitta-code
   - See [configuration.md](../../docs/configuration.md) for detailed options

2. **Sagitta Code Configuration** (`~/.config/sagitta/sagitta_code_config.json`):
   - Contains sagitta-code specific settings (Gemini API, UI preferences, conversation management)
   - Example configuration:
   ```json
   {
     "gemini": {
       "api_key": "your-gemini-api-key",
       "model": "gemini-2.5-flash-preview-05-20",
       "max_history_size": 20,
       "max_reasoning_steps": 50
     },
     "ui": {
       "dark_mode": true,
       "theme": "default",
       "window_width": 900,
       "window_height": 700
     },
     "logging": {
       "log_level": "info",
       "log_to_file": false,
       "log_file_path": null
     },
     "conversation": {
       "storage_path": null,
       "auto_save": true,
       "auto_create": true,
       "max_conversations": 100,
       "auto_cleanup_days": 30,
       "auto_checkpoints": true,
       "auto_branching": false,
       "default_tags": []
     }
   }
   ```

#### Data Storage

Following XDG Base Directory conventions:

- **Conversations**: `~/.local/share/sagitta/conversations/`
- **Logs**: `~/.local/share/sagitta/logs/` (if file logging enabled)
- **Repository Data**: `~/.local/share/sagitta/repositories/` (unless custom path specified)

#### Migration from Previous Versions

Sagitta Code automatically migrates configuration from the old locations:
- `~/.config/sagitta_code/sagitta_code_config.json` → `~/.config/sagitta/sagitta_code_config.json`
- `~/.config/sagitta_code/core_config.toml` → `~/.config/sagitta/config.toml`

The migration runs automatically on first startup and will remove the old directory if it's empty after migration.

### First Run

1. **Start the application**:
   ```bash
   ./target/release/sagitta-code
   ```

2. **Configure your first repository**:
   - Use the Repository panel to add your codebase
   - Wait for indexing to complete
   - Start asking questions about your code!

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| **`Ctrl+T`** | Toggle Conversation Management Panel |
| **`Ctrl+R`** | Toggle Repository Panel |
| **`Ctrl+W`** | Toggle Preview Panel |
| **`Ctrl+S`** | Toggle Settings Panel |
| **`Ctrl+L`** | Toggle Log Panel |
| **`Enter`** | Send Message |
| **`Ctrl+Enter`** | New Line in Chat |
| **`?`** | Show Hotkeys Modal |

## 🎯 Usage Examples

### Accessing Conversation Management Features

**NEW**: Press **`Ctrl+T`** to open the Conversation Management panel! This gives you access to:
- Organization mode selection (Recency, Project, Status, Clusters, Tags, Success)
- Overview of available conversation management features
- Status of implementation and next steps

> **Note**: The conversation management system is fully implemented in the backend but is currently being integrated with the GUI. The panel shows what's available and the current implementation status.

### Basic Code Questions
```
"How does authentication work in this project?"
"Show me all the database models"
"Find functions that handle user registration"
```

### Code Modifications
```
"Add error handling to the login function"
"Refactor this class to use dependency injection"
"Add unit tests for the payment processing module"
```

### Project Analysis
```
"What are the main architectural patterns used here?"
"Find potential security vulnerabilities"
"Show me the most complex functions that need refactoring"
```

### Conversation Management
- **Branch conversations** when exploring alternatives
- **Create checkpoints** before major changes
- **Search conversation history** for previous solutions
- **Analyze patterns** to improve development workflow

## ⚙️ Configuration Reference

### Agent Modes
- **ChatOnly**: Text responses only, no tool execution
- **ToolsWithConfirmation**: Ask before executing tools (default)
- **FullyAutonomous**: Execute tools automatically

### Performance Tuning
See [configuration.md](../../docs/configuration.md#performance-tuning-guide) for detailed performance optimization including:
- GPU memory management
- Parallel processing configuration
- Embedding batch sizes
- Qdrant upload optimization

### Tool Configuration
Individual tools can be enabled/disabled and configured through the settings panel or configuration files.

## 🔧 Development

### Building from Source
```bash
# Development build with all features
cargo build --features gui

# Release build
cargo build --release --features gui

# CLI only
cargo build --release --no-default-features
```

### Running Tests
```bash
# Run all tests
cargo test

# Run conversation management tests specifically
cargo test conversation

# Run with logging
RUST_LOG=debug cargo test

# Test shell execution functionality
cargo run --bin test_shell_execution
```

### Adding Custom Tools
1. Implement the `Tool` trait
2. Register with the `ToolRegistry`
3. Add UI integration if needed

Example:
```rust
use async_trait::async_trait;
use crate::tools::types::{Tool, ToolDefinition, ToolResult};

#[derive(Debug)]
pub struct CustomTool;

#[async_trait]
impl Tool for CustomTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "custom_tool".to_string(),
            description: "A custom tool".to_string(),
            // ... other fields
        }
    }
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, SagittaCodeError> {
        // Implementation
        Ok(ToolResult::Success(serde_json::json!({"result": "success"})))
    }
}
```

## 📊 Analytics & Monitoring

### Conversation Analytics
Access comprehensive analytics through the GUI or programmatically:
- Success rates by project type
- Common conversation patterns
- Efficiency metrics
- Trending topics
- Anomaly detection

### Performance Monitoring
- Real-time indexing progress
- Search performance metrics
- Memory usage tracking
- Tool execution statistics

## 🤝 Integration

### With sagitta-search
Sagitta Code is built on sagitta-search and inherits all its capabilities:
- Semantic code search
- Repository indexing
- Vector embeddings
- Qdrant integration

### With External Tools
- **Git**: Repository management and version control
- **Web Search**: Real-time information retrieval
- **File System**: Direct file operations
- **Code Editors**: Integration possibilities

## 🐛 Troubleshooting

### Common Issues

1. **Docker is not available**:
   Docker is required for secure code execution. See [Docker Installation](#docker-installation) above.
   
   **Symptoms**: 
   - "Docker is required for secure containerized execution but is not installed"
   - Shell execution or test execution fails immediately
   
   **Solutions**:
   - Install Docker Desktop (Windows/Mac) or Docker Engine (Linux)
   - Start Docker service: `sudo systemctl start docker` (Linux)
   - Verify installation: `docker --version && docker run hello-world`

2. **Docker daemon not running**:
   ```bash
   # Check Docker status
   docker info
   
   # Start Docker (Linux)
   sudo systemctl start docker
   
   # Start Docker Desktop (Windows/Mac)
   # Use the system tray or Applications menu
   ```

3. **Shell/Test execution timeouts**:
   Large Docker images may take time to pull on first use.
   
   **Solutions**:
   - Wait for initial Docker image pulls to complete
   - Check network connectivity
   - Pre-pull images: `docker pull python:3.11`, `docker pull rust:1.75`

4. **ONNX Runtime not found**:
   ```bash
   export LD_LIBRARY_PATH=/path/to/onnxruntime/lib:$LD_LIBRARY_PATH
   ```

5. **Qdrant connection failed**:
   - Ensure Qdrant is running on the configured port
   - Check firewall settings
   - Restart Qdrant container if needed

6. **Gemini API errors**:
   - Verify API key is correct
   - Check rate limits and quotas
   - Ensure network connectivity

7. **Indexing failures**:
   - Check file permissions
   - Verify repository accessibility
   - Monitor disk space

### Logging
Enable detailed logging:
```bash
RUST_LOG=sagitta_code=debug ./sagitta-code
```

Access logs through the GUI logging panel (Ctrl+L) or check the console output.

### Testing Your Setup

Run the comprehensive test suite to verify everything works:

```bash
# Test shell execution (requires Docker)
cargo run --bin test_shell_execution

# Run all unit tests
cargo test

# Run integration tests  
cargo test --test shell_execution_integration -- --ignored
```

## 📚 Related Documentation

- [sagitta-search README](../../README.md) - Core functionality and setup
- [Configuration Guide](../../docs/configuration.md) - Detailed configuration options
- [Conversation Management Plan](../../conversation-management-plan.md) - Implementation details
- [Troubleshooting Guide](docs/troubleshooting.md) - Detailed troubleshooting steps

## 🔮 Future Enhancements

- **Task Integration**: Convert conversations to actionable tasks
- **Advanced Navigation**: Enhanced code-aware search
- **Multi-LLM Support**: Support for additional language models
- **Plugin System**: Extensible architecture for custom tools
- **Collaboration Features**: Multi-user conversation sharing
- **IDE Integration**: Direct integration with popular editors

## 📄 License

This project is licensed under the MIT License - see the [LICENSE-MIT](../../LICENSE-MIT) file for details.

---

**Sagitta Code** represents the next generation of AI-powered development tools, combining semantic understanding, intelligent conversation management, and comprehensive code analysis in a single, powerful application.

## Recent Fixes

### Sync Panel Repository Display (Fixed)

**Issue**: The sync panel in the GUI wasn't displaying repositories for syncing, showing "No repositories available" even when repositories were configured.

**Root Cause**: The sync panel was only checking the basic `repositories` list but not the enhanced `enhanced_repositories` list, which is populated when enhanced repository information is available.

**Fix**: 
- Modified the sync panel to check both enhanced and basic repository lists
- Added automatic fallback from enhanced to basic repositories when enhanced list is empty
- Added refresh button and automatic refresh triggering when no repositories are displayed
- Added comprehensive test coverage to prevent regression

**Files Changed**:
- `crates/sagitta-code/src/gui/repository/sync.rs` - Enhanced repository detection logic
- `crates/sagitta-code/src/gui/repository/panel.rs` - Auto-refresh for sync tab
- Added integration tests to ensure sync panel always shows available repositories

### Sync Panel Refresh Loop (Fixed)

**Issue**: The sync panel was stuck in an infinite refresh loop, continuously refreshing repositories every second and causing "Repository manager is locked" errors.

**Root Cause**: The auto-refresh logic was triggering continuously because it wasn't properly tracking whether an initial load attempt had been made.

**Fix**:
- Fixed auto-refresh condition to only trigger once when no repositories are available
- Added `initial_load_attempted` flag to prevent infinite refresh loops
- Improved repository manager lock contention handling with better error messages
- Enhanced UI layout to prevent text overlap between repository names and branch fields

**Files Changed**:
- `crates/sagitta-code/src/gui/repository/panel.rs` - Fixed refresh loop logic
- `crates/sagitta-code/src/gui/repository/sync.rs` - Improved UI layout and lock handling
- Added test to verify infinite loop prevention works correctly