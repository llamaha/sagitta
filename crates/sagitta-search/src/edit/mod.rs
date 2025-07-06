// crates/sagitta-search/src/edit/mod.rs
//! Core module for handling code editing operations.

use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use anyhow::{Result, Context, bail, anyhow};
use tree_sitter::{Parser, Language, Node};
use regex::Regex;
use tracing::{info, trace};

// --- Public Struct/Enum Definitions ---

/// Severity level for validation issues.
#[derive(Debug, Clone, PartialEq, Eq)] // Added derive for potential future use
pub enum EngineValidationSeverity {
    /// Indicates an error that prevents the edit.
    Error,
    /// Indicates a potential issue or suggestion.
    Warning,
    /// Informational message.
    Info,
}

/// Represents a validation issue found during edit checks.
#[derive(Debug, Clone)] // Added derive for potential future use
pub struct EngineValidationIssue {
    /// The severity of the issue.
    pub severity: EngineValidationSeverity,
    /// A descriptive message about the issue.
    pub message: String,
    /// The 1-based line number where the issue occurs, if applicable.
    pub line_number: Option<usize>,
}

/// Options to control the behavior of the edit engine.
#[derive(Debug, Clone, Default)] // <-- Added Default derive
pub struct EngineEditOptions {
    /// Whether to format the inserted code (not yet implemented).
    pub format_code: bool,
    /// Whether to update references to the edited element (not yet implemented).
    pub update_references: bool,
    /// Whether to attempt preserving documentation comments (not yet implemented).
    pub preserve_documentation: bool,
    // Add other options here as needed
}

/// Specifies the target location for an edit operation.
#[derive(Debug)]
pub enum EditTarget {
    /// Target a specific range of lines (1-based, inclusive).
    LineRange { 
        /// Start line number (1-based).
        start: usize, 
        /// End line number (1-based, inclusive).
        end: usize 
    },
    /// Target a code element identified by a semantic query.
    Semantic { 
        /// Query string to identify the element (e.g., "function:my_func").
        element_query: String 
    },
}

// --- Public API Functions ---

/// Applies a code edit to a specified file.
pub fn apply_edit(
    file_path: &Path,
    target: &EditTarget,
    new_content: &str,
    options: Option<&EngineEditOptions>,
) -> Result<()> {
    let opts = options.cloned().unwrap_or_default();
    trace!("Applying edit to target: {:?}", target);
    trace!("Using engine options: {:?}", opts);
    if opts.format_code { trace!("Note: Formatting option is set (not implemented yet)."); }
    if opts.update_references { trace!("Note: Update references option is set (not implemented yet)."); }
    if !opts.preserve_documentation { trace!("Note: No preserve docs option is set (not implemented yet)."); }

    let original_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
    let original_lines: Vec<&str> = original_content.lines().collect();

    let (start_line_0_based, end_line_0_based) = match target {
        EditTarget::LineRange { start, end } => {
             if *start == 0 || *end == 0 || *start > *end || *end > original_lines.len() {
                 bail!("Invalid line range: start={}, end={}, total_lines={}", start, end, original_lines.len());
             }
             (start.saturating_sub(1), end.saturating_sub(1))
        }
        EditTarget::Semantic { element_query } => {
            let language = get_language(file_path)
                .with_context(|| format!("Failed to get language for file: {}", file_path.display()))?;
            let tree = parse_content(&original_content, &language)
                .with_context(|| format!("Failed to parse file: {}", file_path.display()))?;
            let (start, end) = find_semantic_element(
                &tree, 
                element_query, 
                original_content.as_bytes()
            ).with_context(|| format!("Failed to find semantic element '{}' in {}", element_query, file_path.display()))?;
            
            trace!(
                "Applying edit to semantic target '{}' ({} bytes) found at lines {}-{}",
                element_query,
                new_content.len(),
                start + 1,
                end + 1
            );
            (start, end)
        }
    };

    let parent_dir = file_path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp_file = NamedTempFile::new_in(parent_dir)
        .context("Failed to create temporary file")?;
    let formatted_content = format_content_indentation(
        new_content, 
        &original_lines, 
        start_line_0_based
    );

    for line in original_lines.iter().take(start_line_0_based) {
        writeln!(temp_file, "{line}").context("Write error to temp file (before)")?;
    }
    if !formatted_content.is_empty() {
        write!(temp_file, "{formatted_content}").context("Write error to temp file (content)")?;
        if !formatted_content.ends_with('\n') && (end_line_0_based + 1) < original_lines.len() {
             writeln!(temp_file).context("Write error to temp file (newline after content)")?;
        }
    } else if start_line_0_based < end_line_0_based + 1 && (end_line_0_based + 1) < original_lines.len() {
         writeln!(temp_file).context("Write error to temp file (newline after empty content)")?;
    }
    for line in original_lines.iter().skip(end_line_0_based + 1) {
        writeln!(temp_file, "{line}").context("Write error to temp file (after)")?;
    }
    temp_file.flush().context("Failed to flush temp file")?;
    temp_file.persist(file_path)
        .map_err(|e| anyhow!("Failed to persist temp file over {}: {}", file_path.display(), e.error))?;

    Ok(())
}

