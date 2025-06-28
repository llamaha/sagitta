use std::sync::Arc;
use uuid::Uuid;
use std::collections::{HashMap, HashSet};
use terminal_stream::events::StreamEvent;
use sha1::{Sha1, Digest};

pub mod config;
pub mod llm_adapter;
pub mod intent_analyzer;

#[cfg(test)]
pub mod test;
#[cfg(test)]
mod tests;

use async_trait::async_trait;
use reasoning_engine::{
    traits::{EventEmitter, StreamHandler, ToolExecutor, StatePersistence, MetricsCollector},
    ReasoningEvent, ToolResult,
    ReasoningError,
    Result as ReasoningEngineResult,
    state::ReasoningState,
    streaming::StreamChunk,
};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use crate::{agent::events::{AgentEvent, ToolRunId}, tools::{registry::ToolRegistry, types::Tool}, llm::client::{MessagePart, StreamChunk as SagittaCodeStreamChunk}, utils::errors::SagittaCodeError};

/// Error recovery strategy for failed tools
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry the same tool with the same parameters
    Retry,
    /// Skip this tool and continue with workflow
    Skip,
    /// Suggest alternative tool or approach
    Alternative,
    /// Stop the workflow due to critical failure
    Stop,
}

/// Enhanced loop detection and recovery information
#[derive(Debug, Clone)]
pub struct LoopDetectionInfo {
    pub tool_name: String,
    pub identical_calls: usize,
    pub last_call_time: std::time::Instant,
    pub suggested_recovery: RecoveryStrategy,
}

#[derive(Clone)]
pub struct AgentToolExecutor {
    tool_registry: Arc<ToolRegistry>,
    terminal_event_sender: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
    /// Event sender for feedback to LLM
    event_sender: Option<broadcast::Sender<AgentEvent>>,
    /// Enhanced loop detection - track tool call history with more context
    recent_tool_calls: Arc<tokio::sync::Mutex<Vec<(String, Value, std::time::Instant)>>>,
    /// Failed tool tracking for graceful degradation
    failed_tools: Arc<tokio::sync::Mutex<HashMap<String, usize>>>,
    /// Tools that have been explicitly skipped due to repeated failures or loops
    skipped_tools: Arc<tokio::sync::Mutex<HashSet<String>>>,
    /// Maximum identical tool calls before triggering loop detection
    max_identical_calls: usize,
    /// Maximum failures per tool before suggesting to skip
    max_tool_failures: usize,
    /// Default timeout for tool execution in seconds
    pub default_timeout_seconds: u64,
}

