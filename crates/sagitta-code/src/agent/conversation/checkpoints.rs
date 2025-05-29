// Smart conversation checkpoints with context awareness

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::agent::conversation::types::{Conversation, ConversationCheckpoint, ContextSnapshot};
use crate::agent::message::types::AgentMessage;
use crate::llm::client::Role;

/// Smart checkpoint manager for conversation state management
pub struct ConversationCheckpointManager {
    /// Configuration for checkpoint behavior
    config: CheckpointConfig,
    
    /// Checkpoint importance analyzer
    importance_analyzer: CheckpointImportanceAnalyzer,
    
    /// Context snapshot generator
    context_generator: ContextSnapshotGenerator,
}

/// Configuration for checkpoint management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Enable automatic checkpoint creation
    pub auto_checkpoint_enabled: bool,
    
    /// Minimum importance threshold for auto-checkpoints
    pub auto_checkpoint_threshold: f32,
    
    /// Maximum number of checkpoints per conversation
    pub max_checkpoints_per_conversation: usize,
    
    /// Interval between automatic checkpoint evaluations (in messages)
    pub auto_checkpoint_interval: usize,
    
    /// Whether to include file states in snapshots
    pub include_file_states: bool,
    
    /// Whether to include repository states in snapshots
    pub include_repository_states: bool,
    
    /// Whether to include environment variables in snapshots
    pub include_environment: bool,
    
    /// Maximum file size to include in snapshots (in bytes)
    pub max_file_size_bytes: usize,
    
    /// File patterns to exclude from snapshots
    pub excluded_file_patterns: Vec<String>,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            auto_checkpoint_enabled: true,
            auto_checkpoint_threshold: 0.7,
            max_checkpoints_per_conversation: 10,
            auto_checkpoint_interval: 5,
            include_file_states: true,
            include_repository_states: true,
            include_environment: false, // Privacy consideration
            max_file_size_bytes: 1024 * 1024, // 1MB
            excluded_file_patterns: vec![
                "*.log".to_string(),
                "*.tmp".to_string(),
                "target/*".to_string(),
                "node_modules/*".to_string(),
                ".git/*".to_string(),
                "*.exe".to_string(),
                "*.dll".to_string(),
                "*.so".to_string(),
            ],
        }
    }
}

/// Checkpoint suggestion with importance analysis
#[derive(Debug, Clone)]
pub struct CheckpointSuggestion {
    /// Suggested checkpoint location (message ID)
    pub message_id: Uuid,
    
    /// Importance score for this checkpoint (0.0-1.0)
    pub importance: f32,
    
    /// Reason for suggesting this checkpoint
    pub reason: CheckpointReason,
    
    /// Suggested checkpoint title
    pub suggested_title: String,
    
    /// Context that led to this suggestion
    pub context: CheckpointContext,
    
    /// Estimated restoration value
    pub restoration_value: f32,
}

/// Reasons for creating checkpoints
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointReason {
    /// Major milestone or breakthrough achieved
    MajorMilestone,
    
    /// Successful solution or implementation
    SuccessfulSolution,
    
    /// Before attempting risky or experimental approach
    BeforeRiskyOperation,
    
    /// After significant context change
    ContextChange,
    
    /// Before major refactoring or changes
    BeforeRefactoring,
    
    /// After completing a complex task
    TaskCompletion,
    
    /// User explicitly requested checkpoint
    UserRequested,
    
    /// Periodic automatic checkpoint
    PeriodicAutomatic,
    
    /// Before switching conversation branches
    BeforeBranching,
}

/// Context information for checkpoint suggestions
#[derive(Debug, Clone)]
pub struct CheckpointContext {
    /// Messages that influenced this suggestion
    pub relevant_messages: Vec<Uuid>,
    
    /// Keywords or phrases that triggered the suggestion
    pub trigger_keywords: Vec<String>,
    
    /// Current conversation phase
    pub conversation_phase: ConversationPhase,
    
    /// Files that have been modified recently
    pub modified_files: Vec<PathBuf>,
    
