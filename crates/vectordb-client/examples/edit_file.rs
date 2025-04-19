use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use vectordb_client::{VectorDBClient, ValidationSeverity};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a temporary test file to edit
    let temp_file = create_test_file()?;
    let temp_path = temp_file.display().to_string();
    
    println!("Created test file at: {}", temp_path);
    println!("Test file contents:");
    println!("{}", std::fs::read_to_string(&temp_path)?);
    
    // Connect to the VectorDB server
    let server_address = env::var("VECTORDB_SERVER").unwrap_or_else(|_| "http://localhost:50051".to_string());
    let mut client = VectorDBClient::connect(server_address).await?;
    
    println!("\n--- Running validation by lines example ---");
    // First, validate an edit without applying it
    let validation_issues = client.validate_edit_by_lines(
        temp_path.clone(),  // File path
        4, 4,               // Start and end line (line 4)
        "    // This is a new comment line".to_string(), // New content
        false,              // Don't format
        false,              // Don't update references
    ).await?;
    
    if validation_issues.is_empty() {
        println!("Validation passed successfully!");
    } else {
        println!("Validation found issues:");
        for issue in &validation_issues {
            let severity = match issue.severity {
                ValidationSeverity::Info => "INFO",
                ValidationSeverity::Warning => "WARNING",
                ValidationSeverity::Error => "ERROR",
            };
            println!("{}: {}", severity, issue.message);
        }
        
        // Check if there are any errors (blockers)
        let has_errors = validation_issues.iter()
            .any(|issue| matches!(issue.severity, ValidationSeverity::Error));
            
        if has_errors {
            println!("Validation failed with errors, not proceeding with edit.");
            return Ok(());
        }
    }
    
    println!("\n--- Applying edit by lines ---");
    // Apply the edit
    let edit_result = client.edit_file_by_lines(
        temp_path.clone(),  // File path
        4, 4,               // Start and end line (line 4)
        "    // This is a new comment line".to_string(), // New content
        false,              // Don't format
        false,              // Don't update references
    ).await?;
    
    if edit_result.success {
        println!("Edit applied successfully!");
        println!("\nUpdated file content:");
        println!("{}", std::fs::read_to_string(&temp_path)?);
    } else {
        println!("Edit failed: {}", edit_result.error_message.unwrap_or_default());
    }
    
    println!("\n--- Applying edit by semantic element ---");
    // Now apply an edit using semantic targeting
    let edit_result = client.edit_file_by_element(
        temp_path.clone(),           // File path
        "function:calculate_sum".to_string(), // Element query
        "fn calculate_sum(a: i32, b: i32) -> i32 {\n    // Enhanced function with better comments\n    let result = a + b;\n    println!(\"Sum: {}\", result);\n    result\n}".to_string(), // New content
        true,                  // Format code
        false,                 // Don't update references
    ).await?;
    
    if edit_result.success {
        println!("Edit applied successfully to function!");
        println!("\nUpdated file after element edit:");
        println!("{}", std::fs::read_to_string(&temp_path)?);
    } else {
        println!("Element edit failed: {}", edit_result.error_message.unwrap_or_default());
    }
    
    // Clean up
    // std::fs::remove_file(&temp_path)?;
    println!("\nExample complete! (File preserved for examination: {})", temp_path);
    
    Ok(())
}

// Helper function to create a test file
fn create_test_file() -> Result<std::path::PathBuf, std::io::Error> {
    let path = Path::new("./test_file.rs");
    let mut file = File::create(&path)?;
    
    write!(file, r#"// This is a test file for the edit example

fn calculate_sum(a: i32, b: i32) -> i32 {{
    a + b
}}

fn main() {{
    let x = 5;
    let y = 10;
    let sum = calculate_sum(x, y);
    println!("Sum: {{}}", sum);
}}
"#)?;
    
    Ok(path.to_path_buf())
} 