impl AgentToolExecutor {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { 
            tool_registry,
            terminal_event_sender: None,
            event_sender: None,
            recent_tool_calls: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            failed_tools: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            skipped_tools: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            max_identical_calls: 2, // Reduced from 3 to be more responsive
            max_tool_failures: 3,
            default_timeout_seconds: 1800, // 30 minutes for long operations like sync
        }
    }
    
    /// Set the terminal event sender for streaming shell execution
    pub fn set_terminal_event_sender(&mut self, sender: tokio::sync::mpsc::Sender<StreamEvent>) {
        self.terminal_event_sender = Some(sender);
    }
    
    /// Set the event sender for LLM feedback
    pub fn set_event_sender(&mut self, sender: broadcast::Sender<AgentEvent>) {
        self.event_sender = Some(sender);
    }
    
    /// Get timeout duration for specific tools
    pub fn get_timeout_for_tool(&self, tool_name: &str) -> u64 {
        match tool_name {
            "sync_repository" | "add_existing_repository" => {
                // No timeout for sync operations - they use progress reporting instead
                u64::MAX // Effectively no timeout
            }
            _ => self.default_timeout_seconds
        }
    }
    
    /// Enhanced loop detection with recovery strategy determination
    async fn check_loop_detection(&self, tool_name: &str, args: &Value) -> Option<LoopDetectionInfo> {
        let mut recent_calls = self.recent_tool_calls.lock().await;
        let now = std::time::Instant::now();
        
        // Clean up old calls (older than 2 minutes instead of 60 seconds for better context)
        recent_calls.retain(|(_, _, timestamp)| now.duration_since(*timestamp).as_secs() < 120);
        
        // Smart loop detection: Check for significant parameter similarity, not exact equality
        let similar_calls = recent_calls
            .iter()
            .filter(|(name, call_args, timestamp)| {
                name == tool_name && 
                now.duration_since(*timestamp).as_secs() < 60 && // Keep 60 second window for similarity check
                self.are_parameters_significantly_similar(tool_name, call_args, args)
            })
            .count();
        
        // Add current call to history AFTER counting
        let current_call = (tool_name.to_string(), args.clone(), now);
        recent_calls.push(current_call);
        
        // Keep only recent calls (increased to 25 for better context tracking)
        if recent_calls.len() > 25 {
            let len = recent_calls.len();
            recent_calls.drain(0..len - 25);
        }
        
        // Adjusted threshold based on tool type and significance
        let loop_threshold = self.get_loop_threshold_for_tool(tool_name);
        
        if similar_calls >= loop_threshold {
            // Determine recovery strategy based on tool type and failure context
            let recovery_strategy = self.determine_recovery_strategy(tool_name, args).await;
            
            Some(LoopDetectionInfo {
                tool_name: tool_name.to_string(),
                identical_calls: similar_calls + 1, // +1 to include current call
                last_call_time: now,
                suggested_recovery: recovery_strategy,
            })
        } else {
            None
        }
    }
    
    /// Determine if two parameter sets are significantly similar for loop detection
    /// This is more nuanced than exact equality
    fn are_parameters_significantly_similar(&self, tool_name: &str, params1: &Value, params2: &Value) -> bool {
        match tool_name {
            "add_existing_repository" => {
                // For repository tools, compare essential parameters (name, url/local_path)
                // but allow differences in non-essential ones (branch, optional flags)
                let name1 = params1.get("name").and_then(|v| v.as_str());
                let name2 = params2.get("name").and_then(|v| v.as_str());
                
                let path1 = params1.get("local_path").and_then(|v| v.as_str())
                    .or_else(|| params1.get("url").and_then(|v| v.as_str()));
                let path2 = params2.get("local_path").and_then(|v| v.as_str())
                    .or_else(|| params2.get("url").and_then(|v| v.as_str()));
                
                // Also check branches - different branches should NOT be considered similar
                let branch1 = params1.get("branch").and_then(|v| v.as_str());
                let branch2 = params2.get("branch").and_then(|v| v.as_str());
                
                // Consider similar only if same name, same primary path/url, AND same branch (or both missing)
                name1 == name2 && path1 == path2 && branch1 == branch2 && name1.is_some() && path1.is_some()
            }
            "shell_execution" => {
                // For shell commands, compare the actual command but allow different working directories
                // if the core command is different
                let cmd1 = params1.get("command").and_then(|v| v.as_str()).unwrap_or("");
                let cmd2 = params2.get("command").and_then(|v| v.as_str()).unwrap_or("");
                
                // Extract the base command (first word) to allow for parameter variations
                let base_cmd1 = cmd1.split_whitespace().next().unwrap_or("");
                let base_cmd2 = cmd2.split_whitespace().next().unwrap_or("");
                
                // Only consider truly similar if the base command and major structure match
                if base_cmd1 != base_cmd2 {
                    false
                } else {
                    // For same base command, check if the full commands are very similar
                    self.calculate_command_similarity(cmd1, cmd2) > 0.8
                }
            }
            "sync_repository" | "remove_repository" => {
                // For single-parameter repository operations, compare the repository name
                let name1 = params1.get("name").and_then(|v| v.as_str());
                let name2 = params2.get("name").and_then(|v| v.as_str());
                name1 == name2 && name1.is_some()
            }
            _ => {
                // For other tools, use a more sophisticated similarity check
                // rather than exact equality
                self.calculate_parameter_similarity(params1, params2) > 0.9
            }
        }
    }
    
    /// Calculate similarity between two commands (0.0 = completely different, 1.0 = identical)
    fn calculate_command_similarity(&self, cmd1: &str, cmd2: &str) -> f32 {
        if cmd1 == cmd2 {
            return 1.0;
        }
        
        let words1: Vec<&str> = cmd1.split_whitespace().collect();
        let words2: Vec<&str> = cmd2.split_whitespace().collect();
        
        if words1.is_empty() && words2.is_empty() {
            return 1.0;
        }
        
        if words1.is_empty() || words2.is_empty() {
            return 0.0;
        }
        
        // Simple word-based similarity
        let common_words = words1.iter()
            .filter(|word| words2.contains(word))
            .count();
        
        let total_unique_words = words1.len().max(words2.len());
        common_words as f32 / total_unique_words as f32
    }
    
    /// Calculate general parameter similarity for JSON values
    fn calculate_parameter_similarity(&self, params1: &Value, params2: &Value) -> f32 {
        if params1 == params2 {
            return 1.0;
        }
        
        match (params1, params2) {
            (Value::Object(obj1), Value::Object(obj2)) => {
                let all_keys: std::collections::HashSet<&String> = obj1.keys().chain(obj2.keys()).collect();
                if all_keys.is_empty() {
                    return 1.0;
                }
                
                let matching_keys = all_keys.iter()
                    .filter(|&key| {
                        let val1 = obj1.get(key.as_str());
                        let val2 = obj2.get(key.as_str());
                        val1 == val2
                    })
                    .count();
                
                matching_keys as f32 / all_keys.len() as f32
            }
            _ => {
                // For non-objects, fall back to exact comparison
                if params1 == params2 { 1.0 } else { 0.0 }
            }
        }
    }
    
    /// Get the loop detection threshold for a specific tool
    fn get_loop_threshold_for_tool(&self, tool_name: &str) -> usize {
        match tool_name {
            // Repository tools are often problematic and should have lower thresholds
            "add_existing_repository" => 1, // Trigger after 1 similar call (2 total calls)
            "sync_repository" => 1,
            // Shell commands might legitimately be retried more often
            "shell_execution" => 3, // Trigger after 3 similar calls (4 total calls)
            // File operations might need retries for timing issues
            "view_file" | "edit_file" => 4, // Trigger after 4 similar calls (5 total calls)
            // Other tools use the default
            _ => self.max_identical_calls - 1,
        }
    }
    
    /// Determine the best recovery strategy for a looping tool
    async fn determine_recovery_strategy(&self, tool_name: &str, _args: &Value) -> RecoveryStrategy {
        let failed_tools = self.failed_tools.lock().await;
        let failure_count = failed_tools.get(tool_name).unwrap_or(&0);
        
        match tool_name {
            // Repository tools can often be skipped if they fail
            "add_existing_repository" | "sync_repository" => {
                // More aggressive skipping for repository tools - skip after 1st loop detection
                RecoveryStrategy::Skip
            }
            // Search tools can be replaced with alternatives
            "search_file_in_repository" | "query" => RecoveryStrategy::Alternative,
            // Shell execution failures might need different commands
            "shell_execution" => RecoveryStrategy::Alternative,
            // Project creation is often critical - suggest stopping if repeatedly fails
            "create_project" => {
                if *failure_count >= 1 {
                    RecoveryStrategy::Stop
                } else {
                    RecoveryStrategy::Retry
                }
            }
            // Default: skip after first loop detection to prevent infinite loops
            _ => RecoveryStrategy::Skip,
        }
    }
    
    /// Enhanced error feedback with recovery suggestions
    async fn surface_error_to_llm_with_recovery(&self, tool_name: &str, error_message: &str, error_code: &str, recovery_info: Option<LoopDetectionInfo>) {
        if let Some(ref event_sender) = self.event_sender {
            let llm_feedback = match error_code {
                "invalid_parameters" => {
                    self.generate_parameter_error_feedback(tool_name, error_message).await
                }
                "tool_not_found" => {
                    format!("⚠️ Tool '{}' not found. Available alternatives: {}. Please try a different tool.", 
                           tool_name, self.suggest_alternative_tools(tool_name).await)
                }
                "execution_error" => {
                    format!("⚠️ Tool '{}' execution failed: {}. {}.", 
                           tool_name, error_message, self.suggest_execution_recovery(tool_name).await)
                }
                "execution_timeout" => {
                    format!("⏱️ Tool '{}' timed out: {}. Try breaking the operation into smaller steps or check if the command is appropriate for this environment.", 
                           tool_name, error_message)
                }
                "tool_skipped" => {
                    format!("⏭️ Tool '{}' skipped: {}. I'll continue with alternative approaches.", 
                           tool_name, error_message)
                }
                "loop_detected" => {
                    self.generate_loop_recovery_feedback(tool_name, recovery_info).await
                }
                _ => {
                    format!("❌ Tool '{}' error: {}. {}", 
                           tool_name, error_message, self.suggest_general_recovery(tool_name).await)
                }
            };
            
            // Send as both a streaming chunk and an event
            let _ = event_sender.send(AgentEvent::LlmChunk {
                content: format!("\n{}\n\n", llm_feedback),
                is_final: false,
                is_thinking: false,
            });
            
            let _ = event_sender.send(AgentEvent::Error(error_message.to_string()));
        }
    }
    
    /// Generate helpful parameter error feedback with specific suggestions
    async fn generate_parameter_error_feedback(&self, tool_name: &str, error_message: &str) -> String {
        match tool_name {
            "add_existing_repository" => {
                if error_message.contains("oneOf") || error_message.contains("parameter combinations") || error_message.contains("Either URL or existing local repository path") {
                    format!("⚠️ Tool 'add_existing_repository' parameter error: {}. \n\n**Quick Fix**: You must provide EITHER:\n• `url` (for remote repositories): {{\"name\": \"repo-name\", \"url\": \"https://github.com/user/repo.git\"}}\n• `local_path` (for existing local directories): {{\"name\": \"repo-name\", \"local_path\": \"/absolute/path/to/directory\"}}\n\n💡 **Note**: This tool is ONLY for adding existing repositories. To create new projects, use shell commands like:\n• Rust: `cargo new project-name`\n• Node.js: `npm init project-name`\n• Python: `python -m venv project-name`", error_message)
                } else {
                    format!("⚠️ Tool 'add_existing_repository' failed: {}. \n\n**Quick Fix**: Try specifying either 'url' OR 'local_path' parameter.\n• For remote repos: provide `url`\n• For local directories: provide `local_path`\n\n💡 **Note**: This tool is ONLY for existing repositories. To create new projects, use shell commands instead.", error_message)
                }
            }
            "shell_execution" => {
                format!("⚠️ Shell command parameter error: {}. \n\n**Quick Fix**: Please provide a valid 'command' parameter with the shell command to execute.", error_message)
            }
            _ => {
                format!("⚠️ Tool '{}' parameter validation failed: {}. \n\n**Quick Fix**: Please check the required parameters and try again with the correct format.", tool_name, error_message)
            }
        }
    }
    
    /// Generate loop detection feedback with recovery suggestions
    async fn generate_loop_recovery_feedback(&self, tool_name: &str, recovery_info: Option<LoopDetectionInfo>) -> String {
        if let Some(info) = recovery_info {
            match info.suggested_recovery {
                RecoveryStrategy::Skip => {
                    format!("🔄 **Loop Detected**: Tool '{}' has been called {} times with identical parameters.\n\n**Recommended Action**: This step appears to be problematic. Let me skip this tool and continue with the rest of the workflow. I'll proceed without '{}' to avoid blocking progress.", 
                           tool_name, info.identical_calls, tool_name)
                }
                RecoveryStrategy::Alternative => {
                    let alternatives = self.suggest_alternative_tools(tool_name).await;
                    format!("🔄 **Loop Detected**: Tool '{}' has been called {} times with identical parameters.\n\n**Recommended Action**: Let me try a different approach using alternative tools: {}. If you have specific requirements for this step, please clarify.", 
                           tool_name, info.identical_calls, alternatives)
                }
                RecoveryStrategy::Stop => {
                    format!("🔄 **Critical Loop Detected**: Tool '{}' has failed {} times with identical parameters.\n\n**Action Required**: This tool is essential for the workflow but keeps failing. Please check the parameters or environment. The workflow cannot continue without resolving this issue.", 
                           tool_name, info.identical_calls)
                }
                RecoveryStrategy::Retry => {
                    format!("🔄 **Loop Detected**: Tool '{}' has been called {} times. Let me try once more with modified parameters or wait briefly before retrying.", 
                           tool_name, info.identical_calls)
                }
            }
        } else {
            format!("🔄 Loop detected: Tool '{}' has been called repeatedly. Please try a different approach.", tool_name)
        }
    }
    
    /// Suggest alternative tools for common scenarios
    async fn suggest_alternative_tools(&self, failed_tool: &str) -> String {
        match failed_tool {
            "add_existing_repository" => "use shell commands (cargo new, npm init, etc.) for new projects, or verify the repository path/URL for existing ones".to_string(),
            "search_file_in_repository" => "try 'query' for general searches or check repository is synced".to_string(),
            "sync_repository" => "try 'add_existing_repository' again or check network connection".to_string(),
            "shell_execution" => "try breaking the command into smaller steps or check permissions".to_string(),
            _ => "check tool parameters or try a different approach".to_string(),
        }
    }
    
    /// Suggest execution recovery strategies
    async fn suggest_execution_recovery(&self, tool_name: &str) -> String {
        match tool_name {
            "add_existing_repository" => "Consider using an absolute path for local repositories or verify the Git URL is accessible",
            "shell_execution" => "Try breaking complex commands into simpler steps or check if the command is available",
            "sync_repository" => "Check network connectivity and repository permissions",
            _ => "Please verify the tool parameters and try again with corrected values",
        }.to_string()
    }
    
    /// Suggest general recovery approaches
    async fn suggest_general_recovery(&self, tool_name: &str) -> String {
        match tool_name {
            "add_existing_repository" => "For troubleshooting: ensure Git is installed, check repository URL/path exists, and verify permissions",
            _ => "Please check the error details and adjust your approach accordingly",
        }.to_string()
    }
    
    /// Track tool failure for graceful degradation
    async fn track_tool_failure(&self, tool_name: &str) {
        let mut failed_tools = self.failed_tools.lock().await;
        let count = failed_tools.entry(tool_name.to_string()).or_insert(0);
        *count += 1;
    }
    
    /// Check if a tool should be skipped due to repeated failures or explicit skip decisions
    async fn should_skip_tool(&self, tool_name: &str) -> bool {
        let failed_tools = self.failed_tools.lock().await;
        let skipped_tools = self.skipped_tools.lock().await;
        
        // Check if tool is explicitly skipped
        if skipped_tools.contains(tool_name) {
            return true;
        }
        
        // Check failure count
        if let Some(failure_count) = failed_tools.get(tool_name) {
            // Skip after 3 failures (more aggressive than max_tool_failures for loop detection)
            *failure_count >= 3
        } else {
            false
        }
    }
    
    /// Mark a tool as skipped for the remainder of this session
    async fn mark_tool_as_skipped(&self, tool_name: &str) {
        let mut skipped_tools = self.skipped_tools.lock().await;
        skipped_tools.insert(tool_name.to_string());
    }
    
    /// Reset tool failure tracking when a tool succeeds
    async fn reset_tool_failure_tracking(&self, tool_name: &str) {
        let mut failed_tools = self.failed_tools.lock().await;
        failed_tools.remove(tool_name);
        
        // Also remove from skipped tools if it succeeds
        let mut skipped_tools = self.skipped_tools.lock().await;
        skipped_tools.remove(tool_name);
    }
    
    /// Enhanced parameter validation with better error messages
    async fn validate_tool_parameters(&self, tool_name: &str, args: &Value) -> Result<(), String> {
        // Get the tool definition to access its parameter schema
        let tool_definitions = self.tool_registry.get_definitions().await;
        let tool_def = tool_definitions
            .iter()
            .find(|def| def.name == tool_name)
            .ok_or_else(|| format!("Tool '{}' not found in registry", tool_name))?;

        // Check required fields
        if let Some(required_fields) = tool_def.parameters.get("required").and_then(|r| r.as_array()) {
            for required_field in required_fields {
                if let Some(field_name) = required_field.as_str() {
                    if !args.get(field_name).is_some() {
                        return Err(format!("Missing required parameter: '{}'", field_name));
                    }
                }
            }
        }

        // Enhanced oneOf constraint validation
        if let Some(one_of) = tool_def.parameters.get("oneOf").and_then(|o| o.as_array()) {
            let mut satisfied_constraint = false;
            let mut constraint_details = Vec::new();
            
            for constraint in one_of {
                if let Some(required_in_constraint) = constraint.get("required").and_then(|r| r.as_array()) {
                    let required_fields: Vec<String> = required_in_constraint
                        .iter()
                        .filter_map(|f| f.as_str().map(|s| s.to_string()))
                        .collect();
                    
                    constraint_details.push(required_fields.clone());
                    
                    let all_fields_present = required_fields
                        .iter()
                        .all(|field_name| {
                            args.get(field_name).is_some() && 
                            !args.get(field_name).unwrap().is_null()
                        });
                    
                    if all_fields_present {
                        satisfied_constraint = true;
                        break;
                    }
                }
            }
            
            if !satisfied_constraint {
                // Generate more helpful error message
                let constraint_descriptions: Vec<String> = constraint_details
                    .iter()
                    .map(|fields| {
                        if fields.len() == 1 {
                            format!("'{}'", fields[0])
                        } else {
                            format!("({})", fields.join(" AND "))
                        }
                    })
                    .collect();
                
                return Err(format!(
                    "Either URL or existing local repository path must be provided. Please provide one of: {}",
                    constraint_descriptions.join(" OR ")
                ));
            }
        }

        // Additional validation for specific tools
        match tool_name {
            "add_existing_repository" => {
                // Ensure neither url nor local_path are empty strings
                if let Some(url) = args.get("url") {
                    if url.is_string() && url.as_str().unwrap_or("").trim().is_empty() {
                        return Err("URL parameter cannot be empty".to_string());
                    }
                }
                if let Some(local_path) = args.get("local_path") {
                    if local_path.is_string() && local_path.as_str().unwrap_or("").trim().is_empty() {
                        return Err("local_path parameter cannot be empty".to_string());
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait]
impl ToolExecutor for AgentToolExecutor {
    async fn execute_tool(&self, name: &str, args: Value) -> ReasoningEngineResult<ToolResult> {
        log::info!("REASONING: execute_tool called with name: '{}', args: {:?}", name, args);
        
        // Check if tool should be skipped due to repeated failures
        if self.should_skip_tool(name).await {
            let error_msg = format!("Tool '{}' is being skipped due to repeated failures", name);
            self.surface_error_to_llm_with_recovery(name, &error_msg, "tool_skipped", None).await;
            return Ok(reasoning_engine::ToolResult::failure(error_msg, 0));
        }
        
        // Enhanced loop detection with recovery strategy
        if let Some(loop_info) = self.check_loop_detection(name, &args).await {
            let error_msg = format!("Loop detected: Tool '{}' called {} times with similar parameters", 
                                   name, loop_info.identical_calls);
            
            match loop_info.suggested_recovery {
                RecoveryStrategy::Skip => {
                    self.mark_tool_as_skipped(name).await;
                    self.surface_error_to_llm_with_recovery(name, &error_msg, "loop_detected", Some(loop_info)).await;
                    return Ok(reasoning_engine::ToolResult::failure(
                        format!("Skipping '{}' due to loop detection", name), 0
                    ));
                }
                RecoveryStrategy::Stop => {
                    self.surface_error_to_llm_with_recovery(name, &error_msg, "loop_detected", Some(loop_info)).await;
                    return Err(ReasoningError::tool_execution(name.to_string(), format!("Critical loop detected for tool '{}'", name)));
                }
                _ => {
                    // Continue with execution but provide feedback
                    self.surface_error_to_llm_with_recovery(name, &error_msg, "loop_detected", Some(loop_info)).await;
                }
            }
        }

        // Enhanced parameter validation with better error messages
        if let Err(validation_error) = self.validate_tool_parameters(name, &args).await {
            self.track_tool_failure(name).await;
            self.surface_error_to_llm_with_recovery(name, &validation_error, "invalid_parameters", None).await;
            return Ok(reasoning_engine::ToolResult::failure(validation_error, 0));
        }

        // Get the tool from registry
        let tool = match self.tool_registry.get(name).await {
            Some(tool) => tool,
            None => {
                let error_msg = format!("Tool '{}' not found in registry", name);
                self.surface_error_to_llm_with_recovery(name, &error_msg, "tool_not_found", None).await;
                return Ok(reasoning_engine::ToolResult::failure(error_msg, 0));
            }
        };

        log::debug!("Found tool '{}', executing with args: {:?}", name, args);

        // Generate unique run ID for this tool execution
        let run_id = Uuid::new_v4();
        
        // Create progress channel for this tool run
        let (progress_tx, mut progress_rx) = mpsc::channel::<StreamEvent>(200);
        
        // Emit tool run started event
        if let Some(ref event_sender) = self.event_sender {
            let _ = event_sender.send(AgentEvent::ToolRunStarted {
                run_id,
                tool: name.to_string(),
            });
        }
        
        // Spawn task to forward progress events
        if let Some(event_sender) = self.event_sender.clone() {
            tokio::spawn(async move {
                while let Some(event) = progress_rx.recv().await {
                    let _ = event_sender.send(AgentEvent::ToolStream { run_id, event });
                }
            });
        }

        // Clone necessary data for the spawned task
        let tool_clone = tool.clone();
        let args_clone = args.clone();
        let name_clone = name.to_string();
        let terminal_sender = self.terminal_event_sender.clone();
        
        // Spawn the tool execution on a separate task to prevent deadlocks
        let execution_task = tokio::spawn(async move {
            // Special handling for progress-aware repository tools
            if name_clone == "add_existing_repository" {
                if let Some(add_tool) = tool_clone.as_any().downcast_ref::<crate::tools::repository::add::AddExistingRepositoryTool>() {
                    // Create a new instance with progress sender
                    let repo_manager = add_tool.repo_manager.clone();
                    let progress_tool = crate::tools::repository::add::AddExistingRepositoryTool::new_with_progress_sender(
                        repo_manager,
                        Some(progress_tx.clone())
                    );
                    return progress_tool.execute(args_clone).await;
                }
            } else if name_clone == "sync_repository" {
                if let Some(sync_tool) = tool_clone.as_any().downcast_ref::<crate::tools::repository::sync::SyncRepositoryTool>() {
                    // For sync tool, we need to pass the progress sender to the sync operation
                    // This requires modifying the sync tool to accept a progress sender
                    // For now, fall through to standard execution
                }
            }
            
            // Special handling for streaming shell execution
            if name_clone == "streaming_shell_execution" || name_clone == "shell_execution" {
                if let Some(sender) = terminal_sender {
                    // Try to execute with streaming if the tool supports it
                    if let Some(shell_tool) = tool_clone.as_any().downcast_ref::<crate::tools::shell_execution::StreamingShellExecutionTool>() {
                        // Parse shell parameters
                        if let Ok(params) = serde_json::from_value::<crate::tools::shell_execution::ShellExecutionParams>(args_clone.clone()) {
                            log::debug!("Executing streaming shell tool with params: {:?}", params);
                            match shell_tool.execute_streaming(params, sender).await {
                                Ok(result) => {
                                    let result_value = serde_json::to_value(result).unwrap_or_default();
                                    return Ok(crate::tools::types::ToolResult::Success(result_value));
                                }
                                Err(e) => {
                                    return Ok(crate::tools::types::ToolResult::Error { 
                                        error: format!("Streaming shell execution failed: {}", e)
                                    });
                                }
                            }
                        }
                    }
                }
            }
            
            // Standard tool execution
            tool_clone.execute(args_clone).await
        });

        // Execute with tool-specific timeout
        let timeout_seconds = self.get_timeout_for_tool(name);
        let timeout_duration = if timeout_seconds == u64::MAX {
            // For operations without timeout, use a very large duration
            std::time::Duration::from_secs(86400) // 24 hours
        } else {
            std::time::Duration::from_secs(timeout_seconds)
        };
        let result = match tokio::time::timeout(timeout_duration, execution_task).await {
            Ok(task_result) => {
                match task_result {
                    Ok(tool_result) => {
                        // Tool completed successfully - convert sagitta-code ToolResult to reasoning-engine ToolResult
                        self.reset_tool_failure_tracking(name).await;
                        
                        log::debug!("Tool '{}' completed successfully", name);
                        
                        // Emit tool run completed event
                        if let Some(ref event_sender) = self.event_sender {
                            let success = matches!(tool_result, Ok(crate::tools::types::ToolResult::Success(_)));
                            let _ = event_sender.send(AgentEvent::ToolRunCompleted {
                                run_id,
                                tool: name.to_string(),
                                success,
                            });
                        }
                        
                        match tool_result {
                            Ok(crate::tools::types::ToolResult::Success(data)) => {
                                Ok(reasoning_engine::ToolResult::success(data, 0))
                            }
                            Ok(crate::tools::types::ToolResult::Error { error }) => {
                                let error_msg = error;
                                self.track_tool_failure(name).await;
                                self.surface_error_to_llm_with_recovery(name, &error_msg, "execution_error", None).await;
                                Ok(reasoning_engine::ToolResult::failure(error_msg, 0))
                            }
                            Err(sagitta_error) => {
                                let error_msg = sagitta_error.to_string();
                                self.track_tool_failure(name).await;
                                self.surface_error_to_llm_with_recovery(name, &error_msg, "execution_error", None).await;
                                Ok(reasoning_engine::ToolResult::failure(error_msg, 0))
                            }
                        }
                    }
                    Err(sagitta_error) => {
                        // Tool execution failed
                        let error_msg = sagitta_error.to_string();
                        self.track_tool_failure(name).await;
                        self.surface_error_to_llm_with_recovery(name, &error_msg, "execution_error", None).await;
                        
                        // Emit tool run completed event with failure
                        if let Some(ref event_sender) = self.event_sender {
                            let _ = event_sender.send(AgentEvent::ToolRunCompleted {
                                run_id,
                                tool: name.to_string(),
                                success: false,
                            });
                        }
                        
                        Ok(reasoning_engine::ToolResult::failure(error_msg, 0))
                    }
                }
            }
            Err(_timeout) => {
                // Tool execution timed out
                let error_msg = format!("Tool '{}' timed out after {} seconds", name, timeout_seconds);
                self.track_tool_failure(name).await;
                self.surface_error_to_llm_with_recovery(name, &error_msg, "execution_timeout", None).await;
                
                // Emit tool run completed event with failure
                if let Some(ref event_sender) = self.event_sender {
                    let _ = event_sender.send(AgentEvent::ToolRunCompleted {
                        run_id,
                        tool: name.to_string(),
                        success: false,
                    });
                }
                
                Ok(reasoning_engine::ToolResult::failure(error_msg, 0))
            }
        };

        log::debug!("Tool '{}' execution result: {:?}", name, result);
        result
    }

    async fn get_available_tools(&self) -> ReasoningEngineResult<Vec<reasoning_engine::traits::ToolDefinition>> {
        let sagitta_code_tools: Vec<crate::tools::types::ToolDefinition> = self.tool_registry.get_definitions().await;
        let mut reasoning_tools = Vec::new();
        for sagitta_code_tool in sagitta_code_tools {
            reasoning_tools.push(reasoning_engine::traits::ToolDefinition {
                name: sagitta_code_tool.name,
                description: sagitta_code_tool.description,
                parameters: sagitta_code_tool.parameters, 
                is_required: sagitta_code_tool.is_required,
                category: Some(sagitta_code_tool.category.to_string()), 
                estimated_duration_ms: None, 
            });
        }
        Ok(reasoning_tools)
    }
}

pub struct AgentEventEmitter {
    event_sender: broadcast::Sender<AgentEvent>,
    /// Last sent content hash (SHA-1) for deduplication
    last_sent_hash: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl AgentEventEmitter {
    pub fn new(event_sender: broadcast::Sender<AgentEvent>) -> Self {
        Self { 
            event_sender,
            last_sent_hash: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
    
    /// Helper method to compute SHA-1 hash of content for deduplication
    fn compute_content_hash(&self, content: &str) -> String {
        let mut hasher = Sha1::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    /// Helper method to emit a streaming text chunk with deduplication
    async fn emit_streaming_text(&self, content: String, is_final: bool) {
        // Compute hash for deduplication
        let content_hash = self.compute_content_hash(&content);
        
        // Check if this content is a duplicate
        {
            let mut last_hash = self.last_sent_hash.lock().await;
            if let Some(ref previous_hash) = *last_hash {
                if previous_hash == &content_hash {
                    log::debug!("AgentEventEmitter: Dropping duplicate chunk with hash: {}", content_hash);
                    return; // Skip sending duplicate content
                }
            }
            // Update the last sent hash
            *last_hash = Some(content_hash.clone());
        }
        
        if let Err(e) = self.event_sender.send(AgentEvent::LlmChunk {
            content: content.clone(),
            is_final,
            is_thinking: false,
        }) {
            log::warn!("AgentEventEmitter: Failed to send streaming text chunk: {}", e);
        } else {
            log::debug!("AgentEventEmitter: Sent deduplicated chunk with hash: {}", content_hash);
        }
    }
    
    /// Helper method to emit an agent event with deduplication for certain event types
    async fn emit_deduplicated_agent_event(&self, event: AgentEvent) {
        // Only apply deduplication to events that contain text content
        let should_deduplicate = match &event {
            AgentEvent::LlmChunk { content, .. } => {
                let content_hash = self.compute_content_hash(content);
                let mut last_hash = self.last_sent_hash.lock().await;
                if let Some(ref previous_hash) = *last_hash {
                    if previous_hash == &content_hash {
                        log::debug!("AgentEventEmitter: Dropping duplicate LlmChunk with hash: {}", content_hash);
                        return; // Skip sending duplicate content
                    }
                }
                *last_hash = Some(content_hash.clone());
                log::debug!("AgentEventEmitter: Sending deduplicated LlmChunk with hash: {}", content_hash);
                false // Don't need to deduplicate again since we already handled it
            }
            _ => false, // Don't deduplicate other event types
        };
        
        if let Err(e) = self.event_sender.send(event) {
            log::error!("Failed to broadcast AgentEvent from AgentEventEmitter: {}", e);
        }
    }
}

#[async_trait]
impl EventEmitter for AgentEventEmitter {
    async fn emit_event(&self, event: ReasoningEvent) -> ReasoningEngineResult<()> {
        let agent_event = match event {
            ReasoningEvent::SessionStarted { session_id, input, timestamp: _ } => {
                // Don't emit streaming text for session start to avoid clutter
                AgentEvent::ReasoningStarted {
                    session_id,
                    input,
                }
            }
            ReasoningEvent::SessionCompleted {
                session_id,
                success,
                total_duration_ms,
                steps_executed,
                tools_used,
            } => AgentEvent::ReasoningCompleted {
                session_id,
                success,
                duration_ms: total_duration_ms,
                steps: steps_executed,
                tools: tools_used,
            },
            ReasoningEvent::StepCompleted { session_id, step_id: _, step_type, confidence: _, duration_ms } => {
                AgentEvent::ReasoningStep {
                    session_id,
                    step: duration_ms as u32,
                    description: step_type,
                }
            }
            ReasoningEvent::ToolExecutionStarted { session_id: _, tool_name, tool_args: _ } => {
                // Only emit a log event, do not emit streaming text to avoid duplicate UI messages
                AgentEvent::Log(format!("Tool execution started: {}", tool_name))
            }
            ReasoningEvent::ToolExecutionCompleted { session_id: _, tool_name, success, duration_ms } => {
                // Don't emit redundant text - the tool card already shows completion status
                
                AgentEvent::ToolCompleted {
                    tool_name,
                    success,
                    duration_ms,
                }
            }
            ReasoningEvent::Summary { session_id, content, timestamp } => {
                // Don't emit streaming text for summary - it's redundant with tool cards
                AgentEvent::Log(format!("Summary generated: {}", content.chars().take(100).collect::<String>()))
            }
            ReasoningEvent::DecisionMade { session_id, decision_id: _, options_considered: _, chosen_option, confidence } => {
                AgentEvent::DecisionMade {
                    session_id,
                    decision: chosen_option,
                    confidence,
                }
            }
            ReasoningEvent::StreamChunkReceived { session_id: _, chunk_type, chunk_size } => {
                AgentEvent::Log(format!("ReasoningEngine: StreamChunkReceived - Type: {}, Size: {}", chunk_type, chunk_size))
            }
            ReasoningEvent::ErrorOccurred { session_id: _, error_type: _, error_message, recoverable: _ } => {
                // Emit streaming text for errors
                self.emit_streaming_text(
                    format!("❌ Error: {}\n\n", error_message),
                    false
                ).await;
                
                AgentEvent::Error(error_message)
            }
            ReasoningEvent::TokenUsageReceived { session_id, usage } => {
                AgentEvent::TokenUsageReport {
                    conversation_id: Some(session_id),
                    model_name: usage.model_name,
                    prompt_tokens: usage.prompt_tokens as u32,
                    completion_tokens: usage.completion_tokens as u32,
                    cached_tokens: usage.cached_tokens.map(|ct| ct as u32),
                    total_tokens: usage.total_tokens as u32,
                }
            }
            _ => {
                AgentEvent::Log(format!("Unhandled/Generic ReasoningEvent: {:?}", event))
            }
        };

        self.emit_deduplicated_agent_event(agent_event).await;
        Ok(())
    }
}

pub struct AgentStreamHandler {
    output_chunk_sender: mpsc::UnboundedSender<Result<SagittaCodeStreamChunk, SagittaCodeError>>,
}

impl AgentStreamHandler {
    pub fn new(output_chunk_sender: mpsc::UnboundedSender<Result<SagittaCodeStreamChunk, SagittaCodeError>>) -> Self {
        Self { output_chunk_sender }
    }
}

#[async_trait]
impl StreamHandler for AgentStreamHandler {
    async fn handle_chunk(&self, chunk: StreamChunk) -> ReasoningEngineResult<()> {
        let sagitta_code_chunk_conversion_result: Result<Option<SagittaCodeStreamChunk>, SagittaCodeError> = {
            match chunk.chunk_type.as_str() {
                "llm_text" | "text" | "llm_output" => {
                    String::from_utf8(chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid UTF-8 for text: {}", e)))
                        .map(|text_content| Some(SagittaCodeStreamChunk {
                            part: MessagePart::Text { text: text_content },
                            is_final: chunk.is_final,
                            finish_reason: if chunk.is_final { chunk.metadata.get("finish_reason").cloned() } else { None },
                            token_usage: None,
                        }))
                }
                "summary" => {
                    // Handle summary chunks with proper metadata
                    String::from_utf8(chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid UTF-8 for summary: {}", e)))
                        .map(|text_content| {
                            // Create a text chunk with summary metadata
                            let mut sagitta_code_chunk = SagittaCodeStreamChunk {
                                part: MessagePart::Text { text: text_content },
                                is_final: chunk.is_final,
                                finish_reason: if chunk.is_final { chunk.metadata.get("finish_reason").cloned() } else { None },
                                token_usage: None,
                            };
                            // The metadata will be handled by the agent event system
                            Some(sagitta_code_chunk)
                        })
                }
                "tool_call" => {
                    serde_json::from_slice(&chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid JSON for tool_call: {}", e)))
                        .map(|tool_call_data: serde_json::Value| {
                            let call_id = tool_call_data.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let name = tool_call_data.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let parameters = tool_call_data.get("parameters").cloned().unwrap_or(serde_json::Value::Null);
                            Some(SagittaCodeStreamChunk {
                                part: MessagePart::ToolCall { tool_call_id: call_id, name, parameters },
                                is_final: chunk.is_final,
                                finish_reason: None,
                                token_usage: None,
                            })
                        })
                }
                "tool_result" => {
                    serde_json::from_slice(&chunk.data)
                        .map_err(|e| SagittaCodeError::ParseError(format!("Stream chunk data is not valid JSON for tool_result: {}", e)))
                        .map(|tool_result_data: serde_json::Value| {
                            let call_id = tool_result_data.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let name = tool_result_data.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let result_val = tool_result_data.get("result").cloned().unwrap_or(serde_json::Value::Null);
                            Some(SagittaCodeStreamChunk {
                                part: MessagePart::ToolResult { tool_call_id: call_id, name, result: result_val },
                                is_final: chunk.is_final,
                                finish_reason: None,
                                token_usage: None,
                            })
                        })
                }
                _ => Ok(None), 
            }
        };

        match sagitta_code_chunk_conversion_result {
            Ok(Some(sagitta_code_chunk)) => {
                if self.output_chunk_sender.send(Ok(sagitta_code_chunk)).is_err() {
                    return Err(ReasoningError::streaming("output_channel_closed".to_string(), "Error sending SagittaCodeStreamChunk".to_string()));
                }
            }
            Ok(None) => {}
            Err(sagitta_code_error) => {
                if self.output_chunk_sender.send(Err(sagitta_code_error)).is_err() {
                    return Err(ReasoningError::streaming("output_channel_closed_sending_error".to_string(), "Error sending SagittaCodeError".to_string()));
                }
            }
        }
        Ok(())
    }

    async fn handle_stream_complete(&self, _stream_id: Uuid) -> ReasoningEngineResult<()> {
        let final_chunk = SagittaCodeStreamChunk {
            part: MessagePart::Text { text: String::new() }, 
            is_final: true,
            finish_reason: Some("REASONING_INTERNAL_STREAM_ENDED".to_string()),
            token_usage: None,
        };
        if self.output_chunk_sender.send(Ok(final_chunk)).is_err() {
            // eprintln!("Output channel closed before sending final stream complete marker.");
        }
        Ok(())
    }

    async fn handle_stream_error(&self, stream_id: Uuid, error: ReasoningError) -> ReasoningEngineResult<()> {
        let sagitta_code_error = SagittaCodeError::ReasoningError(format!("Internal stream {} failed: {}", stream_id, error));
        if self.output_chunk_sender.send(Err(sagitta_code_error)).is_err() {
            return Err(ReasoningError::streaming("output_channel_closed_on_error".to_string(), "Error sending SagittaCodeError after stream error".to_string()));
        }
        Ok(())
    }
}

pub struct AgentStatePersistence {}

impl AgentStatePersistence {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl StatePersistence for AgentStatePersistence {
    async fn save_state(&self, session_id: Uuid, state_bytes: &[u8]) -> ReasoningEngineResult<()> {
        println!("AgentStatePersistence: Saving state for session {}", session_id);
        // Placeholder: actual save logic for state_bytes
        // Example: std::fs::write(format!("{}_state.bin", session_id), state_bytes).map_err(|e| ReasoningError::state("save_error".to_string(), e.to_string()))?;
        Ok(())
    }

    async fn load_state(&self, session_id: Uuid) -> ReasoningEngineResult<Option<Vec<u8>>> {
        println!("AgentStatePersistence: Loading state for session {}", session_id);
        // Placeholder: actual load logic
        // Example: match std::fs::read(format!("{}_state.bin", session_id)) {
        //     Ok(bytes) => Ok(Some(bytes)),
        //     Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        //     Err(e) => Err(ReasoningError::state("load_error".to_string(), e.to_string())),
        // }
        Ok(None)
    }

    async fn delete_state(&self, _session_id: Uuid) -> ReasoningEngineResult<()> {
        Ok(())
    }

    async fn list_states(&self) -> ReasoningEngineResult<Vec<Uuid>> {
        Ok(Vec::new())
    }
}

pub struct AgentMetricsCollector {}

impl AgentMetricsCollector {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl MetricsCollector for AgentMetricsCollector {
    async fn record_counter(&self, counter_name: &str, value: u64, tags: HashMap<String, String>) -> ReasoningEngineResult<()> {
        println!("AgentMetricsCollector: Recording counter: {}, value: {}, tags: {:?}", counter_name, value, tags);
        Ok(())
    }

    async fn record_gauge(&self, gauge_name: &str, value: f64, tags: HashMap<String, String>) -> ReasoningEngineResult<()> {
        println!("AgentMetricsCollector: Recording gauge: {}, value: {}, tags: {:?}", gauge_name, value, tags);
        Ok(())
    }

    async fn record_histogram(&self, histogram_name: &str, value: f64, tags: HashMap<String, String>) -> ReasoningEngineResult<()> {
        println!("AgentMetricsCollector: Recording histogram: {}, value: {}, tags: {:?}", histogram_name, value, tags);
        Ok(())
    }
    // record_timing has a default implementation in the trait
}

pub use config::create_reasoning_config;
pub use llm_adapter::ReasoningLlmClientAdapter;
pub use intent_analyzer::SagittaCodeIntentAnalyzer;

#[cfg(test)]
mod reasoning_tests {
    use super::*;
    use tokio::sync::broadcast;
    use uuid::Uuid;
    use chrono::Utc;
    use crate::tools::types::ToolDefinition;
    use crate::tools::types::ToolCategory;
    use serde_json::json;
    use tokio::time::{sleep, Duration};
    
    async fn create_test_executor() -> AgentToolExecutor {
        let tool_registry = Arc::new(crate::tools::registry::ToolRegistry::new());
        AgentToolExecutor::new(tool_registry)
    }
    
    #[tokio::test]
    async fn test_smart_loop_detection_for_add_repository() {
        let executor = create_test_executor().await;
        
        // Test that different repository names are NOT considered loops
        let args1 = json!({"name": "repo1", "local_path": "/path/to/repo1"});
        let args2 = json!({"name": "repo2", "local_path": "/path/to/repo2"});
        
        assert!(executor.check_loop_detection("add_existing_repository", &args1).await.is_none());
        assert!(executor.check_loop_detection("add_existing_repository", &args2).await.is_none());
        
        // Create a new executor for the identical calls test to avoid interference
        let executor2 = create_test_executor().await;
        
        // Test that same repository name and path ARE considered loops after threshold
        let args_same = json!({"name": "repo1", "local_path": "/path/to/repo1"});
        
        // First call should not trigger loop detection
        let result1 = executor2.check_loop_detection("add_existing_repository", &args_same).await;
        assert!(result1.is_none());
        
        // Second similar call should trigger loop detection (threshold is 1 for add_existing_repository)
        let loop_info = executor2.check_loop_detection("add_existing_repository", &args_same).await;
        
        assert!(loop_info.is_some());
        let info = loop_info.unwrap();
        assert_eq!(info.tool_name, "add_existing_repository");
        assert_eq!(info.identical_calls, 2); // Should be 2 (previous + current)
    }
    
    #[tokio::test]
    async fn test_parameter_variations_not_considered_loops() {
        let executor = create_test_executor().await;
        
        // Test that different branches for same repo are NOT considered identical
        let args1 = json!({"name": "repo1", "url": "https://github.com/user/repo.git", "branch": "main"});
        let args2 = json!({"name": "repo1", "url": "https://github.com/user/repo.git", "branch": "develop"});
        
        // These should NOT be considered loops because branch differences are allowed
        assert!(executor.check_loop_detection("add_existing_repository", &args1).await.is_none());
        assert!(executor.check_loop_detection("add_existing_repository", &args2).await.is_none());
        
        // Create new executor for identical parameters test
        let executor2 = create_test_executor().await;
        
        // But same name and URL with same branch should eventually trigger
        let args_same = json!({"name": "repo1", "url": "https://github.com/user/repo.git", "branch": "main"});
        assert!(executor2.check_loop_detection("add_existing_repository", &args_same).await.is_none());
        assert!(executor2.check_loop_detection("add_existing_repository", &args_same).await.is_some());
    }
    
    #[tokio::test]
    async fn test_shell_command_similarity_detection() {
        let executor = create_test_executor().await;
        
        // Different base commands should NOT be considered similar
        let cmd1 = json!({"command": "ls -la"});
        let cmd2 = json!({"command": "pwd"});
        
        assert!(executor.check_loop_detection("shell_execution", &cmd1).await.is_none());
        assert!(executor.check_loop_detection("shell_execution", &cmd2).await.is_none());
        assert!(executor.check_loop_detection("shell_execution", &cmd1).await.is_none());
        assert!(executor.check_loop_detection("shell_execution", &cmd2).await.is_none());
        
        // Very similar commands should be considered loops
        let cmd_similar = json!({"command": "cargo build"});
        
        // Create new executor for this test to avoid interference
        let executor2 = create_test_executor().await;
        
        // Make enough calls to trigger the threshold (threshold is 3 for shell_execution)
        // Need 4 total calls to trigger: 3 previous similar calls + 1 current = 4 total
        assert!(executor2.check_loop_detection("shell_execution", &cmd_similar).await.is_none()); // 1st call
        assert!(executor2.check_loop_detection("shell_execution", &cmd_similar).await.is_none()); // 2nd call
        assert!(executor2.check_loop_detection("shell_execution", &cmd_similar).await.is_none()); // 3rd call
        
        // 4th call should trigger the loop detection
        let loop_info = executor2.check_loop_detection("shell_execution", &cmd_similar).await;
        assert!(loop_info.is_some());
    }
    
    #[tokio::test]
    async fn test_command_similarity_calculation() {
        let executor = create_test_executor().await;
        
        // Test identical commands
        assert_eq!(executor.calculate_command_similarity("cargo build", "cargo build"), 1.0);
        
        // Test completely different commands
        assert_eq!(executor.calculate_command_similarity("cargo build", "npm install"), 0.0);
        
        // Test similar commands
        let similarity = executor.calculate_command_similarity("cargo build", "cargo build --release");
        assert!(similarity > 0.5 && similarity < 1.0);
        
        // Test empty commands
        assert_eq!(executor.calculate_command_similarity("", ""), 1.0);
        assert_eq!(executor.calculate_command_similarity("", "test"), 0.0);
    }
    
    #[tokio::test]
    async fn test_parameter_similarity_calculation() {
        let executor = create_test_executor().await;
        
        // Test identical objects
        let obj1 = json!({"name": "test", "value": 42});
        assert_eq!(executor.calculate_parameter_similarity(&obj1, &obj1), 1.0);
        
        // Test completely different objects
        let obj2 = json!({"different": "completely"});
        let similarity = executor.calculate_parameter_similarity(&obj1, &obj2);
        assert!(similarity < 0.5);
        
        // Test partially similar objects
        let obj3 = json!({"name": "test", "value": 43, "extra": "field"});
        let similarity2 = executor.calculate_parameter_similarity(&obj1, &obj3);
        // obj1 has 2 keys, obj3 has 3 keys, 1 key matches exactly (name)
        // So similarity should be 1/3 = 0.33... which is < 0.5
        assert!(similarity2 > 0.3 && similarity2 < 0.5);
        
        // Test more similar objects
        let obj4 = json!({"name": "test", "value": 42, "extra": "field"});
        let similarity3 = executor.calculate_parameter_similarity(&obj1, &obj4);
        // obj1 has 2 keys, obj4 has 3 keys, 2 keys match exactly
        // So similarity should be 2/3 = 0.66... which is > 0.5
        assert!(similarity3 > 0.6 && similarity3 < 1.0);
    }
    
    #[tokio::test]
    async fn test_tool_specific_thresholds() {
        let executor = create_test_executor().await;
        
        // Test that different tools have different thresholds
        assert_eq!(executor.get_loop_threshold_for_tool("add_existing_repository"), 1);
        assert_eq!(executor.get_loop_threshold_for_tool("sync_repository"), 1);
        assert_eq!(executor.get_loop_threshold_for_tool("shell_execution"), 3);
        assert_eq!(executor.get_loop_threshold_for_tool("view_file"), 4);
        assert_eq!(executor.get_loop_threshold_for_tool("unknown_tool"), executor.max_identical_calls - 1);
    }
    
    #[tokio::test]
    async fn test_time_based_loop_detection_cleanup() {
        let executor = create_test_executor().await;
        
        // Add a call and verify it's tracked
        let args = json!({"name": "test-repo", "local_path": "/test"});
        assert!(executor.check_loop_detection("add_existing_repository", &args).await.is_none());
        
        // Verify the call is in recent_calls
        {
            let recent_calls = executor.recent_tool_calls.lock().await;
            assert_eq!(recent_calls.len(), 1);
        }
        
        // Wait a short time and add another call
        sleep(Duration::from_millis(100)).await;
        assert!(executor.check_loop_detection("add_existing_repository", &args).await.is_some());
        
        // Verify both calls are still there
        {
            let recent_calls = executor.recent_tool_calls.lock().await;
            assert_eq!(recent_calls.len(), 2);
        }
    }
    
    #[tokio::test]
    async fn test_context_aware_repository_similarity() {
        let executor = create_test_executor().await;
        
        // Test that URL vs local_path for same repo are considered different (different path sources)
        let args_url = json!({"name": "myrepo", "url": "https://github.com/user/myrepo.git"});
        let args_local = json!({"name": "myrepo", "local_path": "/path/to/myrepo"});
        
        // These should be considered different (different path sources)
        assert!(!executor.are_parameters_significantly_similar("add_existing_repository", &args_url, &args_local));
        
        // Same URL but different branches should be considered different (to avoid loops)
        let args_url2 = json!({"name": "myrepo", "url": "https://github.com/user/myrepo.git", "branch": "develop"});
        assert!(!executor.are_parameters_significantly_similar("add_existing_repository", &args_url, &args_url2));
        
        // But same URL with same branch should be considered similar
        let args_url3 = json!({"name": "myrepo", "url": "https://github.com/user/myrepo.git"});
        assert!(executor.are_parameters_significantly_similar("add_existing_repository", &args_url, &args_url3));
    }

    // Legacy tests (keeping existing ones that are still relevant)
    #[tokio::test]
    async fn test_summary_event_conversion() {
        let (tx, _rx) = broadcast::channel(16);
        let emitter = AgentEventEmitter::new(tx);
        
        let reasoning_event = ReasoningEvent::Summary {
            session_id: Uuid::new_v4(),
            content: "Test summary".to_string(),
            timestamp: chrono::Utc::now(),
        };
        
        // This should not panic
        let _ = emitter.emit_event(reasoning_event).await;
    }
    
    #[tokio::test]
    async fn test_response_deduplication() {
        let (tx, mut rx) = broadcast::channel(16);
        let emitter = AgentEventEmitter::new(tx);
        
        // Send the same content twice
        emitter.emit_streaming_text("Hello world".to_string(), false).await;
        emitter.emit_streaming_text("Hello world".to_string(), false).await;
        
        // Should only receive one event due to deduplication
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, AgentEvent::LlmChunk { .. }));
        
        // Second event should be filtered out
        let result = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_err()); // Timeout because no event was sent
    }
    
    #[tokio::test]
    async fn test_hash_computation() {
        let (tx, _rx) = broadcast::channel(16);
        let emitter = AgentEventEmitter::new(tx);
        
        let hash1 = emitter.compute_content_hash("test content");
        let hash2 = emitter.compute_content_hash("test content");
        let hash3 = emitter.compute_content_hash("different content");
        
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 40); // SHA-1 produces 40 character hex string
    }
    
    #[tokio::test]
    async fn test_tool_execution_started_event_conversion() {
        let (tx, _rx) = broadcast::channel(16);
        let emitter = AgentEventEmitter::new(tx);
        
        let reasoning_event = ReasoningEvent::ToolExecutionStarted {
            session_id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            tool_args: serde_json::json!({"param": "value"}),
        };
        
        // This should not panic
        let _ = emitter.emit_event(reasoning_event).await;
    }
    
    #[tokio::test]
    async fn test_session_completed_event_conversion() {
        let (tx, _rx) = broadcast::channel(16);
        let emitter = AgentEventEmitter::new(tx);
        
        let reasoning_event = ReasoningEvent::SessionCompleted {
            session_id: Uuid::new_v4(),
            success: true,
            total_duration_ms: 1000,
            steps_executed: 5,
            tools_used: vec!["tool1".to_string(), "tool2".to_string()],
        };
        
        // This should not panic
        let _ = emitter.emit_event(reasoning_event).await;
    }
    
    #[tokio::test]
    async fn test_tool_execution_completed_icons() {
        let (tx, mut rx) = broadcast::channel(16);
        let emitter = AgentEventEmitter::new(tx);
        
        // Test successful tool execution
        let success_event = ReasoningEvent::ToolExecutionCompleted {
            session_id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            success: true,
            duration_ms: 100,
        };
        
        let _ = emitter.emit_event(success_event).await;
        
        // The emitter might emit multiple events (streaming text + ToolCompleted)
        // We need to find the ToolCompleted event specifically
        let mut found_tool_completed = false;
        for _ in 0..3 { // Try to receive up to 3 events
            if let Ok(event) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                if let Ok(agent_event) = event {
                    if let AgentEvent::ToolCompleted { tool_name, success, duration_ms } = agent_event {
                        assert_eq!(tool_name, "test_tool");
                        assert_eq!(success, true);
                        assert_eq!(duration_ms, 100);
                        found_tool_completed = true;
                        break;
                    }
                }
            } else {
                break; // Timeout, no more events
            }
        }
        
        assert!(found_tool_completed, "Expected ToolCompleted event was not found");
    }
} 