    /// Tools or commands that were executed
    pub executed_tools: Vec<String>,
    
    /// Success indicators detected
    pub success_indicators: Vec<String>,
}

/// Current phase of the conversation for checkpoint analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationPhase {
    /// Initial problem definition
    ProblemDefinition,
    
    /// Research and exploration
    Research,
    
    /// Solution design
    Design,
    
    /// Implementation
    Implementation,
    
    /// Testing and validation
    Testing,
    
    /// Debugging and troubleshooting
    Debugging,
    
    /// Optimization and refinement
    Optimization,
    
    /// Documentation and cleanup
    Documentation,
    
    /// Project completion
    Completion,
}

/// Checkpoint importance analyzer
pub struct CheckpointImportanceAnalyzer {
    /// Keywords that indicate important moments
    milestone_keywords: Vec<String>,
    
    /// Success indicators
    success_indicators: Vec<String>,
    
    /// Risk indicators
    risk_indicators: Vec<String>,
    
    /// Completion indicators
    completion_indicators: Vec<String>,
}

impl Default for CheckpointImportanceAnalyzer {
    fn default() -> Self {
        Self {
            milestone_keywords: vec![
                "breakthrough".to_string(),
                "solved".to_string(),
                "working".to_string(),
                "success".to_string(),
                "completed".to_string(),
                "finished".to_string(),
                "achieved".to_string(),
                "milestone".to_string(),
                "progress".to_string(),
                "accomplished".to_string(),
            ],
            success_indicators: vec![
                "tests pass".to_string(),
                "compilation successful".to_string(),
                "no errors".to_string(),
                "works correctly".to_string(),
                "as expected".to_string(),
                "perfect".to_string(),
                "excellent".to_string(),
                "great job".to_string(),
                "well done".to_string(),
            ],
            risk_indicators: vec![
                "experimental".to_string(),
                "risky".to_string(),
                "might break".to_string(),
                "not sure".to_string(),
                "dangerous".to_string(),
                "careful".to_string(),
                "backup".to_string(),
                "save state".to_string(),
                "before we".to_string(),
            ],
            completion_indicators: vec![
                "done".to_string(),
                "complete".to_string(),
                "finished".to_string(),
                "ready".to_string(),
                "deployed".to_string(),
                "shipped".to_string(),
                "released".to_string(),
                "final".to_string(),
            ],
        }
    }
}

/// Context snapshot generator
pub struct ContextSnapshotGenerator {
    /// Configuration for snapshot generation
    config: SnapshotConfig,
}

/// Configuration for context snapshots
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Maximum number of files to include
    pub max_files: usize,
    
    /// Maximum total snapshot size (in bytes)
    pub max_total_size: usize,
    
    /// Whether to compress snapshots
    pub compress_snapshots: bool,
    
    /// Whether to include binary files
    pub include_binary_files: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            max_files: 100,
            max_total_size: 10 * 1024 * 1024, // 10MB
            compress_snapshots: true,
            include_binary_files: false,
        }
    }
}

