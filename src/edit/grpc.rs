use tonic::{Request, Response, Status};
use std::path::PathBuf;

// Import generated proto types correctly
use crate::grpc_generated::editing::{ 
    edit_target::TargetType as ProtoTargetType,
    validation_issue::Severity as ProtoSeverity, 
    EditCodeRequest,  // Correct request type
    EditCodeResponse, // Correct response type
    ValidateEditRequest,
    ValidateEditResponse,
    ValidationIssue,
    EditOptions as ProtoEditOptions, // Import gRPC EditOptions
    editing_service_server::{EditingService},
};

// Import engine types
use crate::edit::engine::{self, EditTarget, EngineValidationSeverity, EngineEditOptions}; // Import EngineEditOptions

// The server implementation
#[derive(Debug, Default)]
pub struct EditingServiceImpl {}

#[tonic::async_trait]
impl EditingService for EditingServiceImpl {
    // --- edit_code implementation ---
    async fn edit_code(
        &self,
        request: Request<EditCodeRequest>,
    ) -> Result<Response<EditCodeResponse>, Status> {
        println!("gRPC Request: EditCode");
        let req = request.into_inner();

        if req.file_path.is_empty() {
            return Err(Status::invalid_argument("File path cannot be empty"));
        }
        
        let target = parse_edit_target(req.target)?;
        let engine_options = map_grpc_options(req.options.as_ref()); // Map options

        match engine::apply_edit(
            &PathBuf::from(&req.file_path),
            &target,
            &req.content, // Use req.content which is the new code
            engine_options.as_ref(), // Pass options to engine
        ) {
            Ok(_) => {
                let response = EditCodeResponse {
                    success: true,
                    error_message: None,
                    affected_elements: vec![], // Empty for now
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                eprintln!("Error applying edit via gRPC: {:?}", e);
                // Return error in the response body, not as gRPC status
                let response = EditCodeResponse {
                    success: false,
                    error_message: Some(format!("Failed to apply edit: {}", e)),
                    affected_elements: vec![],
                };
                 Ok(Response::new(response))
            }
        }
    }

    // --- validate_edit implementation ---
    async fn validate_edit(
        &self,
        request: Request<ValidateEditRequest>,
    ) -> Result<Response<ValidateEditResponse>, Status> {
        println!("gRPC Request: ValidateEdit");
        let req = request.into_inner();

        if req.file_path.is_empty() {
            return Err(Status::invalid_argument("File path cannot be empty"));
        }

        let target = parse_edit_target(req.target)?;
        let engine_options = map_grpc_options(req.options.as_ref()); // Map options

        match engine::validate_edit(
            &PathBuf::from(&req.file_path),
            &target,
            &req.content, // Use req.content which is the new code
            engine_options.as_ref(), // Pass options to engine
        ) {
            Ok(engine_issues) => {
                // Calculate validity BEFORE consuming the vector
                let is_valid = !engine_issues
                    .iter()
                    .any(|issue| issue.severity == EngineValidationSeverity::Error);

                let proto_issues = engine_issues
                    .into_iter()
                    .map(|issue| ValidationIssue {
                        severity: map_severity(issue.severity).into(),
                        message: issue.message,
                        // Convert Option<usize> to Option<u32>, handling potential truncation (though unlikely for line numbers)
                        line_number: issue.line_number.map(|ln| ln as u32), 
                    })
                    .collect();

                let response = ValidateEditResponse {
                    is_valid,
                    issues: proto_issues,
                };
                Ok(Response::new(response))
            }
            Err(e) => {
                 // Handle engine errors (like file read or language detection fails)
                 eprintln!("Error during validation via gRPC: {:?}", e);
                 // Return a gRPC internal error status
                 Err(Status::internal(format!("Validation failed: {}", e)))
            }
        }
    }
}

// --- Helper Functions ---

// Helper to map gRPC EditOptions to engine::EditOptions
fn map_grpc_options(grpc_opts: Option<&ProtoEditOptions>) -> Option<EngineEditOptions> {
    grpc_opts.map(|opts| EngineEditOptions {
        // Map fields directly - add more as options expand
        format_code: opts.format_code, 
        update_references: opts.update_references, 
    })
}

// Helper to map EngineValidationSeverity to gRPC Severity
fn map_severity(engine_severity: EngineValidationSeverity) -> ProtoSeverity {
    match engine_severity {
        EngineValidationSeverity::Error => ProtoSeverity::Error,
        EngineValidationSeverity::Warning => ProtoSeverity::Warning,
        EngineValidationSeverity::Info => ProtoSeverity::Info,
    }
}

// Helper to parse gRPC EditTarget into engine::EditTarget
fn parse_edit_target(target: Option<crate::grpc_generated::editing::EditTarget>) -> Result<EditTarget, Status> {
    match target.and_then(|t| t.target_type) { // Use target_type oneof
        Some(ProtoTargetType::LineRange(range)) => {
            if range.start_line == 0 || range.end_line == 0 {
                 return Err(Status::invalid_argument("Line numbers must be 1-based"));
            }
            if range.start_line > range.end_line {
                 return Err(Status::invalid_argument("Start line cannot be greater than end line"));
            }
            Ok(EditTarget::LineRange { 
                start: range.start_line as usize, 
                end: range.end_line as usize 
            })
        }
        Some(ProtoTargetType::SemanticElement(semantic)) => {
            if semantic.element_query.is_empty() {
                 return Err(Status::invalid_argument("Semantic element query cannot be empty"));
            }
            Ok(EditTarget::Semantic { 
                element_query: semantic.element_query 
            })
        }
        None => Err(Status::invalid_argument("Edit target is required"))
    }
} 