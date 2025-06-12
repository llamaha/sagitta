use std::time::Duration;
use crate::error::Result;
use crate::traits::{ToolResult, ValidationStatus, CompletionEvidence, EvidenceType};
use super::types::{ToolExecutionRequest, ValidationOutcome};

/// Handles validation of tool execution results
pub struct ValidationEngine {
    // In a real implementation, this might have configuration for validation rules
}

impl ValidationEngine {
    /// Create a new validation engine
    pub fn new() -> Self {
        Self {}
    }

    /// Validate a tool execution result
    pub async fn validate_tool_execution_result(
        &self,
        request: &ToolExecutionRequest,
        tool_result: &mut ToolResult,
        execution_time: Duration,
    ) -> ValidationOutcome {
        // Check for basic validation status - note: validation_status is NOT optional
        match &tool_result.validation_status {
            ValidationStatus::Validated => {},
            ValidationStatus::NeedsVerification { reason } => {
                return ValidationOutcome::NeedsVerification { 
                    reason: reason.clone() 
                };
            },
            ValidationStatus::Inconsistent { details } => {
                return ValidationOutcome::Inconsistent { 
                    details: details.clone() 
                };
            },
            ValidationStatus::UnableToValidate => {
                return ValidationOutcome::NeedsVerification {
                    reason: "Unable to validate tool result".to_string()
                };
            }
        }

        // Perform tool-specific validation
        if let Err(e) = self.perform_tool_specific_validation(&request.tool_name, &request.parameters, tool_result).await {
            return ValidationOutcome::VerificationFailed { 
                error: e.to_string() 
            };
        }

        // Check completion evidence if present
        if !tool_result.completion_evidence.is_empty() {
            if !self.validate_completion_evidence(&tool_result.completion_evidence) {
                return ValidationOutcome::Inconsistent {
                    details: "Completion evidence is inconsistent".to_string()
                };
            }
        }

        // Check execution time reasonableness
        if let Some(timeout) = request.timeout {
            if execution_time > timeout {
                return ValidationOutcome::Inconsistent {
                    details: format!("Execution time ({:?}) exceeded timeout ({:?})", execution_time, timeout)
                };
            }
        }

        ValidationOutcome::Validated
    }

    /// Perform tool-specific validation
    async fn perform_tool_specific_validation(
        &self,
        tool_name: &str,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        match tool_name {
            "git_clone" | "git_pull" | "git_push" => {
                self.validate_repository_operation(parameters, tool_result).await
            },
            "shell_execute" => {
                self.validate_shell_execution(parameters, tool_result).await
            },
            "file_edit" | "file_create" | "file_delete" => {
                self.validate_file_operation(parameters, tool_result).await
            },
            "sync_repository" => {
                self.validate_sync_operation(parameters, tool_result).await
            },
            _ => {
                self.validate_generic_tool_result(tool_result).await
            }
        }
    }

    /// Validate repository operations
    async fn validate_repository_operation(
        &self,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        // Check if the operation was successful based on the result content
        let content_str = tool_result.data.to_string();
        
        // Look for error indicators
        if content_str.to_lowercase().contains("error") 
            || content_str.to_lowercase().contains("failed") 
            || content_str.to_lowercase().contains("fatal") {
            
            tool_result.validation_status = ValidationStatus::Inconsistent {
                details: "Repository operation appears to have failed based on output".to_string()
            };
            return Ok(());
        }

        // For git operations, look for success indicators
        if content_str.contains("Cloning into") 
            || content_str.contains("Already up to date")
            || content_str.contains("Fast-forward") {
            
            tool_result.validation_status = ValidationStatus::Validated;
        }

        // Check if repository path exists for clone operations
        if let Some(path) = parameters.get("path").and_then(|p| p.as_str()) {
            // In a real implementation, we would check if the directory exists
            // For now, just add evidence that we should verify the path
            tool_result.completion_evidence.push(CompletionEvidence {
                evidence_type: EvidenceType::RepositoryOperation,
                description: format!("Repository should exist at path: {}", path),
                supports_success: true,
                confidence: 0.8,
            });
        }

        Ok(())
    }