impl ConversationCheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(config: CheckpointConfig) -> Self {
        Self {
            config,
            importance_analyzer: CheckpointImportanceAnalyzer::default(),
            context_generator: ContextSnapshotGenerator::new(SnapshotConfig::default()),
        }
    }
    
    /// Create checkpoint manager with default configuration
    pub fn with_default_config() -> Self {
        Self::new(CheckpointConfig::default())
    }
    
    /// Analyze conversation for potential checkpoint opportunities
    pub fn analyze_checkpoint_opportunities(&self, conversation: &Conversation) -> Result<Vec<CheckpointSuggestion>> {
        if !self.config.auto_checkpoint_enabled {
            return Ok(Vec::new());
        }
        
        let mut suggestions = Vec::new();
        
        // Check if we should evaluate for checkpoints based on interval
        if conversation.messages.len() % self.config.auto_checkpoint_interval != 0 {
            return Ok(suggestions);
        }
        
        // Analyze recent messages for checkpoint opportunities
        let recent_messages = self.get_recent_messages(conversation);
        
        for (i, message) in recent_messages.iter().enumerate() {
            if let Some(suggestion) = self.analyze_message_for_checkpoint(message, &recent_messages, i, conversation)? {
                if suggestion.importance >= self.config.auto_checkpoint_threshold {
                    suggestions.push(suggestion);
                }
            }
        }
        
        // Sort by importance (highest first)
        suggestions.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
        
        // Limit to reasonable number of suggestions
        suggestions.truncate(3);
        
        Ok(suggestions)
    }
    
    /// Get recent messages for analysis
    fn get_recent_messages<'a>(&self, conversation: &'a Conversation) -> Vec<&'a AgentMessage> {
        let window_size = 10; // Analyze last 10 messages
        let start_index = conversation.messages.len().saturating_sub(window_size);
        conversation.messages[start_index..].iter().collect()
    }
    
    /// Analyze a single message for checkpoint opportunities
    fn analyze_message_for_checkpoint(
        &self,
        message: &AgentMessage,
        context_messages: &[&AgentMessage],
        message_index: usize,
        conversation: &Conversation,
    ) -> Result<Option<CheckpointSuggestion>> {
        let content = &message.content.to_lowercase();
        
        let mut importance: f32 = 0.0;
        let mut reasons = Vec::new();
        let mut trigger_keywords = Vec::new();
        
        // Check for milestone keywords
        for keyword in &self.importance_analyzer.milestone_keywords {
            if content.contains(keyword) {
                importance += 0.3;
                reasons.push(CheckpointReason::MajorMilestone);
                trigger_keywords.push(keyword.clone());
            }
        }
        
        // Check for success indicators
        for indicator in &self.importance_analyzer.success_indicators {
            if content.contains(indicator) {
                importance += 0.4;
                reasons.push(CheckpointReason::SuccessfulSolution);
                trigger_keywords.push(indicator.clone());
            }
        }
        
        // Check for risk indicators (suggest checkpoint before risky operations)
        for indicator in &self.importance_analyzer.risk_indicators {
            if content.contains(indicator) {
                importance += 0.3;
                reasons.push(CheckpointReason::BeforeRiskyOperation);
                trigger_keywords.push(indicator.clone());
            }
        }
        
        // Check for completion indicators
        for indicator in &self.importance_analyzer.completion_indicators {
            if content.contains(indicator) {
                importance += 0.4;
                reasons.push(CheckpointReason::TaskCompletion);
                trigger_keywords.push(indicator.clone());
            }
        }
        
        // Analyze conversation phase
        let conversation_phase = self.analyze_conversation_phase(context_messages);
        
        // Adjust importance based on conversation phase
        match conversation_phase {
            ConversationPhase::Implementation => importance += 0.2,
            ConversationPhase::Testing => importance += 0.3,
            ConversationPhase::Completion => importance += 0.4,
            ConversationPhase::Debugging => importance += 0.1,
            _ => {}
        }
        
        // Check for tool usage (indicates action taken)
        if !message.tool_calls.is_empty() {
            importance += 0.2;
            reasons.push(CheckpointReason::ContextChange);
        }
        
        // Check if this is after a long conversation (periodic checkpoint)
        if conversation.messages.len() > 20 && conversation.messages.len() % 20 == 0 {
            importance += 0.1;
            reasons.push(CheckpointReason::PeriodicAutomatic);
        }
        
        if importance < 0.3 {
            return Ok(None);
        }
        
        // Determine primary reason
        let primary_reason = reasons.into_iter().next().unwrap_or(CheckpointReason::PeriodicAutomatic);
        
        // Generate suggested title
        let suggested_title = self.generate_checkpoint_title(&primary_reason, &trigger_keywords, conversation_phase);
        
        // Calculate restoration value
        let restoration_value = self.calculate_restoration_value(message, context_messages, &primary_reason);
        
        // Build context
        let context = CheckpointContext {
            relevant_messages: context_messages.iter().map(|m| m.id).collect(),
            trigger_keywords,
            conversation_phase,
            modified_files: self.detect_modified_files(context_messages),
            executed_tools: self.extract_executed_tools(context_messages),
            success_indicators: self.extract_success_indicators(content),
        };
        
        Ok(Some(CheckpointSuggestion {
            message_id: message.id,
            importance: importance.min(1.0),
            reason: primary_reason,
            suggested_title,
            context,
            restoration_value,
        }))
    }
    
    /// Analyze the current phase of the conversation
    fn analyze_conversation_phase(&self, messages: &[&AgentMessage]) -> ConversationPhase {
        if messages.is_empty() {
            return ConversationPhase::ProblemDefinition;
        }
        
        let recent_content: String = messages.iter()
            .rev()
            .take(5)
            .map(|m| m.content.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        
        // Check for completion phase
        if recent_content.contains("done") || recent_content.contains("complete") || recent_content.contains("finished") {
            return ConversationPhase::Completion;
        }
        
        // Check for testing phase
        if recent_content.contains("test") || recent_content.contains("verify") || recent_content.contains("validate") {
            return ConversationPhase::Testing;
        }
        
        // Check for debugging phase
        if recent_content.contains("debug") || recent_content.contains("error") || recent_content.contains("fix") {
            return ConversationPhase::Debugging;
        }
        
        // Check for implementation phase
        if recent_content.contains("implement") || recent_content.contains("code") || recent_content.contains("build") {
            return ConversationPhase::Implementation;
        }
        
        // Check for design phase
        if recent_content.contains("design") || recent_content.contains("plan") || recent_content.contains("architecture") {
            return ConversationPhase::Design;
        }
        
        // Check for research phase
        if recent_content.contains("research") || recent_content.contains("investigate") || recent_content.contains("explore") {
            return ConversationPhase::Research;
        }
        
        // Check for documentation phase
        if recent_content.contains("document") || recent_content.contains("readme") || recent_content.contains("comment") {
            return ConversationPhase::Documentation;
        }
        
        // Default to problem definition
        ConversationPhase::ProblemDefinition
    }
    
    /// Generate a suggested checkpoint title
    fn generate_checkpoint_title(&self, reason: &CheckpointReason, keywords: &[String], phase: ConversationPhase) -> String {
        match reason {
            CheckpointReason::MajorMilestone => {
                if let Some(keyword) = keywords.first() {
                    format!("Milestone: {}", keyword.to_title_case())
                } else {
                    "Major Milestone Reached".to_string()
                }
            },
            CheckpointReason::SuccessfulSolution => "Successful Solution".to_string(),
            CheckpointReason::BeforeRiskyOperation => "Before Risky Operation".to_string(),
            CheckpointReason::ContextChange => "Context Change".to_string(),
            CheckpointReason::BeforeRefactoring => "Before Refactoring".to_string(),
            CheckpointReason::TaskCompletion => "Task Completed".to_string(),
            CheckpointReason::UserRequested => "User Checkpoint".to_string(),
            CheckpointReason::PeriodicAutomatic => format!("Auto Checkpoint - {:?}", phase),
            CheckpointReason::BeforeBranching => "Before Branching".to_string(),
        }
    }
    
    /// Calculate the restoration value of a checkpoint
    fn calculate_restoration_value(&self, message: &AgentMessage, context: &[&AgentMessage], reason: &CheckpointReason) -> f32 {
        let mut value = 0.5; // Base value
        
        // Adjust based on reason
        match reason {
            CheckpointReason::SuccessfulSolution => value += 0.4,
            CheckpointReason::MajorMilestone => value += 0.3,
            CheckpointReason::TaskCompletion => value += 0.3,
            CheckpointReason::BeforeRiskyOperation => value += 0.2,
            CheckpointReason::ContextChange => value += 0.1,
            _ => {}
        }
        
        // Adjust based on message characteristics
        if !message.tool_calls.is_empty() {
            value += 0.2; // Tool usage indicates important state
        }
        
        // Adjust based on context length (more context = higher value)
        let context_factor = (context.len() as f32 / 10.0).min(1.0) * 0.1;
        value += context_factor;
        
        value.clamp(0.0, 1.0)
    }
    
    /// Detect modified files from conversation context
    fn detect_modified_files(&self, messages: &[&AgentMessage]) -> Vec<PathBuf> {
        let mut files = Vec::new();
        
        for message in messages {
            // Look for file paths in message content
            let content = &message.content;
            
            // Simple heuristic: look for common file extensions and patterns
            let file_patterns = vec![".rs", ".py", ".js", ".ts", ".go", ".java", ".cpp", ".c", ".h", ".md", ".txt", ".json", ".yaml", ".yml", ".toml"];
            
            for pattern in file_patterns {
                if content.contains(pattern) {
                    // Extract potential file paths (simplified)
                    for word in content.split_whitespace() {
                        if word.contains(pattern) && !word.contains("http") {
                            let cleaned_word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-');
                            if !cleaned_word.is_empty() {
                                files.push(PathBuf::from(cleaned_word));
                            }
                        }
                    }
                }
            }
            
            // Also look for common file names without extensions
            let common_files = vec!["README", "Cargo.toml", "package.json", "Makefile", "Dockerfile"];
            for file_name in common_files {
                if content.contains(file_name) {
                    for word in content.split_whitespace() {
                        if word.contains(file_name) && !word.contains("http") {
                            let cleaned_word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-');
                            if !cleaned_word.is_empty() {
                                files.push(PathBuf::from(cleaned_word));
                            }
                        }
                    }
                }
            }
        }
        
        files.sort();
        files.dedup();
        files
    }
    
    /// Extract executed tools from conversation context
    fn extract_executed_tools(&self, messages: &[&AgentMessage]) -> Vec<String> {
        let mut tools = Vec::new();
        
        for message in messages {
            for tool_call in &message.tool_calls {
                tools.push(tool_call.name.clone());
            }
        }
        
        tools.sort();
        tools.dedup();
        tools
    }
    
    /// Extract success indicators from content
    fn extract_success_indicators(&self, content: &str) -> Vec<String> {
        self.importance_analyzer.success_indicators.iter()
            .filter(|indicator| content.contains(*indicator))
            .cloned()
            .collect()
    }
    
    /// Create a smart checkpoint with context snapshot
    pub async fn create_smart_checkpoint(
        &self,
        conversation: &mut Conversation,
        suggestion: &CheckpointSuggestion,
        working_directory: Option<&PathBuf>,
    ) -> Result<Uuid> {
        // Generate context snapshot
        let context_snapshot = self.context_generator.generate_snapshot(
            working_directory,
            &suggestion.context.modified_files,
            &self.config,
        ).await?;
        
        let checkpoint = ConversationCheckpoint::new(
            suggestion.message_id,
            suggestion.suggested_title.clone(),
            None,
            Some(context_snapshot),
            true, // Auto-generated
        );
        
        let checkpoint_id = checkpoint.id;
        conversation.checkpoints.push(checkpoint);
        conversation.last_active = Utc::now();
        
        // Cleanup old checkpoints if we exceed the limit
        if conversation.checkpoints.len() > self.config.max_checkpoints_per_conversation {
            self.cleanup_old_checkpoints(conversation);
        }
        
        Ok(checkpoint_id)
    }
    
    /// Cleanup old checkpoints to maintain the limit
    fn cleanup_old_checkpoints(&self, conversation: &mut Conversation) {
        // Sort by creation time (oldest first)
        conversation.checkpoints.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        
        // Keep only the most recent checkpoints
        let keep_count = self.config.max_checkpoints_per_conversation;
        if conversation.checkpoints.len() > keep_count {
            conversation.checkpoints.drain(0..conversation.checkpoints.len() - keep_count);
        }
    }
    
    /// Get configuration
    pub fn get_config(&self) -> &CheckpointConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn update_config(&mut self, config: CheckpointConfig) {
        self.config = config;
    }

    async fn suggest_checkpoint_title(&self, _agent_state: &str, _message_id: Uuid) -> Result<String> {
        let title_suggestion = "Suggested Checkpoint Title"; // Placeholder
        Ok(title_suggestion.to_string())
    }

    async fn create_automatic_checkpoint(
        &self,
        _conversation_id: Uuid, // Mark as unused if not needed now
        message_id: Uuid,
        context_snapshot: ContextSnapshot,
        _checkpoint_config: &CheckpointConfig, // Mark as unused if not needed now
    ) -> Result<ConversationCheckpoint> {
        // Corrected method name call
        let suggested_title = self.suggest_checkpoint_title(&context_snapshot.agent_state, message_id).await?; // Assuming agent_state and message_id are relevant for title
        let checkpoint = ConversationCheckpoint::new(
            message_id,
            suggested_title,
            None, // Added None for description
            Some(context_snapshot),
            true, // Auto-generated
        );
        Ok(checkpoint)
    }

    async fn create_recovery_checkpoint(
        &self,
        message_id: Uuid,
        context_snapshot: ContextSnapshot,
    ) -> Result<ConversationCheckpoint> {
        let title = format!("Recovery Point: {}", Utc::now().to_rfc3339());
        let checkpoint = ConversationCheckpoint::new(
            message_id,
            title,
            Some("Automatic recovery checkpoint".to_string()), // Added description
            Some(context_snapshot),
            true, // Auto-generated
        );
        Ok(checkpoint)
    }
}

