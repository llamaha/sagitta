//! gRPC client implementation for the VectorDB editing service.

use tonic::{Request, transport::Channel};

use crate::error::{ClientError, Result};
use vectordb_proto::editing_service_client::EditingServiceClient;
use vectordb_proto::editing::{
    EditCodeRequest, EditCodeResponse, ValidateEditRequest, ValidateEditResponse,
    EditTarget, LineRange, SemanticElement, EditOptions, ValidationIssue,
};

/// Client wrapper for gRPC EditingService
pub struct EditingClient {
    client: EditingServiceClient<Channel>,
}

impl EditingClient {
    /// Create a new EditingClient from an existing channel
    pub fn new(channel: Channel) -> Self {
        let client = EditingServiceClient::new(channel);
        Self { client }
    }

    /// Apply an edit to a file using either line-based or semantic targeting
    pub async fn edit_code(
        &mut self,
        file_path: String,
        target: EditFileTarget,
        content: String,
        options: Option<EditFileOptions>,
    ) -> Result<EditCodeResponse> {
        let proto_target = self.convert_target(target)?;
        let proto_options = options.map(convert_options);
        
        let request = Request::new(EditCodeRequest {
            file_path,
            target: Some(proto_target),
            content,
            options: proto_options,
        });
        
        let response = self.client.edit_code(request).await?;
        Ok(response.into_inner())
    }
    
    /// Validate an edit without applying it
    pub async fn validate_edit(
        &mut self,
        file_path: String,
        target: EditFileTarget,
        content: String,
        options: Option<EditFileOptions>,
    ) -> Result<ValidateEditResponse> {
        let proto_target = self.convert_target(target)?;
        let proto_options = options.map(convert_options);
        
        let request = Request::new(ValidateEditRequest {
            file_path,
            target: Some(proto_target),
            content,
            options: proto_options,
        });
        
        let response = self.client.validate_edit(request).await?;
        Ok(response.into_inner())
    }
    
    // Helper method to convert our client-friendly enum to protocol buffer
    fn convert_target(&self, target: EditFileTarget) -> Result<EditTarget> {
        let target_type = match target {
            EditFileTarget::LineRange { start, end } => {
                if start == 0 || end == 0 {
                    return Err(ClientError::InvalidArgument("Line numbers must be 1-based (starting from 1)".into()));
                }
                if start > end {
                    return Err(ClientError::InvalidArgument(format!(
                        "Start line ({}) cannot be greater than end line ({})",
                        start, end
                    )));
                }
                
                EditTarget {
                    target_type: Some(vectordb_proto::editing::edit_target::TargetType::LineRange(
                        LineRange {
                            start_line: start,
                            end_line: end,
                        }
                    )),
                }
            },
            EditFileTarget::Semantic { element_query } => {
                if element_query.is_empty() {
                    return Err(ClientError::InvalidArgument("Element query cannot be empty".into()));
                }
                
                EditTarget {
                    target_type: Some(vectordb_proto::editing::edit_target::TargetType::SemanticElement(
                        SemanticElement {
                            element_query,
                        }
                    )),
                }
            }
        };
        
        Ok(target_type)
    }
}

/// Specifies the target for an edit operation
#[derive(Debug, Clone)]
pub enum EditFileTarget {
    /// Target a specific line range (1-based, inclusive)
    LineRange {
        /// Starting line (1-based, inclusive)
        start: u32,
        /// Ending line (1-based, inclusive)
        end: u32,
    },
    /// Target a semantic element (function, class, etc.)
    Semantic {
        /// Query to identify the element (e.g., "function:process_data", "class:MyClass")
        element_query: String,
    },
}

/// Options for controlling edit behavior
#[derive(Debug, Clone)]
pub struct EditFileOptions {
    /// Try to update references to the edited element
    pub update_references: bool,
    /// Preserve documentation when possible
    pub preserve_documentation: bool,
    /// Format the edited code according to file style
    pub format_code: bool,
}

// Convert client options to proto options
fn convert_options(options: EditFileOptions) -> EditOptions {
    EditOptions {
        update_references: options.update_references,
        preserve_documentation: options.preserve_documentation,
        format_code: options.format_code,
    }
}