    /// Validate shell execution
    async fn validate_shell_execution(
        &self,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        // Check exit code if available
        if let Some(exit_code) = tool_result.metadata.get("exit_code").and_then(|c| c.as_i64()) {
            if exit_code != 0 {
                tool_result.validation_status = ValidationStatus::Inconsistent {
                    details: format!("Shell command failed with exit code: {}", exit_code)
                };
                return Ok(());
            }
        }

        // Look for error patterns in output
        let content_str = tool_result.data.to_string().to_lowercase();
        
        if content_str.contains("command not found") 
            || content_str.contains("permission denied")
            || content_str.contains("no such file") {
            
            tool_result.validation_status = ValidationStatus::Inconsistent {
                details: "Shell command output indicates an error".to_string()
            };
            return Ok(());
        }

        // Check for common error patterns
        if content_str.contains("error:") || content_str.contains("failed:") {
            tool_result.validation_status = ValidationStatus::NeedsVerification {
                reason: "Shell command output contains error messages".to_string()
            };
        }

        // Add execution evidence
        if let Some(command) = parameters.get("command").and_then(|c| c.as_str()) {
            tool_result.completion_evidence.push(CompletionEvidence {
                evidence_type: EvidenceType::CommandOutput,
                description: format!("Shell command executed: {}", command),
                supports_success: true,
                confidence: 0.9,
            });
        }

        Ok(())
    }

    /// Validate file operations
    async fn validate_file_operation(
        &self,
        parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        // Check if file operation was successful
        let content_str = tool_result.data.to_string();
        
        if content_str.to_lowercase().contains("error") 
            || content_str.to_lowercase().contains("failed") 
            || content_str.to_lowercase().contains("permission denied") {
            
            tool_result.validation_status = ValidationStatus::Inconsistent {
                details: "File operation appears to have failed".to_string()
            };
            return Ok(());
        }

        // Add file evidence
        if let Some(file_path) = parameters.get("file_path").and_then(|p| p.as_str()) {
            tool_result.completion_evidence.push(CompletionEvidence {
                evidence_type: EvidenceType::FileSystemChange,
                description: format!("File operation performed on: {}", file_path),
                supports_success: true,
                confidence: 0.8,
            });
        }

        Ok(())
    }

    /// Validate sync operations
    async fn validate_sync_operation(
        &self,
        _parameters: &serde_json::Value,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        // Check for sync success indicators
        let content_str = tool_result.data.to_string();
        
        if content_str.to_lowercase().contains("sync completed")
            || content_str.to_lowercase().contains("synchronized") {
            
            tool_result.validation_status = ValidationStatus::Validated;
        } else if content_str.to_lowercase().contains("sync failed") 
            || content_str.to_lowercase().contains("synchronization error") {
            
            tool_result.validation_status = ValidationStatus::Inconsistent {
                details: "Synchronization operation failed".to_string()
            };
        }

        Ok(())
    }

    /// Generic validation for unknown tools
    async fn validate_generic_tool_result(
        &self,
        tool_result: &mut ToolResult,
    ) -> Result<()> {
        // Generic validation - just check for obvious error patterns
        let content_str = tool_result.data.to_string().to_lowercase();
        
        if content_str.contains("fatal error") 
            || content_str.contains("critical error")
            || content_str.contains("exception") {
            
            tool_result.validation_status = ValidationStatus::Inconsistent {
                details: "Tool output contains error indicators".to_string()
            };
        } else if content_str.contains("warning") {
            tool_result.validation_status = ValidationStatus::NeedsVerification {
                reason: "Tool output contains warnings".to_string()
            };
        }

        Ok(())
    }

    /// Validate completion evidence
    fn validate_completion_evidence(&self, evidence: &[CompletionEvidence]) -> bool {
        // Check if any evidence has sufficient confidence
        evidence.iter().any(|e| e.confidence > 0.1 && e.supports_success)
    }
} 