impl ContextSnapshotGenerator {
    /// Create a new context snapshot generator
    pub fn new(config: SnapshotConfig) -> Self {
        Self { config }
    }
    
    /// Generate a context snapshot
    pub async fn generate_snapshot(
        &self,
        working_directory: Option<&PathBuf>,
        modified_files: &[PathBuf],
        checkpoint_config: &CheckpointConfig,
    ) -> Result<ContextSnapshot> {
        let mut file_states = HashMap::new();
        let mut repository_states = HashMap::new();
        let mut environment = HashMap::new();
        
        // Capture file states if enabled
        if checkpoint_config.include_file_states {
            file_states = self.capture_file_states(working_directory, modified_files, checkpoint_config).await?;
        }
        
        // Capture repository states if enabled
        if checkpoint_config.include_repository_states {
            repository_states = self.capture_repository_states(working_directory).await?;
        }
        
        // Capture environment if enabled
        if checkpoint_config.include_environment {
            environment = self.capture_environment().await?;
        }
        
        let working_directory = working_directory.cloned().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
        
        Ok(ContextSnapshot {
            file_states,
            repository_states,
            environment,
            working_directory,
            agent_state: "checkpoint".to_string(), // Simplified for now
        })
    }
    
    /// Capture file states
    async fn capture_file_states(
        &self,
        working_directory: Option<&PathBuf>,
        modified_files: &[PathBuf],
        config: &CheckpointConfig,
    ) -> Result<HashMap<PathBuf, String>> {
        let mut file_states = HashMap::new();
        
        let base_dir = working_directory.cloned().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
        
        for file_path in modified_files {
            let full_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                base_dir.join(file_path)
            };
            
            // Check if file should be excluded
            if self.should_exclude_file(&full_path, &config.excluded_file_patterns) {
                continue;
            }
            
            // Check file size
            if let Ok(metadata) = tokio::fs::metadata(&full_path).await {
                if metadata.len() > config.max_file_size_bytes as u64 {
                    continue;
                }
            }
            
            // Read file content
            if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                file_states.insert(file_path.clone(), content);
            }
            
