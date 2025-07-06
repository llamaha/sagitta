# Implement comprehensive auto-commit and sync system for sagitta-code

This commit adds a complete auto-commit and sync system inspired by Aider's workflow, ensuring repositories are always up-to-date with the latest changes.

## Features Implemented:

### 1. File Watching System
- Cross-platform file monitoring using `notify` crate
- Configurable debouncing to avoid excessive triggers
- Smart filtering to exclude build artifacts and git directories
- Repository-specific watching with event streaming

### 2. Auto-Commit System
- AI-powered commit message generation using FastModelProvider
- Automatic detection and staging of changes
- Configurable cooldown periods between commits
- Fallback commit messages when AI generation fails
- Proper git attribution for AI-generated commits

### 3. Repository Sync Integration
- Automatic syncing after commits (configurable)
- Sync hooks for repository add/switch events
- Queue management for multiple repositories
- Out-of-sync detection and status tracking

### 4. Git Workflow UI Controls
- Git controls component integrated into chat interface
- Branch/tag/ref selection with real-time switching
- Visual sync status indicators (syncing, out-of-sync, error)
- Advanced controls for creating branches and force syncing
- Channel-based async command system for UI thread safety

### 5. Sync Status Warnings
- Visual warnings in chat UI when repository is out of sync
- One-click sync button in warning display
- Real-time status updates every 5 seconds
- Color-coded indicators based on sync state

### 6. Auto-Sync Settings UI
- Complete configuration section in settings panel
- Toggle controls for all auto-sync features
- Debounce timing and cooldown configuration
- Per-feature enable/disable switches

### 7. Event-Driven Architecture
- RepositoryAdded and RepositorySwitched events
- Automatic sync triggering based on configuration
- Integration with existing repository manager

### 8. Service Initialization
- All services properly initialized in main app
- Background tasks for file watching and commit handling
- Proper error handling and logging throughout

## Configuration:
The system is fully configurable through settings with sensible defaults:
- File watcher debounce: 2 seconds (like Aider)
- Auto-commit cooldown: 30 seconds
- Sync after commit: enabled by default
- Sync on repo switch/add: enabled by default

## Architecture:
- Modular service design with clear separation of concerns
- Thread-safe async operations throughout
- Proper resource management with Arc/Mutex patterns
- Integration with existing MCP tools for syncing

This implementation ensures that sagitta-code maintains an always-current index of repository content, dramatically improving the AI assistant's ability to understand and work with code.