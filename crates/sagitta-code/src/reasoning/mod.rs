use std::sync::Arc;
use uuid::Uuid;
use std::collections::{HashMap, HashSet};
use terminal_stream::events::StreamEvent;
use sha1::{Sha1, Digest};

pub mod config;
pub mod llm_adapter;
pub mod intent_analyzer;

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

use crate::{agent::events::AgentEvent, tools::{registry::ToolRegistry, types::Tool}, llm::client::{MessagePart, StreamChunk as SagittaCodeStreamChunk}, utils::errors::SagittaCodeError};

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
    
    /// Enhanced loop detection with recovery strategy determination
    async fn check_loop_detection(&self, tool_name: &str, args: &Value) -> Option<LoopDetectionInfo> {
        let mut recent_calls = self.recent_tool_calls.lock().await;
        let now = std::time::Instant::now();
        
        // Clean up old calls (older than 60 seconds)
        recent_calls.retain(|(_, _, timestamp)| now.duration_since(*timestamp).as_secs() < 60);
        
        // Check for identical calls in the last 30 seconds BEFORE adding current call
        let identical_count = recent_calls
            .iter()
            .filter(|(name, call_args, timestamp)| {
                name == tool_name && 
                call_args == args && 
                now.duration_since(*timestamp).as_secs() < 30
            })
            .count();
        
        // Add current call to history AFTER counting
        let current_call = (tool_name.to_string(), args.clone(), now);
        recent_calls.push(current_call);
        
        // Keep only recent calls (last 15) to prevent memory growth
        if recent_calls.len() > 15 {
            let len = recent_calls.len();
            recent_calls.drain(0..len - 15);
        }
        
        if identical_count >= self.max_identical_calls {
            // Determine recovery strategy based on tool type and failure context
            let recovery_strategy = self.determine_recovery_strategy(tool_name, args).await;
            
            Some(LoopDetectionInfo {
                tool_name: tool_name.to_string(),
                identical_calls: identical_count + 1, // +1 to include current call
                last_call_time: now,
                suggested_recovery: recovery_strategy,
            })
        } else {
            None
        }
    }
    
    /// Determine the best recovery strategy for a looping tool
    async fn determine_recovery_strategy(&self, tool_name: &str, _args: &Value) -> RecoveryStrategy {
        let failed_tools = self.failed_tools.lock().await;
        let failure_count = failed_tools.get(tool_name).unwrap_or(&0);
        
        match tool_name {
            // Repository tools can often be skipped if they fail
            "add_repository" | "sync_repository" => {
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
            });
            
            let _ = event_sender.send(AgentEvent::Error(error_message.to_string()));
        }
    }
    
    /// Generate helpful parameter error feedback with specific suggestions
    async fn generate_parameter_error_feedback(&self, tool_name: &str, error_message: &str) -> String {
        match tool_name {
            "add_repository" => {
                if error_message.contains("oneOf") || error_message.contains("parameter combinations") || error_message.contains("Either URL or existing local repository path") {
                    format!("⚠️ Tool 'add_repository' parameter error: {}. \n\n**Quick Fix**: You must provide EITHER:\n• `url` (for remote repositories): {{\"name\": \"repo-name\", \"url\": \"https://github.com/user/repo.git\"}}\n• `local_path` (for local directories): {{\"name\": \"repo-name\", \"local_path\": \"/path/to/directory\"}}\n\n**Alternative**: If you're trying to create a new project, use the `create_project` tool instead.", error_message)
                } else {
                    format!("⚠️ Tool 'add_repository' failed: {}. \n\n**Quick Fix**: Try specifying either 'url' OR 'local_path' parameter.\n• For remote repos: provide `url`\n• For local directories: provide `local_path`", error_message)
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
            "add_repository" => "try 'create_project' for new projects, or verify the repository path/URL".to_string(),
            "search_file_in_repository" => "try 'query' for general searches or check repository is synced".to_string(),
            "sync_repository" => "try 'add_repository' again or check network connection".to_string(),
            "shell_execution" => "try breaking the command into smaller steps or check permissions".to_string(),
            _ => "check tool parameters or try a different approach".to_string(),
        }
    }
    
    /// Suggest execution recovery strategies
    async fn suggest_execution_recovery(&self, tool_name: &str) -> String {
        match tool_name {
            "add_repository" => "Consider using an absolute path for local repositories or verify the Git URL is accessible",
            "shell_execution" => "Try breaking complex commands into simpler steps or check if the command is available",
            "sync_repository" => "Check network connectivity and repository permissions",
            _ => "Please verify the tool parameters and try again with corrected values",
        }.to_string()
    }
    
    /// Suggest general recovery approaches
    async fn suggest_general_recovery(&self, tool_name: &str) -> String {
        match tool_name {
            "add_repository" => "For troubleshooting: ensure Git is installed, check repository URL/path exists, and verify permissions",
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
            "add_repository" => {
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
    
    /// Execute shell command with streaming when terminal sender is available
    async fn execute_streaming_shell_tool(&self, name: &str, args: Value) -> ReasoningEngineResult<ToolResult> {
        // Get the streaming shell execution tool
        let streaming_tool = self
            .tool_registry
            .get("streaming_shell_execution")
            .await
            .ok_or_else(|| ReasoningError::tool_execution(name.to_string(), "Streaming shell execution tool not found".to_string()))?;

        // Parse shell execution parameters
        let shell_params: crate::tools::shell_execution::ShellExecutionParams = serde_json::from_value(args.clone())
            .map_err(|e| ReasoningError::tool_execution(name.to_string(), format!("Invalid shell execution parameters: {}", e)))?;

        // Cast to streaming shell tool
        let streaming_shell_tool = streaming_tool
            .as_any()
            .downcast_ref::<crate::tools::shell_execution::StreamingShellExecutionTool>()
            .ok_or_else(|| ReasoningError::tool_execution(name.to_string(), "Failed to cast to StreamingShellExecutionTool".to_string()))?;

        // Get terminal event sender
        let terminal_sender = self.terminal_event_sender.as_ref()
            .ok_or_else(|| ReasoningError::tool_execution(name.to_string(), "Terminal event sender not configured".to_string()))?;

        // Execute with streaming
        match streaming_shell_tool.execute_streaming(shell_params, terminal_sender.clone()).await {
            Ok(shell_result) => {
                let result_value = serde_json::to_value(shell_result)
                    .map_err(|e| ReasoningError::tool_execution(name.to_string(), format!("Failed to serialize shell result: {}", e)))?;
                
                // Post-process the result for better completion confidence
                let processed_result = self.post_process_shell_result(&result_value).unwrap_or(result_value);
                
                Ok(ToolResult {
                    success: true,
                    data: processed_result,
                    error: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
            Err(error) => {
                Err(ReasoningError::tool_execution(name.to_string(), error.to_string()))
            }
        }
    }
    
    /// Post-process shell execution results to provide better context and completion confidence
    fn post_process_shell_result(&self, result_value: &Value) -> Option<Value> {
        // Try to extract meaningful information from shell execution results
        if let Some(result_obj) = result_value.as_object() {
            if let (Some(stdout), Some(exit_code)) = (
                result_obj.get("stdout").and_then(|v| v.as_str()),
                result_obj.get("exit_code").and_then(|v| v.as_i64())
            ) {
                if exit_code == 0 && !stdout.trim().is_empty() {
                    // Try to detect common patterns and provide helpful summaries
                    let stdout_trimmed = stdout.trim();
                    
                    // Line counting commands (wc -l, find | wc -l, etc.)
                    if let Ok(line_count) = stdout_trimmed.parse::<u64>() {
                        if line_count > 0 {
                            let mut enhanced_result = result_obj.clone();
                            enhanced_result.insert(
                                "summary".to_string(),
                                Value::String(format!("Command completed successfully. Total count: {}", line_count))
                            );
                            return Some(Value::Object(enhanced_result));
                        }
                    }
                    
                    // Multi-line output with numbers (like detailed file counts)
                    if stdout_trimmed.lines().count() > 1 {
                        let lines: Vec<&str> = stdout_trimmed.lines().collect();
                        if let Some(last_line) = lines.last() {
                            if last_line.contains("total") || last_line.trim().parse::<u64>().is_ok() {
                                let mut enhanced_result = result_obj.clone();
                                enhanced_result.insert(
                                    "summary".to_string(),
                                    Value::String(format!("Command completed successfully. Result summary: {}", last_line.trim()))
                                );
                                return Some(Value::Object(enhanced_result));
                            }
                        }
                    }
                    
                    // For other successful commands, add a basic summary
                    if stdout_trimmed.len() < 200 {
                        let mut enhanced_result = result_obj.clone();
                        enhanced_result.insert(
                            "summary".to_string(),
                            Value::String(format!("Command completed successfully. Output: {}", stdout_trimmed))
                        );
                        return Some(Value::Object(enhanced_result));
                    } else {
                        let mut enhanced_result = result_obj.clone();
                        enhanced_result.insert(
                            "summary".to_string(),
                            Value::String(format!("Command completed successfully. Output length: {} characters", stdout_trimmed.len()))
                        );
                        return Some(Value::Object(enhanced_result));
                    }
                }
            }
        }
        
        None
    }

    /// Backward compatibility method for surface_error_to_llm
    async fn surface_error_to_llm(&self, tool_name: &str, error_message: &str, error_code: &str) {
        self.surface_error_to_llm_with_recovery(tool_name, error_message, error_code, None).await;
    }
    
    /// Get recovery suggestions for workflow continuation
    pub async fn get_workflow_continuation_suggestions(&self, failed_tool: &str) -> Vec<String> {
        match failed_tool {
            "add_repository" => vec![
                "Continue with existing repositories in the workspace".to_string(),
                "Use 'create_project' to start a new project instead".to_string(),
                "Skip repository setup and work with local files".to_string(),
            ],
            "sync_repository" => vec![
                "Continue with existing repository state".to_string(),
                "Try manual repository refresh later".to_string(),
                "Work with current cached repository content".to_string(),
            ],
            "search_file_in_repository" => vec![
                "Use 'query' for general text search".to_string(),
                "Browse repository structure manually".to_string(),
                "Ask user to specify exact file paths".to_string(),
            ],
            "shell_execution" => vec![
                "Break command into smaller steps".to_string(),
                "Use alternative tools for the same task".to_string(),
                "Ask user to run commands manually".to_string(),
            ],
            _ => vec![
                "Continue with remaining workflow steps".to_string(),
                "Try alternative approaches for this task".to_string(),
                "Ask user for clarification or alternative approach".to_string(),
            ],
        }
    }
    
    /// Check if workflow can continue without this tool
    pub async fn can_workflow_continue_without(&self, tool_name: &str) -> bool {
        match tool_name {
            // Repository management tools - workflow can usually continue
            "add_repository" | "sync_repository" | "remove_repository" => true,
            // Search tools - alternatives usually exist
            "search_file_in_repository" | "query" => true,
            // File operations - often non-critical
            "read_file" | "edit_file" => false, // These are often critical
            // Shell execution - depends on context but often has alternatives
            "shell_execution" => true,
            // Project creation - often critical for new project workflows
            "create_project" => false,
            // Default: assume tools are important but workflow can continue
            _ => true,
        }
    }
}

#[async_trait]
impl ToolExecutor for AgentToolExecutor {
    async fn execute_tool(&self, name: &str, args: Value) -> ReasoningEngineResult<ToolResult> {
        // Check if this tool should be skipped due to previous failures
        if self.should_skip_tool(name).await {
            let skip_message = format!("Skipping tool '{}' due to repeated failures. Continuing with workflow.", name);
            self.surface_error_to_llm_with_recovery(name, &skip_message, "tool_skipped", None).await;
            
            return Ok(ToolResult {
                success: false,
                data: serde_json::json!({
                    "skipped": true,
                    "reason": "Tool skipped due to repeated failures",
                    "message": skip_message
                }),
                error: Some(skip_message),
                execution_time_ms: 0,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("error_code".to_string(), Value::String("tool_skipped".to_string()));
                    meta.insert("recovery_action".to_string(), Value::String("skipped".to_string()));
                    meta
                },
            });
        }
        
        // Enhanced loop detection with recovery strategies
        if let Some(loop_info) = self.check_loop_detection(name, &args).await {
            // Handle different recovery strategies
            match loop_info.suggested_recovery {
                RecoveryStrategy::Skip => {
                    // Mark tool as skipped for future calls
                    self.mark_tool_as_skipped(name).await;
                    
                    let skip_message = format!("Loop detected for tool '{}'. Skipping to avoid blocking workflow progress.", name);
                    self.surface_error_to_llm_with_recovery(name, &skip_message, "loop_detected", Some(loop_info.clone())).await;
                    
                    return Ok(ToolResult {
                        success: false,
                        data: serde_json::json!({
                            "skipped": true,
                            "reason": "Loop detected - tool skipped",
                            "loop_info": {
                                "identical_calls": loop_info.identical_calls,
                                "recovery_strategy": "Skip"
                            }
                        }),
                        error: Some(skip_message),
                        execution_time_ms: 0,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("error_code".to_string(), Value::String("loop_detected".to_string()));
                            meta.insert("recovery_action".to_string(), Value::String("skipped".to_string()));
                            meta.insert("loop_count".to_string(), Value::Number(loop_info.identical_calls.into()));
                            meta
                        },
                    });
                }
                RecoveryStrategy::Stop => {
                    let stop_message = format!("Critical loop detected for essential tool '{}'. Workflow cannot continue.", name);
                    self.surface_error_to_llm_with_recovery(name, &stop_message, "loop_detected", Some(loop_info.clone())).await;
                    
                    return Ok(ToolResult {
                        success: false,
                        data: Value::Null,
                        error: Some(stop_message),
                        execution_time_ms: 0,
                        metadata: {
                            let mut meta = HashMap::new();
                            meta.insert("error_code".to_string(), Value::String("loop_detected".to_string()));
                            meta.insert("recovery_action".to_string(), Value::String("stop".to_string()));
                            meta.insert("loop_count".to_string(), Value::Number(loop_info.identical_calls.into()));
                            meta.insert("critical".to_string(), Value::Bool(true));
                            meta
                        },
                    });
                }
                RecoveryStrategy::Alternative => {
                    // Provide feedback and mark as failed, but allow one more attempt
                    // If this is the second alternative attempt, escalate to Skip
                    let failure_count = {
                        let failed_tools = self.failed_tools.lock().await;
                        failed_tools.get(name).unwrap_or(&0).clone()
                    };
                    
                    if failure_count >= 2 {
                        // Escalate to Skip after multiple alternative attempts
                        self.mark_tool_as_skipped(name).await;
                        let skip_message = format!("Multiple loop attempts detected for tool '{}'. Skipping to avoid infinite loops.", name);
                        self.surface_error_to_llm_with_recovery(name, &skip_message, "loop_detected", Some(loop_info.clone())).await;
                        
                        return Ok(ToolResult {
                            success: false,
                            data: serde_json::json!({
                                "skipped": true,
                                "reason": "Multiple loop attempts - tool skipped",
                                "loop_info": {
                                    "identical_calls": loop_info.identical_calls,
                                    "recovery_strategy": "Skip"
                                }
                            }),
                            error: Some(skip_message),
                            execution_time_ms: 0,
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("error_code".to_string(), Value::String("loop_detected".to_string()));
                                meta.insert("recovery_action".to_string(), Value::String("skipped".to_string()));
                                meta.insert("loop_count".to_string(), Value::Number(loop_info.identical_calls.into()));
                                meta
                            },
                        });
                    } else {
                        // First alternative attempt - provide feedback and track failure
                        self.track_tool_failure(name).await;
                        self.surface_error_to_llm_with_recovery(name, "Loop detected - please try alternative approach", "loop_detected", Some(loop_info.clone())).await;
                        // Continue with execution but this is tracked as a failure
                    }
                }
                RecoveryStrategy::Retry => {
                    // Track failure and provide feedback, then continue with execution
                    self.track_tool_failure(name).await;
                    self.surface_error_to_llm_with_recovery(name, "Loop detected - retrying with caution", "loop_detected", Some(loop_info.clone())).await;
                    // Continue with execution
                }
            }
        }

        // Enhanced parameter validation with detailed feedback
        if let Err(validation_error) = self.validate_tool_parameters(name, &args).await {
            let error_message = format!("Parameter validation failed: {}", validation_error);
            self.track_tool_failure(name).await;
            self.surface_error_to_llm_with_recovery(name, &error_message, "invalid_parameters", None).await;
            
            return Ok(ToolResult {
                success: false,
                data: Value::Null,
                error: Some(error_message),
                execution_time_ms: 0,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("error_code".to_string(), Value::String("invalid_parameters".to_string()));
                    meta.insert("validation_error".to_string(), Value::String(validation_error));
                    meta
                },
            });
        }

        // Special handling for shell_execution when terminal streaming is available
        if name == "shell_execution" && self.terminal_event_sender.is_some() {
            return self.execute_streaming_shell_tool(name, args).await;
        }
        
        let tool = self
            .tool_registry
            .get(name)
            .await
            .ok_or_else(|| {
                // Surface tool not found error to LLM with helpful suggestions
                tokio::spawn({
                    let executor = self.clone();
                    let tool_name = name.to_string();
                    async move {
                        executor.surface_error_to_llm_with_recovery(&tool_name, "Tool not found in registry", "tool_not_found", None).await;
                    }
                });
                ReasoningError::tool_execution(name.to_string(), "Tool not found".to_string())
            })?;

        match tool.execute(args.clone()).await {
            Ok(sagitta_code_tool_result) => {
                match sagitta_code_tool_result {
                    crate::tools::types::ToolResult::Success(val) => {
                        // Reset failure tracking for successful execution
                        self.reset_tool_failure_tracking(name).await;
                        
                        // Post-process shell execution results to provide better completion confidence
                        let processed_result = if name == "shell_execution" {
                            self.post_process_shell_result(&val).unwrap_or(val)
                        } else {
                            val
                        };
                        
                        Ok(ToolResult {
                            success: true,
                            data: processed_result,
                            error: None,
                            execution_time_ms: 0, // Placeholder for duration
                            metadata: HashMap::new(),
                        })
                    },
                    crate::tools::types::ToolResult::Error { error: msg } => {
                        // Track failure and provide enhanced error feedback
                        self.track_tool_failure(name).await;
                        self.surface_error_to_llm_with_recovery(name, &msg, "execution_error", None).await;
                        
                        // Check if this tool should now be skipped due to repeated failures
                        if self.should_skip_tool(name).await {
                            let skip_message = format!("Tool '{}' has failed repeatedly and will be skipped in future calls. Consider using alternative approaches.", name);
                            self.surface_error_to_llm_with_recovery(name, &skip_message, "tool_marked_for_skipping", None).await;
                        }
                        
                        Ok(ToolResult { 
                            success: false,
                            data: Value::Null,
                            error: Some(msg),
                            execution_time_ms: 0, // Placeholder for duration
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("error_code".to_string(), Value::String("execution_error".to_string()));
                                meta
                            },
                        })
                    },
                }
            }
            Err(sagitta_code_error) => {
                // Track system error and provide recovery suggestions
                self.track_tool_failure(name).await;
                let error_msg = sagitta_code_error.to_string();
                self.surface_error_to_llm_with_recovery(name, &error_msg, "system_error", None).await;
                
                Err(ReasoningError::tool_execution(
                    name.to_string(),
                    error_msg,
                ))
            }
        }
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
                // Emit streaming text for tool execution completion
                let status_icon = if success { "✅" } else { "❌" };
                self.emit_streaming_text(
                    format!("\n{} Tool **{}** completed in {}ms\n\n", status_icon, tool_name, duration_ms),
                    false
                ).await;
                
                AgentEvent::ToolCompleted {
                    tool_name,
                    success,
                    duration_ms,
                }
            }
            ReasoningEvent::Summary { session_id: _, content, timestamp: _ } => {
                // Don't emit LlmChunk here - the AgentStreamHandler already handles this
                // Just emit a log for tracking
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
                    format!("❌ Error: {}", error_message),
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
mod tests {
    use super::*;
    use tokio::sync::broadcast;
    use uuid::Uuid;
    use chrono::Utc;

    #[tokio::test]
    async fn test_summary_event_conversion() {
        let (sender, _receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);
        
        let summary_event = ReasoningEvent::Summary {
            session_id: Uuid::new_v4(),
            content: "Test summary content".to_string(),
            timestamp: Utc::now(),
        };
        
        let result = emitter.emit_event(summary_event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_response_deduplication() {
        let (sender, mut receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);
        
        // Send the same content twice
        let content = "This is a test message that should be deduplicated";
        
        // First emission - should go through
        emitter.emit_streaming_text(content.to_string(), false).await;
        
        // Second emission with same content - should be dropped
        emitter.emit_streaming_text(content.to_string(), false).await;
        
        // Third emission with different content - should go through
        emitter.emit_streaming_text("Different content".to_string(), false).await;
        
        // Verify only 2 events were sent (first and third)
        let mut received_count = 0;
        let mut received_contents = Vec::new();
        
        // Use try_recv to avoid blocking
        while let Ok(event) = receiver.try_recv() {
            match event {
                AgentEvent::LlmChunk { content, .. } => {
                    received_count += 1;
                    received_contents.push(content);
                }
                _ => {}
            }
        }
        
        assert_eq!(received_count, 2, "Should have received exactly 2 events (duplicate filtered out)");
        assert_eq!(received_contents[0], "This is a test message that should be deduplicated");
        assert_eq!(received_contents[1], "Different content");
    }

    #[tokio::test]
    async fn test_hash_computation() {
        let (sender, _receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);
        
        let content1 = "Hello, world!";
        let content2 = "Hello, world!";
        let content3 = "Different content";
        
        let hash1 = emitter.compute_content_hash(content1);
        let hash2 = emitter.compute_content_hash(content2);
        let hash3 = emitter.compute_content_hash(content3);
        
        assert_eq!(hash1, hash2, "Same content should produce same hash");
        assert_ne!(hash1, hash3, "Different content should produce different hash");
        assert_eq!(hash1.len(), 40, "SHA-1 hash should be 40 characters long");
    }

    #[tokio::test]
    async fn test_tool_execution_started_event_conversion() {
        let (sender, _receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);
        
        let tool_execution_event = ReasoningEvent::ToolExecutionStarted {
            session_id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            tool_args: serde_json::json!({"arg1": "value1"}),
        };
        
        let result = emitter.emit_event(tool_execution_event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_session_completed_event_conversion() {
        let (sender, _receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);
        
        let session_completed_event = ReasoningEvent::SessionCompleted {
            session_id: Uuid::new_v4(),
            success: true,
            total_duration_ms: 1000,
            steps_executed: 5,
            tools_used: vec!["tool1".to_string(), "tool2".to_string()],
        };
        
        let result = emitter.emit_event(session_completed_event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tool_execution_completed_icons() {
        let (sender, _receiver) = broadcast::channel(100);
        let emitter = AgentEventEmitter::new(sender);
        
        // Test successful tool execution
        let success_event = ReasoningEvent::ToolExecutionCompleted {
            session_id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            success: true,
            duration_ms: 500,
        };
        
        let result = emitter.emit_event(success_event).await;
        assert!(result.is_ok());
        
        // Test failed tool execution
        let failure_event = ReasoningEvent::ToolExecutionCompleted {
            session_id: Uuid::new_v4(),
            tool_name: "test_tool".to_string(),
            success: false,
            duration_ms: 300,
        };
        
        let result = emitter.emit_event(failure_event).await;
        assert!(result.is_ok());
    }
} 