            // Limit number of files
            if file_states.len() >= self.config.max_files {
                break;
            }
        }
        
        Ok(file_states)
    }
    
    /// Check if a file should be excluded from snapshots
    fn should_exclude_file(&self, file_path: &PathBuf, patterns: &[String]) -> bool {
        let path_str = file_path.to_string_lossy();
        
        for pattern in patterns {
            if pattern.contains('*') {
                // Simple glob matching
                let pattern_parts: Vec<&str> = pattern.split('*').collect();
                if pattern_parts.len() == 2 {
                    let prefix = pattern_parts[0];
                    let suffix = pattern_parts[1];
                    if path_str.starts_with(prefix) && path_str.ends_with(suffix) {
                        return true;
                    }
                }
            } else if path_str.contains(pattern) {
                return true;
            }
        }
        
        false
    }
    
    /// Capture repository states
    async fn capture_repository_states(&self, working_directory: Option<&PathBuf>) -> Result<HashMap<String, String>> {
        let mut repository_states = HashMap::new();
        
        let base_dir = working_directory.cloned().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
        
        // Check if this is a git repository
        let git_dir = base_dir.join(".git");
        if git_dir.exists() {
            // Get current commit hash
            if let Ok(output) = tokio::process::Command::new("git")
                .args(&["rev-parse", "HEAD"])
                .current_dir(&base_dir)
                .output()
                .await
            {
                if output.status.success() {
                    let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    repository_states.insert("git_commit".to_string(), commit_hash);
                }
            }
            
            // Get current branch
            if let Ok(output) = tokio::process::Command::new("git")
                .args(&["branch", "--show-current"])
                .current_dir(&base_dir)
                .output()
                .await
            {
                if output.status.success() {
                    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    repository_states.insert("git_branch".to_string(), branch);
                }
            }
        }
        
        Ok(repository_states)
    }
    
    /// Capture environment variables
    async fn capture_environment(&self) -> Result<HashMap<String, String>> {
        let mut environment = HashMap::new();
        
        // Capture only safe environment variables
        let safe_vars = vec![
            "PATH", "HOME", "USER", "SHELL", "TERM", "LANG", "PWD",
            "CARGO_HOME", "RUSTUP_HOME", "NODE_ENV", "PYTHON_PATH",
        ];
        
        for var in safe_vars {
            if let Ok(value) = std::env::var(var) {
                environment.insert(var.to_string(), value);
            }
        }
        
        Ok(environment)
    }
}

