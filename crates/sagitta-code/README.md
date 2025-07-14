# Sagitta Code

<!-- Do not update this file unless specifically asked to do so -->

**Sagitta Code** is an AI coding assistant built on top of the [sagitta-embed](../sagitta-embed) search engine. It provides intelligent code interaction, repository management, and conversation handling capabilities using Claude Code subscription.

Installation is currently a manual process, with future improvements to the install process being planned.

## Supported Languages

- Rust
- Python
- JavaScript
- TypeScript
- Go
- Ruby
- C++
- Markdown
- YAML
- HTML

## Backend Components

- **sagitta-embed**: Semantic search and embeddings
- **sagitta-search**: Core search functionality
- **code-parsers**: Language-specific code parsing
- **git-manager**: Git repository operations

## Prerequisites

1. **Rust Toolchain**: Install from [rustup.rs](https://rustup.rs/)
2. **ONNX Runtime**: Required for embedding generation. See [ONNX Runtime installation](../../README.md#prerequisites) in the main README
3. **Qdrant**: Vector database for semantic search
4. **Claude Code**: Install the Claude desktop app and authenticate with your subscription

## Installation

1. **Clone and build**:
   ```bash
   git clone https://gitlab.com/amulvany/sagitta.git
   cd sagitta
   # Build with GPU acceleration (see main README for all execution provider options)
   cargo build --release --features cuda
   ```
   
   **For other execution providers** (CoreML, ROCm, DirectML), see the [Execution Provider Support](../../README.md#5b-execution-provider-support) section in the main README.

2. **Start Qdrant**:
   ```bash
   docker run -d --name qdrant_db -p 6333:6333 -p 6334:6334 \
       -v $(pwd)/qdrant_storage:/qdrant/storage:z \
       qdrant/qdrant:latest
   ```

3. **Run the application**:
   ```bash
   ./target/release/sagitta-code
   ```

## GUI Features

- **Chat View**: Main conversation interface with AI assistant
- **Repository Panel**: Manage and sync code repositories
- **Settings Panel**: Configure LLM provider, models, and UI preferences
- **Conversation Sidebar**: Browse and manage conversation history
- **Events Panel**: View system events and tool execution logs
- **Preview Panel**: Display tool outputs and code changes
- **Analytics Panel**: Conversation statistics and usage metrics
- **Theme Customizer**: Customize UI colors and appearance
- **Model Selection**: Quick model switching

## Fast Model for Conversation Features

Sagitta Code now supports using a fast model (like Claude Haiku) for conversation management tasks:

- **Automatic Title Generation**: Generates descriptive titles after 2 messages
- **Smart Tag Suggestions**: Suggests relevant tags based on conversation content
- **Status Management**: Evaluates conversation status (Active, Completed, etc.)
- **Background Processing**: Non-blocking updates while you continue chatting

### Configuration

In Settings → Claude Code → Conversation Features:
- Enable/disable fast model usage
- Select which model to use (defaults to Claude Haiku)
- All features fall back to rule-based methods if fast model is unavailable

## Code Search

The semantic code search tool returns minimal output by default:
- File paths, line numbers, scores, and a one-line preview
- Full code content is not included to prevent context overflow
- Use `repository_view_file` tool to view specific code sections

## Configuration

### Core Configuration (`~/.config/sagitta/config.toml`)
Contains sagitta-search settings shared across all Sagitta tools. See [configuration.md](../../docs/configuration.md) for detailed options.

Note: Most of this can be configured in the settings menu of the GUI.

### Sagitta Code Configuration (`~/.config/sagitta/sagitta_code_config.json`)
Contains sagitta-code specific settings.  These are configured through the GUI.

### Data Storage
Following XDG Base Directory conventions:
- **Conversations**: `~/.local/share/sagitta/conversations/`
- **Repository Data**: `~/.local/share/sagitta/repositories/`

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](../../LICENSE-MIT) file for details.