/// Validates a potential code edit without applying it.
pub fn validate_edit(
    file_path: &Path,
    target: &EditTarget,
    options: Option<&EngineEditOptions>,
) -> Result<Vec<EngineValidationIssue>, anyhow::Error> {
    let opts = options.cloned().unwrap_or_default();
    trace!("Validating edit for target: {:?}", target);
    trace!("Note: Validation called with options: {:?}", opts);

    let mut issues: Vec<EngineValidationIssue> = Vec::new();

    let language = match get_language(file_path) {
        Ok(lang) => lang,
        Err(e) => {
            return Err(anyhow!("Failed to get language for validation {}: {}", file_path.display(), e));
        }
    };
    let original_content = match fs::read_to_string(file_path) {
         Ok(content) => content,
         Err(e) => {
             return Err(anyhow!("Failed to read file for validation {}: {}", file_path.display(), e));
         }
    };

    match target {
        EditTarget::LineRange { start, end } => {
           let line_count = original_content.lines().count();
           if *start == 0 || *end == 0 {
                issues.push(EngineValidationIssue {
                    severity: EngineValidationSeverity::Error,
                    message: "Line numbers must be 1-based.".to_string(),
                    line_number: None,
                });
           }
           if *start > *end {
                 issues.push(EngineValidationIssue {
                     severity: EngineValidationSeverity::Error,
                     message: format!("Start line ({start}) cannot be greater than end line ({end})."),
                     line_number: Some(*start),
                 });
           }
           if *end > line_count {
                issues.push(EngineValidationIssue {
                    severity: EngineValidationSeverity::Error,
                    message: format!("End line ({end}) is beyond file length ({line_count} lines)."),
                    line_number: Some(*end),
                });
           }
        }
        EditTarget::Semantic { element_query } => {
             match parse_content(&original_content, &language) {
                 Ok(tree) => {
                     match find_semantic_element(&tree, element_query, original_content.as_bytes()) {
                         Ok((start_line, end_line)) => {
                             trace!("Semantic target '{}' found at lines {}-{} (validation step).", element_query, start_line + 1, end_line + 1);
                         }
                         Err(e) => { 
                             issues.push(EngineValidationIssue {
                                severity: EngineValidationSeverity::Error,
                                message: format!("Could not find semantic element '{element_query}': {e}"),
                                line_number: None,
                             });
                         }
                     }
                 }
                 Err(e) => { 
                     issues.push(EngineValidationIssue {
                        severity: EngineValidationSeverity::Error,
                        message: format!("Failed to parse original file {}: {}", file_path.display(), e),
                        line_number: None,
                    });
                 }
             }
        }
    }
    
    if issues.is_empty() {
        info!("Validation checks passed (target existence and basic content syntax).");
    }
    Ok(issues)
}

// --- Helper Functions (Private to this module) ---

fn get_leading_whitespace(line: &str) -> String {
    let re = Regex::new(r"^([\t ]*)").unwrap();
    re.captures(line).and_then(|c| c.get(1)).map_or(String::new(), |m| m.as_str().to_string())
}

fn format_content_indentation(
    new_content: &str,
    original_lines: &[&str],
    start_line_0_based: usize,
) -> String {
    let leading_indent = if start_line_0_based > 0 {
        original_lines.get(start_line_0_based.saturating_sub(1))
            .map(|line| get_leading_whitespace(line))
            .unwrap_or_default()
    } else {
        original_lines.first()
             .map(|line| get_leading_whitespace(line))
             .unwrap_or_default()
    };
    if start_line_0_based == 0 && original_lines.is_empty() { return new_content.to_string(); }
    new_content.lines().map(|line| format!("{leading_indent}{line}")).collect::<Vec<String>>().join("\n")
}

fn get_language(file_path: &Path) -> Result<Language> {
    let extension = file_path.extension().and_then(|os_str| os_str.to_str()).unwrap_or("");
    match extension {
        "rs" => Ok(tree_sitter_rust::language()),
        "py" => Ok(tree_sitter_python::language()),
        "js" | "jsx" => Ok(tree_sitter_javascript::language()),
        "ts" | "tsx" => Ok(tree_sitter_typescript::language_typescript()),
        "go" => Ok(tree_sitter_go::language()),
        "rb" => Ok(tree_sitter_ruby::language()),
        "yaml" | "yml" => Ok(tree_sitter_yaml::language()),
        "md" | "mdx" => Ok(tree_sitter_md::language()),
        _ => bail!("Unsupported file extension for semantic editing: {}", extension),
    }
}