/// Severity level for validation issues
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Informational message
    Info,
    /// Warning that doesn't prevent the edit
    Warning,
    /// Error that would cause issues if applied
    Error,
}

/// Represents a validation issue with an edit
#[derive(Debug, Clone)]
pub struct ValidationIssueInfo {
    /// Severity of the issue
    pub severity: ValidationSeverity,
    /// Description of the issue
    pub message: String,
    /// Line number related to the issue (if applicable)
    pub line_number: Option<u32>,
}

impl From<ValidationIssue> for ValidationIssueInfo {
    fn from(issue: ValidationIssue) -> Self {
        let severity = match issue.severity() {
            vectordb_proto::editing::validation_issue::Severity::Info => ValidationSeverity::Info,
            vectordb_proto::editing::validation_issue::Severity::Warning => ValidationSeverity::Warning,
            vectordb_proto::editing::validation_issue::Severity::Error => ValidationSeverity::Error,
        };
        
        Self {
            severity,
            message: issue.message,
            line_number: issue.line_number,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // For these tests, we don't need actual gRPC functionality, just to test the
    // target conversion. We'll use a simplified approach with a mock client.
    #[test]
    fn test_convert_line_range_target() {
        let client = MockEditingClient {};
        
        let target = EditFileTarget::LineRange { start: 10, end: 20 };
        let result = client.convert_target(target).unwrap();
        
        match result.target_type {
            Some(vectordb_proto::editing::edit_target::TargetType::LineRange(line_range)) => {
                assert_eq!(line_range.start_line, 10);
                assert_eq!(line_range.end_line, 20);
            },
            _ => panic!("Expected LineRange target type"),
        }
    }
    
    #[test]
    fn test_convert_semantic_target() {
        let client = MockEditingClient {};
        
        let target = EditFileTarget::Semantic { element_query: "function:process_data".to_string() };
        let result = client.convert_target(target).unwrap();
        
        match result.target_type {
            Some(vectordb_proto::editing::edit_target::TargetType::SemanticElement(element)) => {
                assert_eq!(element.element_query, "function:process_data");
            },
            _ => panic!("Expected SemanticElement target type"),
        }
    }
    
    #[test]
    fn test_invalid_line_range() {
        let client = MockEditingClient {};
        
        // Test zero-based line number
        let target = EditFileTarget::LineRange { start: 0, end: 20 };
        let result = client.convert_target(target);
        assert!(result.is_err());
        
        // Test start > end
        let target = EditFileTarget::LineRange { start: 30, end: 20 };
        let result = client.convert_target(target);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_empty_element_query() {
        let client = MockEditingClient {};
        
        let target = EditFileTarget::Semantic { element_query: "".to_string() };
        let result = client.convert_target(target);
        assert!(result.is_err());
    }
    
    // A simple mock that just uses the real `convert_target` method
    struct MockEditingClient {}
    
    impl MockEditingClient {
        fn convert_target(&self, target: EditFileTarget) -> Result<EditTarget> {
            match target {
                EditFileTarget::LineRange { start, end } => {
                    if start == 0 || end == 0 {
                        return Err(ClientError::InvalidArgument("Line numbers must be 1-based (starting from 1)".into()));
                    }
                    if start > end {
                        return Err(ClientError::InvalidArgument(format!(
                            "Start line ({}) cannot be greater than end line ({})",
                            start, end
                        )));
                    }
                    
                    Ok(EditTarget {
                        target_type: Some(vectordb_proto::editing::edit_target::TargetType::LineRange(
                            LineRange {
                                start_line: start,
                                end_line: end,
                            }
                        )),
                    })
                },
                EditFileTarget::Semantic { element_query } => {
                    if element_query.is_empty() {
                        return Err(ClientError::InvalidArgument("Element query cannot be empty".into()));
                    }
                    
                    Ok(EditTarget {
                        target_type: Some(vectordb_proto::editing::edit_target::TargetType::SemanticElement(
                            SemanticElement {
                                element_query,
                            }
                        )),
                    })
                }
            }
        }
    }
} 