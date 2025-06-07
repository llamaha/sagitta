# Conversation Panel Documentation

The Conversation Panel is a sophisticated sidebar component in Sagitta Code that provides intelligent organization, management, and navigation of your coding conversations. This document covers all features, organization modes, and usage instructions.

## Table of Contents

- [Overview](#overview)
- [Organization Modes](#organization-modes)
- [Advanced Features](#advanced-features)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Accessibility Features](#accessibility-features)
- [Performance Features](#performance-features)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)

## Overview

The Conversation Panel appears as a resizable sidebar on the left side of the Sagitta Code interface. It intelligently organizes your conversations using various modes, provides advanced search and filtering capabilities, and offers context-aware suggestions for branching and checkpoints.

### Key Features

- **6 Organization Modes**: Recency, Project, Status, Clusters, Tags, and Success
- **Smart Search**: Real-time search with debouncing for performance
- **Advanced Filtering**: Filter by status, features, tags, and more
- **Branch Suggestions**: Context-aware branching recommendations
- **Smart Checkpoints**: Automatic checkpoint suggestions at key moments
- **Responsive Design**: Adapts to different screen sizes
- **Accessibility**: Color-blind friendly palette and screen reader support
- **Persistent State**: Remembers your preferences across sessions

## Organization Modes

### 📅 Recency Mode

**Purpose**: Organizes conversations by when they were last active.

**How it works**:
- Groups conversations into time-based categories:
  - **Today**: Conversations active today
  - **Yesterday**: Conversations from yesterday
  - **This Week**: Conversations from the past 7 days
  - **This Month**: Conversations from the past 30 days
  - **Older**: Conversations older than 30 days

**Best for**: Quick access to recent work, daily workflow management.

**Keyboard Shortcut**: `Ctrl+1`

### 📁 Project Mode

**Purpose**: Organizes conversations by project workspace or repository.

**How it works**:
- Groups conversations by their associated workspace
- Shows workspace selector when active
- Displays conversations without workspaces in "No Workspace" group
- Can filter to show only conversations from active workspace

**Features**:
- **Workspace Selector**: ComboBox to switch between workspaces
- **"All Workspaces"**: Option to view conversations across all projects
- **Project Context**: Shows project-specific conversation patterns

**Best for**: Multi-project development, maintaining context separation between different codebases.

**Keyboard Shortcut**: `Ctrl+2`

### 📊 Status Mode

**Purpose**: Organizes conversations by their current status.

**How it works**:
- Groups conversations into status categories:
  - **🟢 Active**: Currently ongoing conversations
  - **⏸️ Paused**: Temporarily paused conversations
  - **✅ Completed**: Successfully completed conversations
  - **📦 Archived**: Archived conversations
  - **⏳ Summarizing**: Conversations being processed

**Status Indicators**:
- Each conversation shows a colored status icon
- Groups are ordered by priority (Active first)
- Shows count of conversations in each status

**Best for**: Project management, tracking conversation lifecycle, identifying stalled work.

**Keyboard Shortcut**: `Ctrl+3`

### 🔗 Clusters Mode

**Purpose**: Organizes conversations using semantic clustering based on content similarity.

**How it works**:
- Uses AI embeddings to group similar conversations
- Clusters are created based on:
  - Topic similarity
  - Code patterns
  - Problem domains
  - Solution approaches

**Features**:
- **Cohesion Score**: Shows how tightly related conversations in a cluster are
  - 🟢 High cohesion (>80%): Very related conversations
  - 🟠 Medium cohesion (60-80%): Moderately related
  - 🔴 Low cohesion (<60%): Loosely related
- **Common Tags**: Displays shared tags across cluster conversations
- **Breadcrumb Navigation**: "All → Clusters → [Cluster Name]"
- **Unclustered**: Shows conversations that don't fit into any cluster

**Best for**: Discovering patterns in your work, finding related conversations, understanding your coding themes.

**Keyboard Shortcut**: `Ctrl+4`

### 🏷️ Tags Mode

**Purpose**: Organizes conversations by their tags and topics.

**How it works**:
- Groups conversations by their assigned tags
- Tags can be:
  - **Manual**: Added by you
  - **Auto-suggested**: Proposed by AI based on content
  - **Rule-based**: Generated from patterns and keywords

**Features**:
- **Tag Frequency**: Most common tags appear first
- **Tag Management**: Accept/reject auto-suggested tags
- **Untagged Group**: Conversations without any tags
- **Tag Statistics**: Shows how many conversations use each tag

**Best for**: Topic-based organization, finding conversations about specific technologies or concepts.

**Keyboard Shortcut**: `Ctrl+5`

### ✅ Success Mode

**Purpose**: Organizes conversations by their success rate and completion status.

**How it works**:
- Groups conversations based on successful outcomes:
  - **Successful**: Conversations that led to working solutions
  - **In Progress**: Currently active conversations
  - **Other**: Conversations with unclear or unsuccessful outcomes

**Success Indicators**:
- Completion status as a proxy for success
- Future versions will include AI-analyzed success metrics
- Shows progress indicators for active conversations

**Best for**: Learning from successful patterns, identifying effective approaches, reviewing completed work.

**Keyboard Shortcut**: `Ctrl+6`

## Advanced Features

### 🔍 Smart Search

The conversation panel includes a powerful search system with the following features:

**Search Capabilities**:
- **Title Search**: Searches conversation titles
- **Tag Search**: Finds conversations with matching tags
- **Project Search**: Searches project names
- **Real-time Results**: Updates as you type
- **Case Insensitive**: Matches regardless of capitalization

**Performance Features**:
- **Debouncing**: 300ms delay prevents excessive searches while typing
- **Search Indicator**: ⏱ icon shows when search is debouncing
- **Clear Button**: ✖ button to quickly clear search

### 🎛️ Advanced Filtering

**Filter Options**:
- **Status Filters**: Active, Completed, Archived
- **Feature Filters**: 
  - Has branches
  - Has checkpoints
  - Favorites only
- **Date Range**: Filter by activity date (future feature)
- **Message Count**: Minimum number of messages
- **Success Rate**: Minimum success threshold (future feature)

### 🌿 Branch Suggestions

**Context-Aware Branching**:
- AI analyzes conversation context to suggest branching opportunities
- Suggestions appear as 🌳 badges next to conversations
- Color-coded by confidence:
  - 🟢 Green: High confidence (≥80%)
  - 🟡 Yellow: Medium confidence (≥60%)
  - 🟠 Orange: Lower confidence (<60%)

**Branch Reasons**:
- 🔀 **Alternative Approach**: Different solution path
- 🔧 **Error Recovery**: Recovering from errors
- ❓ **Exploration**: Investigating possibilities
- 🧩 **Experimentation**: Trying new approaches
- 🔄 **Iteration**: Refining solutions
- 🧪 **Testing**: Testing different scenarios
- 👤 **User Request**: User-initiated branching

**Actions**:
- **Create Branch**: Start new conversation branch
- **Dismiss**: Hide suggestion
- **Show Details**: View detailed reasoning
- **Refresh**: Update suggestions

### 📍 Smart Checkpoints

**Automatic Checkpoint Detection**:
- AI identifies key moments worth saving as checkpoints
- Suggestions appear as 📍 badges
- Reasons for checkpoint suggestions:
  - 🏆 **Major Breakthrough**: Significant progress
  - ✅ **Successful Solution**: Working implementation
  - ⚠️ **Before Major Change**: Before risky modifications
  - 🔄 **Iteration Complete**: End of development cycle
  - 🔧 **Working State**: Stable, functional state
  - 🎯 **Milestone Reached**: Project milestone
  - 👤 **User Requested**: Manual checkpoint request
  - 🤖 **Auto Suggested**: AI-recommended checkpoint
  - 🌳 **Before Branch**: Before creating branches

**Checkpoint Actions**:
- **Create Checkpoint**: Save current state
- **Restore Checkpoint**: Return to saved state
- **Jump to Checkpoint**: Navigate to checkpoint location
- **Show Details**: View checkpoint information

## Keyboard Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Ctrl+1` | Recency Mode | Switch to time-based organization |
| `Ctrl+2` | Project Mode | Switch to workspace-based organization |
| `Ctrl+3` | Status Mode | Switch to status-based organization |
| `Ctrl+4` | Clusters Mode | Switch to semantic clustering |
| `Ctrl+5` | Tags Mode | Switch to tag-based organization |
| `Ctrl+6` | Success Mode | Switch to success-based organization |

**Note**: Keyboard shortcuts can be disabled in configuration if needed.

## Accessibility Features

### 🎨 Color-Blind Friendly Palette

When enabled, the conversation panel uses a scientifically-designed color palette based on the Viridis color scheme:

- **Success**: Dark purple (`#440154`) instead of green
- **Warning**: Bright yellow (`#FDE725`)
- **Error**: Accessible green (`#5EC962`)
- **Info**: Teal (`#21918C`)
- **Primary**: Blue (`#3B528B`)
- **Secondary**: Gray (`#B4B4B4`)

### 🔊 Screen Reader Support

- **Announcements**: Important actions are announced to screen readers
- **Rate Limiting**: Announcements are limited to prevent spam (max 1 per 500ms)
- **Action Feedback**: Search actions, mode changes, and navigation are announced
- **Tooltip Support**: All interactive elements have descriptive tooltips

### ⌨️ Keyboard Navigation

- Full keyboard navigation support
- Tab order follows logical flow
- All actions accessible via keyboard
- Focus indicators for current selection

## Performance Features

### 🚀 Virtual Scrolling

For large conversation lists (1000+ conversations):
- Only visible items are rendered
- Smooth scrolling performance
- Configurable threshold (default: 1000 conversations)
- Memory-efficient handling of large datasets

### ⏱️ Search Debouncing

- 300ms delay prevents excessive search queries
- Visual indicator shows when debouncing is active
- Improves performance with large conversation lists
- Configurable debounce timing

### 💾 Caching System

- Rendered items are cached for performance
- Cache invalidation when data changes
- Reduces re-rendering overhead
- Optimized for frequent mode switching

## Configuration

### Persistent State

The conversation panel automatically saves and restores:

- **Organization Mode**: Last selected mode
- **Expanded Groups**: Which groups were expanded
- **Search Query**: Last search term
- **Filter Settings**: Active filters
- **UI State**: Panel visibility, accessibility settings

### Configuration File

Settings are stored in your Sagitta Code configuration file under `[conversation.sidebar]`:

```toml
[conversation.sidebar]
last_organization_mode = "Recency"
expanded_groups = ["today", "this_week"]
last_search_query = "rust async"
show_filters = false
show_branch_suggestions = true
show_checkpoint_suggestions = true
enable_accessibility = true
color_blind_friendly = false

[conversation.sidebar.filters]
project_types = ["Rust", "Python"]
statuses = ["Active"]
tags = ["important"]
min_messages = 5
favorites_only = false
branches_only = false
checkpoints_only = false

[conversation.sidebar.performance]
enable_virtual_scrolling = true
virtual_scrolling_threshold = 1000
search_debounce_ms = 300
```

### Responsive Configuration

```toml
[conversation.sidebar.responsive]
enabled = true
small_screen_breakpoint = 1366.0

[conversation.sidebar.responsive.compact_mode]
small_buttons = true
reduced_spacing = true
abbreviated_labels = true
hide_secondary_elements = false
```

## Troubleshooting

### Common Issues

**Conversations not appearing**:
- Check active filters - they might be hiding conversations
- Verify search query isn't too restrictive
- Ensure conversations are properly loaded

**Slow performance**:
- Enable virtual scrolling for large datasets
- Increase search debounce timing
- Check if clustering service is responsive

**State not persisting**:
- Verify configuration file permissions
- Check if auto-save is enabled
- Ensure proper shutdown (not force-killed)

**Keyboard shortcuts not working**:
- Check if shortcuts are enabled in configuration
- Verify no other application is capturing the shortcuts
- Ensure conversation panel has focus

### Performance Optimization

For optimal performance with large conversation lists:

1. **Enable Virtual Scrolling**: Set `enable_virtual_scrolling = true`
2. **Adjust Thresholds**: Lower `virtual_scrolling_threshold` if needed
3. **Increase Debounce**: Higher `search_debounce_ms` for slower systems
4. **Use Filters**: Apply filters to reduce visible conversations

### Getting Help

If you encounter issues:

1. Check the [troubleshooting guide](troubleshooting.md)
2. Review configuration file for syntax errors
3. Check application logs for error messages
4. Reset configuration to defaults if needed

---

## Future Enhancements

Planned features for future releases:

- **Analytics Integration**: Success rate calculations and metrics
- **Custom Organization**: User-defined organization rules
- **Advanced Search**: Full-text search across conversation content
- **Export/Import**: Backup and restore conversation data
- **Collaboration**: Shared conversations and workspaces
- **AI Insights**: Deeper analysis of conversation patterns

---

*This documentation covers Sagitta Code v0.1.0. Features and interfaces may change in future versions.* 