// Helper trait for string formatting
trait StringExt {
    fn to_title_case(&self) -> String;
}

impl StringExt for str {
    fn to_title_case(&self) -> String {
        self.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::message::types::AgentMessage;

    #[test]
    fn test_checkpoint_manager_creation() {
        let manager = ConversationCheckpointManager::with_default_config();
        assert!(manager.config.auto_checkpoint_enabled);
        assert_eq!(manager.config.max_checkpoints_per_conversation, 10);
    }

    #[test]
    fn test_checkpoint_suggestion_importance() {
        let manager = ConversationCheckpointManager::with_default_config();
        let mut conversation = Conversation::new("Test".to_string(), None);
        
        // Add messages that should trigger checkpoint suggestions
        conversation.add_message(AgentMessage::user("Let's implement this solution"));
        conversation.add_message(AgentMessage::assistant("Great! The implementation is working perfectly"));
        conversation.add_message(AgentMessage::user("Excellent! Tests are passing"));
        conversation.add_message(AgentMessage::assistant("Task completed successfully"));
        conversation.add_message(AgentMessage::user("Perfect, this is a major milestone"));
        
        let suggestions = manager.analyze_checkpoint_opportunities(&conversation).unwrap();
        
        // Should have suggestions due to success keywords
        assert!(!suggestions.is_empty());
        
        // Check that importance is reasonable
        for suggestion in &suggestions {
            assert!(suggestion.importance >= 0.0 && suggestion.importance <= 1.0);
        }
    }

    #[test]
    fn test_conversation_phase_analysis() {
        let manager = ConversationCheckpointManager::with_default_config();
        
        let implementation_msg = AgentMessage::user("Let's implement this feature");
        let implementation_messages = vec![&implementation_msg];
        let phase = manager.analyze_conversation_phase(&implementation_messages);
        assert_eq!(phase, ConversationPhase::Implementation);
        
        let testing_msg = AgentMessage::user("Now let's test this solution");
        let testing_messages = vec![&testing_msg];
        let phase = manager.analyze_conversation_phase(&testing_messages);
        assert_eq!(phase, ConversationPhase::Testing);
        
        let completion_msg = AgentMessage::user("We're done with this task");
        let completion_messages = vec![&completion_msg];
        let phase = manager.analyze_conversation_phase(&completion_messages);
        assert_eq!(phase, ConversationPhase::Completion);
    }

    #[test]
    fn test_checkpoint_title_generation() {
        let manager = ConversationCheckpointManager::with_default_config();
        
        let title = manager.generate_checkpoint_title(
            &CheckpointReason::SuccessfulSolution, 
            &[], 
            ConversationPhase::Implementation
        );
        assert_eq!(title, "Successful Solution");
        
        let title = manager.generate_checkpoint_title(
            &CheckpointReason::MajorMilestone, 
            &["breakthrough".to_string()], 
            ConversationPhase::Testing
        );
        assert!(title.contains("Milestone"));
        assert!(title.contains("Breakthrough"));
    }

    #[test]
    fn test_restoration_value_calculation() {
        let manager = ConversationCheckpointManager::with_default_config();
        
        let message = AgentMessage::assistant("Task completed successfully");
        let context = vec![];
        
        let value = manager.calculate_restoration_value(&message, &context, &CheckpointReason::SuccessfulSolution);
        
        assert!(value >= 0.0 && value <= 1.0);
        assert!(value > 0.5); // Successful solutions should have higher restoration value
    }

    #[test]
    fn test_file_exclusion() {
        let generator = ContextSnapshotGenerator::new(SnapshotConfig::default());
        let patterns = vec!["*.log".to_string(), "target/*".to_string(), ".git/*".to_string()];
        
        assert!(generator.should_exclude_file(&PathBuf::from("debug.log"), &patterns));
        assert!(generator.should_exclude_file(&PathBuf::from("target/debug/app"), &patterns));
        assert!(generator.should_exclude_file(&PathBuf::from(".git/config"), &patterns));
        assert!(!generator.should_exclude_file(&PathBuf::from("src/main.rs"), &patterns));
    }

    #[test]
    fn test_modified_files_detection() {
        let manager = ConversationCheckpointManager::with_default_config();
        
        let user_msg = AgentMessage::user("I modified src/main.rs and tests/test.py");
        let assistant_msg = AgentMessage::assistant("Let's also update the README.md file");
        let messages = vec![&user_msg, &assistant_msg];
        
        let files = manager.detect_modified_files(&messages);
        
        assert!(files.iter().any(|f| f.to_string_lossy().contains("main.rs")));
        assert!(files.iter().any(|f| f.to_string_lossy().contains("test.py")));
        assert!(files.iter().any(|f| f.to_string_lossy().contains("README.md")));
    }

    #[tokio::test]
    async fn test_smart_checkpoint_creation() {
        let manager = ConversationCheckpointManager::with_default_config();
        let mut conversation = Conversation::new("Test".to_string(), None);
        
        let suggestion = CheckpointSuggestion {
            message_id: Uuid::new_v4(),
            importance: 0.8,
            reason: CheckpointReason::SuccessfulSolution,
            suggested_title: "Successful Implementation".to_string(),
            context: CheckpointContext {
                relevant_messages: vec![],
                trigger_keywords: vec!["success".to_string()],
                conversation_phase: ConversationPhase::Implementation,
                modified_files: vec![PathBuf::from("src/main.rs")],
                executed_tools: vec!["cargo".to_string()],
                success_indicators: vec!["working".to_string()],
            },
            restoration_value: 0.9,
        };
        
        let checkpoint_id = manager.create_smart_checkpoint(&mut conversation, &suggestion, None).await.unwrap();
        
        assert_eq!(conversation.checkpoints.len(), 1);
        assert_eq!(conversation.checkpoints[0].id, checkpoint_id);
        assert_eq!(conversation.checkpoints[0].title, "Successful Implementation");
        assert!(conversation.checkpoints[0].auto_generated);
    }
} 