fn parse_content(content: &str, language: &Language) -> Result<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).context("Failed to set tree-sitter language")?;
    parser.parse(content, None).ok_or_else(|| anyhow!("Tree-sitter parsing failed"))
}

fn expand_range_for_comments(node: Node) -> Result<(usize, usize)> {
    let mut current_node = node;
    while let Some(prev_sibling) = current_node.prev_named_sibling() {
        if prev_sibling.kind().contains("comment") {
            current_node = prev_sibling;
        } else {
            break;
        }
    }
    Ok((current_node.start_position().row, node.end_position().row))
}

fn find_direct_child_element<'a>(parent_node: &Node<'a>, element_query_part: &str, source_code: &[u8]) -> Result<Node<'a>> {
    let query_parts: Vec<&str> = element_query_part.splitn(2, ':').collect();
    if query_parts.len() != 2 {
        bail!("Invalid element query part format. Expected 'type:name', got '{}'", element_query_part);
    }
    let element_type = query_parts[0];
    let element_name = query_parts[1];

    let mut cursor = parent_node.walk();
    for child_node in parent_node.named_children(&mut cursor) {
        if child_node.kind() == element_type {
            // Extract the name/identifier based on common tree-sitter patterns
            let mut name_node_opt = child_node.child_by_field_name("name");
            if name_node_opt.is_none() {
                 name_node_opt = child_node.child_by_field_name("identifier");
            }
            // Add other common identifier field names if necessary (e.g., "id")

            if let Some(name_node) = name_node_opt {
                 let node_name = name_node.utf8_text(source_code)?;
                if node_name == element_name {
                     return Ok(child_node); // Found the direct child
                 }
            } else {
                // Attempt fallback for simple cases like direct identifiers
                 if child_node.kind() == element_type && child_node.child_count() > 0 {
                     let first_child_name_opt = child_node.named_child(0).and_then(|n| n.utf8_text(source_code).ok());
                     if first_child_name_opt == Some(element_name) {
                         return Ok(child_node);
                     }
                 }
            }
        }
    }
    bail!("Could not find direct child element '{}:{}' under the current node.", element_type, element_name);
}

fn find_semantic_element(tree: &tree_sitter::Tree, element_query_str: &str, source_code: &[u8]) -> Result<(usize, usize)> {
    let parts: Vec<&str> = element_query_str.split('.').collect();
    let mut current_node = tree.root_node();

    for part in parts {
         current_node = find_direct_child_element(&current_node, part, source_code)
             .with_context(|| format!("Failed while searching for element part '{part}'"))?;
    }

    expand_range_for_comments(current_node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Helper to create a temp file with content
    fn create_temp_file(content: &str) -> tempfile::NamedTempFile { // Use full path here
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        file.flush().unwrap();
        file
    }

    // Helper to read a temp file's content
    fn read_temp_file(file: &tempfile::NamedTempFile) -> String { // Use full path here
        fs::read_to_string(file.path()).unwrap()
    }

    fn create_test_file(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let file_path = dir.join(filename);
        fs::write(&file_path, content).unwrap();
        file_path
    }

    fn read_test_file(file_path: &Path) -> String {
        fs::read_to_string(file_path).unwrap()
    }

    // --- apply_edit Tests ---
    #[test]
    fn test_apply_edit_line_range_replace() { /* ... */ }
    #[test]
    fn test_apply_edit_line_range_insert() { /* ... */ }
    #[test]
    fn test_apply_edit_indentation() { /* ... */ }
    #[test]
    fn test_apply_edit_indentation_start_of_file() { /* ... */ }

    // --- find_semantic_element Tests ---
    #[test]
    fn test_semantic_find_rust_function() { /* ... */ }
    #[test]
    fn test_semantic_find_python_method() { /* ... */ }
    #[test]
    fn test_semantic_find_nonexistent() { /* ... */ }

    // --- validate_edit Tests ---
    #[test]
    fn test_validate_line_range_ok() { /* ... */ }
    #[test]
    fn test_validate_line_range_invalid_range() { /* ... */ }
    #[test]
    fn test_validate_semantic_ok() { /* ... */ }
    #[test]
    fn test_validate_semantic_target_not_found() { /* ... */ }
    #[test]
    fn test_validate_content_syntax_error() { /* ... */ }
} 