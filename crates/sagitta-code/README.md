# Fred Agent

**Fred Agent** is an advanced AI coding assistant built on top of [sagitta-search](../../README.md) that provides intelligent code interaction, repository management, and conversation handling capabilities. It combines the power of Google's Gemini LLM with semantic code search to deliver a superior development experience.

## 🚀 Features

### 🧠 Advanced Conversation Management
Fred Agent features a **revolutionary conversation management system** that surpasses traditional linear chat interfaces:

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
2. **ONNX Runtime**: GPU-enabled version recommended (see [main README](../../README.md#prerequisites))
3. **Qdrant**: Vector database (Docker recommended)
4. **Google Gemini API Key**: For LLM functionality

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

Fred Agent uses a layered configuration system:

1. **Core Configuration** (`~/.config/sagitta/config.toml`):
   - See [configuration.md](../../docs/configuration.md) for sagitta-search settings
   - Includes Qdrant URL, ONNX model paths, performance tuning

2. **Fred Agent Configuration** (`~/.config/sagitta-code/config.toml`):
   ```toml
   [gemini]
   api_key = "your-gemini-api-key"
   model = "gemini-1.5-pro"
   
   [agent]
   default_mode = "ToolsWithConfirmation"
   max_conversation_history = 100
   auto_save_conversations = true
   
   [ui]
   theme = "Dark"
   show_thinking = true
   enable_animations = true
   ```

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
    
    async fn execute(&self, parameters: serde_json::Value) -> Result<ToolResult, FredAgentError> {
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
Fred Agent is built on sagitta-search and inherits all its capabilities:
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

1. **ONNX Runtime not found**:
   ```bash
   export LD_LIBRARY_PATH=/path/to/onnxruntime/lib:$LD_LIBRARY_PATH
   ```

2. **Qdrant connection failed**:
   - Ensure Qdrant is running on the configured port
   - Check firewall settings

3. **Gemini API errors**:
   - Verify API key is correct
   - Check rate limits and quotas

4. **Indexing failures**:
   - Check file permissions
   - Verify repository accessibility
   - Monitor disk space

### Logging
Enable detailed logging:
```bash
RUST_LOG=fred_agent=debug ./sagitta-code
```

Access logs through the GUI logging panel (Ctrl+L) or check the console output.

## 📚 Related Documentation

- [sagitta-search README](../../README.md) - Core functionality and setup
- [Configuration Guide](../../docs/configuration.md) - Detailed configuration options
- [Conversation Management Plan](../../conversation-management-plan.md) - Implementation details

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

**Fred Agent** represents the next generation of AI-powered development tools, combining semantic understanding, intelligent conversation management, and comprehensive code analysis in a single, powerful application. 