use crate::mcp::types::{EditFileParams, EditFileResult, ErrorObject};
use crate::middleware::auth_middleware::AuthenticatedUser;
use sagitta_search::config::AppConfig;
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
use axum::Extension;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::fs;
use similar::{TextDiff, ChangeTag};

/// Get context around the edit location (10 lines before and after)
fn get_context_lines(content: &str, match_start: usize, match_end: usize) -> (String, String) {
    let lines: Vec<&str> = content.lines().collect();
    let mut start_line = 0;
    let mut end_line = lines.len();
    let mut current_pos = 0;
    
    // Find which lines contain the match
    for (i, line) in lines.iter().enumerate() {
        let line_end = current_pos + line.len() + 1; // +1 for newline
        if current_pos <= match_start && match_start < line_end {
            start_line = i.saturating_sub(10); // 10 lines before
        }
        if current_pos <= match_end && match_end < line_end {
            end_line = (i + 11).min(lines.len()); // 10 lines after
            break;
        }
        current_pos = line_end;
    }
    
    let context_lines = &lines[start_line..end_line];
    let old_context = context_lines.join("\n");
    
    // Calculate new context by applying the change
    let before = &content[..match_start];
    let after = &content[match_end..];
    let new_content = format!("{}{}{}", before, "", after); // We'll replace this in the actual edit
    
    (old_context, new_content)
}

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
                        ChangeTag::Delete => "-",
                        ChangeTag::Insert => "+",
                        ChangeTag::Equal => " ",
                    };
                    result.push_str(&format!("{}{}", prefix, change));
                    if !change.to_string().ends_with('\n') {
                        result.push('\n');
                    }
                }
            }
        }
    }
    
    result
}

/// Handler for editing a file
pub async fn handle_edit_file<C: QdrantClientTrait + Send + Sync + 'static>(
    params: EditFileParams,
    _config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<EditFileResult, ErrorObject> {
    // Read the file
    let content = match fs::read_to_string(&params.file_path).await {
        Ok(content) => content,
        Err(e) => {
            return Err(ErrorObject {
                code: -32603,
                message: format!("Failed to read file: {e}"),
                data: None,
            });
        }
    };
    
    // Find the old_string in the content
    let matches: Vec<_> = content.match_indices(&params.old_string).collect();
    
    if matches.is_empty() {
        return Err(ErrorObject {
            code: -32603,
            message: format!("String '{}' not found in file", params.old_string),
            data: None,
        });
    }
    
    if !params.replace_all && matches.len() > 1 {
        return Err(ErrorObject {
            code: -32603,
            message: format!("String '{}' found {} times. Use replace_all=true or make the string more unique", 
                           params.old_string, matches.len()),
            data: None,
        });
    }
    
    // Perform the replacement
    let new_content = if params.replace_all {
        content.replace(&params.old_string, &params.new_string)
    } else {
        let (start, _) = matches[0];
        let end = start + params.old_string.len();
        format!("{}{}{}", &content[..start], &params.new_string, &content[end..])
    };
    
    // Write the new content
    if let Err(e) = fs::write(&params.file_path, &new_content).await {
        return Err(ErrorObject {
            code: -32603,
            message: format!("Failed to write file: {e}"),
            data: None,
        });
    }
    
    // Get context for display (show limited context around changes)
    let (old_context, new_context) = if matches.len() == 1 && !params.replace_all {
        let (start, _) = matches[0];
        let lines: Vec<&str> = content.lines().collect();
        let mut line_start = 0;
        let mut line_num = 0;
        
        for (i, line) in lines.iter().enumerate() {
            if line_start + line.len() >= start {
                line_num = i;
                break;
            }
            line_start += line.len() + 1;
        }
        
        let context_start = line_num.saturating_sub(3);
        let context_end = (line_num + 4).min(lines.len());
        
        let old_lines = lines[context_start..context_end].join("\n");
        let new_lines: Vec<&str> = new_content.lines().collect();
        let new_context_lines = if new_lines.len() > context_start {
            new_lines[context_start..context_end.min(new_lines.len())].join("\n")
        } else {
            String::new()
        };
        
        (old_lines, new_context_lines)
    } else {
        // For multiple replacements, show full diff
        (content.clone(), new_content.clone())
    };
    
    // Create diff
    let diff = create_diff(&content, &new_content, &params.file_path);
    
    // Create summary
    let changes_summary = if params.replace_all {
        format!("Replaced {} occurrences of the text", matches.len())
    } else {
        "Replaced 1 occurrence of the text".to_string()
    };
    
    Ok(EditFileResult {
        file_path: params.file_path,
        old_content: old_context,
        new_content: new_context,
        diff,
        changes_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::path::PathBuf;
    
    fn create_mock_qdrant() -> Arc<qdrant_client::Qdrant> {
        Arc::new(qdrant_client::Qdrant::from_url("http://localhost:6334").build().unwrap())
    }
    
    #[tokio::test]
    async fn test_edit_file_single_occurrence() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world\nThis is a test\nGoodbye world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello world".to_string(),
            new_string: "Hi universe".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.changes_summary, "Replaced 1 occurrence of the text");
        assert!(result.diff.contains("-Hello world"));
        assert!(result.diff.contains("+Hi universe"));
        
        // Verify file was actually changed
        let new_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(new_content, "Hi universe\nThis is a test\nGoodbye world");
    }
    
    #[tokio::test]
    async fn test_edit_file_multiple_occurrences_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file with multiple occurrences
        let original_content = "Hello world\nHello world again\nGoodbye world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "Hello world".to_string(),
            new_string: "Hi universe".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("found 2 times"));
    }
    
    #[tokio::test]
    async fn test_edit_file_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file with multiple occurrences
        let original_content = "foo bar\nfoo baz\nqux foo";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "foo".to_string(),
            new_string: "FOO".to_string(),
            replace_all: true,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await.unwrap();
        
        assert_eq!(result.changes_summary, "Replaced 3 occurrences of the text");
        
        // Verify file was actually changed
        let new_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(new_content, "FOO bar\nFOO baz\nqux FOO");
    }
    
    #[tokio::test]
    async fn test_edit_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let original_content = "Hello world";
        fs::write(&file_path, original_content).await.unwrap();
        
        let params = EditFileParams {
            file_path: file_path.to_str().unwrap().to_string(),
            old_string: "not found".to_string(),
            new_string: "replacement".to_string(),
            replace_all: false,
        };
        
        let config = Arc::new(RwLock::new(AppConfig::default()));
        let qdrant_client = create_mock_qdrant();
        
        let result = handle_edit_file(params, config, qdrant_client, None).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.message.contains("not found in file"));
    }
}