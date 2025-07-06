use crate::mcp::types::{MultiEditFileParams, MultiEditFileResult, EditOperation, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use similar::TextDiff;

/// Create a unified diff between two strings
fn create_diff(old_content: &str, new_content: &str, file_path: &str) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);
    
    let mut result = String::new();
    result.push_str(&format!("--- {file_path}\n"));
    result.push_str(&format!("+++ {file_path}\n"));
    
    for group in diff.grouped_ops(3) {
        let mut first_old = None;
        let mut last_old = None;
        let mut first_new = None;
        let mut last_new = None;
        
        for op in &group {
            match op {
                similar::DiffOp::Delete { old_index, old_len, .. } => {
                    if first_old.is_none() {
                        first_old = Some(*old_index);
                    }
                    last_old = Some(old_index + old_len);
                }
                similar::DiffOp::Insert { new_index, new_len, .. } => {
                    if first_new.is_none() {
                        first_new = Some(*new_index);
                    }
                    last_new = Some(new_index + new_len);
                }
                similar::DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                    if first_old.is_none() {
                        first_old = Some(*old_index);
                    }
                    last_old = Some(old_index + old_len);
                    if first_new.is_none() {
                        first_new = Some(*new_index);
                    }
                    last_new = Some(new_index + new_len);
                }
                similar::DiffOp::Equal { old_index, new_index, len } => {
                    if first_old.is_none() {
                        first_old = Some(*old_index);
                    }
                    last_old = Some(old_index + len);
                    if first_new.is_none() {
                        first_new = Some(*new_index);
                    }
                    last_new = Some(new_index + len);
                }
            }
        }
        
        if let (Some(old_start), Some(old_end), Some(new_start), Some(new_end)) = 
            (first_old, last_old, first_new, last_new) {
            result.push_str(&format!("@@ -{},{} +{},{} @@\n", 
                old_start + 1, old_end - old_start, 
                new_start + 1, new_end - new_start));
            
            for op in &group {
                for change in diff.iter_changes(op) {
                    let prefix = match change.tag() {
                        similar::ChangeTag::Delete => "-",
                        similar::ChangeTag::Insert => "+",
                        similar::ChangeTag::Equal => " ",
                    };
                    result.push_str(&format!("{prefix}{change}"));
                    if !change.to_string().ends_with('\n') {
                        result.push('\n');
                    }
                }
            }
        }
    }
    
    result
}

/// Apply a single edit operation to content
fn apply_edit(content: &str, edit: &EditOperation) -> Result<String, String> {
    let matches: Vec<_> = content.match_indices(&edit.old_string).collect();
    
    if matches.is_empty() {
        return Err(format!("String '{}' not found", edit.old_string));
    }
    
    if !edit.replace_all && matches.len() > 1 {
        return Err(format!("String '{}' found {} times. Use replace_all=true or make the string more unique", 
                         edit.old_string, matches.len()));
    }
    
    let new_content = if edit.replace_all {
        content.replace(&edit.old_string, &edit.new_string)
    } else {
        let (start, _) = matches[0];
        let end = start + edit.old_string.len();
        format!("{}{}{}", &content[..start], &edit.new_string, &content[end..])
    };
    
    Ok(new_content)
}

/// Handler for multi-edit file operations
pub async fn handle_multi_edit_file<C: QdrantClientTrait + Send + Sync + 'static>(
    params: MultiEditFileParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<MultiEditFileResult, ErrorObject> {
    // Read the file
    let original_content = match fs::read_to_string(&params.file_path).await {
        Ok(content) => content,
        Err(e) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to read file: {e}"),
                data: None,
            });
        }
    };
    
    // Apply edits sequentially
    let mut current_content = original_content.clone();
    let mut edits_applied = 0;
    let mut errors = Vec::new();
    
    for (i, edit) in params.edits.iter().enumerate() {
        match apply_edit(&current_content, edit) {
            Ok(new_content) => {
                current_content = new_content;
                edits_applied += 1;
            }
            Err(e) => {
                errors.push(format!("Edit {} failed: {}", i + 1, e));
                // Stop on first error - all edits must succeed
                break;
            }
        }
    }
    
    // If any edit failed, return error
    if !errors.is_empty() {
        return Err(ErrorObject {
            code: -32603,
            message: format!("Failed to apply edits: {}", errors.join("; ")),
            data: None,
        });
    }
    
    // Write the new content
    if let Err(e) = fs::write(&params.file_path, &current_content).await {
        return Err(ErrorObject {
            code: -32603,
            message: format!("Failed to write file: {e}"),
            data: None,
        });
    }
    
    // Create diff
    let diff = create_diff(&original_content, &current_content, &params.file_path);
    
    // Count total changes
    let total_replacements: usize = params.edits.iter().map(|edit| {
        if edit.replace_all {
            original_content.matches(&edit.old_string).count()
        } else {
            1
        }
    }).sum();
    
    let changes_summary = format!("Applied {edits_applied} edits with {total_replacements} total replacements");
    
    Ok(MultiEditFileResult {
        file_path: params.file_path,
        original_content: if original_content.len() > 1000 {
            format!("{}... (truncated)", &original_content[..1000])
        } else {
            original_content
        },
        final_content: if current_content.len() > 1000 {
            format!("{}... (truncated)", &current_content[..1000])
        } else {
            current_content
        },
        diff,
        edits_applied,
        changes_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn create_mock_qdrant() -> Arc<qdrant_client::Qdrant> {
        Arc::new(qdrant_client::Qdrant::from_url("http://localhost:6334").build().unwrap())
    }
    
    #[tokio::test]
    async fn test_multi_edit_file_sequential() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "foo bar\nbaz qux\nfoo baz";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = MultiEditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            edits: vec![
                EditOperation {
                    old_string: "foo".to_string(),
                    new_string: "FOO".to_string(),
                    replace_all: true,
                },
                EditOperation {
                    old_string: "baz".to_string(),
                    new_string: "BAZ".to_string(),
                    replace_all: true,
                },
            ],
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_multi_edit_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.edits_applied, 2);
        assert!(result.changes_summary.contains("2 edits"));
        
        // Verify file was actually changed
        let new_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(new_content, "FOO bar\nBAZ qux\nFOO BAZ");
    }
    
    #[tokio::test]
    async fn test_multi_edit_file_with_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "let x = 5;\nlet y = x + 10;\nprint(y);";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = MultiEditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            edits: vec![
                EditOperation {
                    old_string: "x".to_string(),
                    new_string: "value".to_string(),
                    replace_all: true,
                },
                EditOperation {
                    old_string: "y".to_string(),
                    new_string: "result".to_string(),
                    replace_all: true,
                },
            ],
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_multi_edit_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.edits_applied, 2);
        
        // Verify file was actually changed correctly
        let new_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(new_content, "let value = 5;\nlet result = value + 10;\nprint(result);");
    }
    
    #[tokio::test]
    async fn test_multi_edit_file_failure_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = MultiEditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            edits: vec![
                EditOperation {
                    old_string: "Hello".to_string(),
                    new_string: "Hi".to_string(),
                    replace_all: false,
                },
                EditOperation {
                    old_string: "not found".to_string(),
                    new_string: "replacement".to_string(),
                    replace_all: false,
                },
            ],
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_multi_edit_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("Edit 2 failed"));
        
        // Verify file was NOT changed (rollback)
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, original_content);
    }
    
    #[tokio::test]
    async fn test_multi_edit_file_empty_edits() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = MultiEditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            edits: vec![],
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_multi_edit_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.edits_applied, 0);
        assert_eq!(result.original_content, result.final_content);
    }
}