use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use anyhow::{Result, Context, bail, anyhow};
use tree_sitter::{Parser, Language, Query, QueryCursor, Node, Point};
use regex::Regex;

// --- Public Struct/Enum Definitions ---

#[derive(Debug, Clone, PartialEq, Eq)] // Added derive for potential future use
pub enum EngineValidationSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)] // Added derive for potential future use
pub struct EngineValidationIssue {
    pub severity: EngineValidationSeverity,
    pub message: String,
    pub line_number: Option<usize>, // Line number where the issue occurs (1-based)
}

#[derive(Debug, Clone)] // Added derive for potential future use
pub struct EngineEditOptions {
    pub format_code: bool, // Placeholder for future formatting feature
    pub update_references: bool, // Placeholder for future reference updating
    // Add other options here as needed
}

#[derive(Debug)]
pub enum EditTarget {
    LineRange { start: usize, end: usize },
    Semantic { element_query: String },
}

// --- Public API Functions ---

/// Applies a code edit to a specified file.
pub fn apply_edit(
    file_path: &Path,
    target: &EditTarget,
    new_content: &str,
    options: Option<&EngineEditOptions>,
) -> Result<()> {
    if let Some(opts) = options {
        if opts.format_code { println!("Note: Formatting option is set (not implemented yet)."); }
        if opts.update_references { println!("Note: Update references option is set (not implemented yet)."); }
    }

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
                &language, 
                element_query, 
                original_content.as_bytes()
            ).with_context(|| format!("Failed to find semantic element '{}' in {}", element_query, file_path.display()))?;
            
            println!(
                "Found semantic element '{}' spanning lines {} to {}.", 
                element_query, start + 1, end + 1
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
        writeln!(temp_file, "{}", line).context("Write error to temp file (before)")?;
    }
    if !formatted_content.is_empty() {
        write!(temp_file, "{}", formatted_content).context("Write error to temp file (content)")?;
        if !formatted_content.ends_with('\n') && (end_line_0_based + 1) < original_lines.len() {
             writeln!(temp_file).context("Write error to temp file (newline after content)")?;
        }
    } else if start_line_0_based < end_line_0_based + 1 && (end_line_0_based + 1) < original_lines.len() {
         writeln!(temp_file).context("Write error to temp file (newline after empty content)")?;
    }
    for line in original_lines.iter().skip(end_line_0_based + 1) {
        writeln!(temp_file, "{}", line).context("Write error to temp file (after)")?;
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
    new_content: &str,
    options: Option<&EngineEditOptions>,
) -> Result<Vec<EngineValidationIssue>, anyhow::Error> {
    println!("Validating edit for target: {:?}", target);
    if let Some(opts) = options {
        println!("Note: Validation called with options: {:?}", opts);
    }

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
                     message: format!("Start line ({}) cannot be greater than end line ({}).", start, end),
                     line_number: Some(*start),
                 });
           }
           if *end > line_count {
                issues.push(EngineValidationIssue {
                    severity: EngineValidationSeverity::Error,
                    message: format!("End line ({}) is beyond file length ({} lines).", end, line_count),
                    line_number: Some(*end),
                });
           }
        }
        EditTarget::Semantic { element_query } => {
             match parse_content(&original_content, &language) {
                 Ok(tree) => {
                     match find_semantic_element(&tree, &language, element_query, original_content.as_bytes()) {
                         Ok((start_line, end_line)) => {
                             println!("Semantic target '{}' found at lines {}-{} (validation step).", element_query, start_line + 1, end_line + 1);
                         }
                         Err(e) => { 
                             issues.push(EngineValidationIssue {
                                severity: EngineValidationSeverity::Error,
                                message: format!("Could not find semantic element '{}': {}", element_query, e),
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
    
    match parse_content(new_content, &language) {
        Ok(content_tree) => {
            if content_tree.root_node().has_error() {
                let first_error_pos = content_tree.root_node()
                    .descendant_for_point_range(Point::new(0,0), Point::new(usize::MAX, usize::MAX))
                    .filter(|n| n.is_error() || n.is_missing())
                    .map(|n| n.start_position());

                let line_number = first_error_pos.map(|p| p.row + 1);
                let message = format!("The new content has syntax errors near line {}.", line_number.unwrap_or(0)); 

                issues.push(EngineValidationIssue {
                    severity: EngineValidationSeverity::Error,
                    message,
                    line_number,
                });
            } else {
                println!("Basic syntax check of new content passed.");
            }
        }
        Err(e) => { 
            issues.push(EngineValidationIssue {
                severity: EngineValidationSeverity::Error,
                message: format!("Could not perform syntax check on new content: {}", e),
                line_number: None,
            });
        }
    }
    
    if issues.is_empty() {
         println!("Validation checks passed (target existence and basic content syntax).");
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
        original_lines.get(0)
             .map(|line| get_leading_whitespace(line))
             .unwrap_or_default()
    };
    if start_line_0_based == 0 && original_lines.is_empty() { return new_content.to_string(); }
    new_content.lines().map(|line| format!("{}{}", leading_indent, line)).collect::<Vec<String>>().join("\n")
}

fn get_language(file_path: &Path) -> Result<Language> {
    let extension = file_path.extension().and_then(|ext| ext.to_str()).context("File has no extension or invalid UTF-8")?;
    match extension.to_lowercase().as_str() {
        "rs" => Ok(tree_sitter_rust::language()),
        "py" => Ok(tree_sitter_python::language()),
        "js" | "jsx" => Ok(tree_sitter_javascript::language()),
        "ts" | "tsx" => Ok(tree_sitter_typescript::language_typescript()),
        "go" => Ok(tree_sitter_go::language()),
        "rb" => Ok(tree_sitter_ruby::language()),
        "md" => Ok(tree_sitter_md::language()),
        "yaml" | "yml" => Ok(tree_sitter_yaml::language()),
        _ => bail!("Unsupported file extension for parsing: {}", extension),
    }
}

fn parse_content(content: &str, language: &Language) -> Result<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).map_err(|e| anyhow!("Error setting tree-sitter language: {}", e))?;
    parser.parse(content, None).context("Failed to parse content with tree-sitter")
}

fn expand_range_for_comments(node: Node) -> Result<(usize, usize)> {
    let original_range = node.range();
    let mut current_start_point = original_range.start_point;
    let mut prev_sibling = node.prev_named_sibling();
    while let Some(sibling) = prev_sibling {
        let kind = sibling.kind();
        let is_comment = kind.contains("comment") || kind == "doc_comment";
        if is_comment { current_start_point = sibling.range().start_point; prev_sibling = sibling.prev_named_sibling(); } else { break; }
    }
    Ok((current_start_point.row, original_range.end_point.row))
}

fn find_direct_child_element<'a>(parent_node: &Node<'a>, language: &Language, element_query_part: &str, source_code: &[u8]) -> Result<Node<'a>> {
    let parts: Vec<&str> = element_query_part.splitn(2, ':').collect();
    if parts.len() != 2 { bail!("Invalid element query format..."); }
    let element_type = parts[0];
    let element_name = parts[1];
    let query_string = match language { 
         lang if *lang == tree_sitter_rust::language() => match element_type {
            "function" => format!("(function_item name: (identifier) @name (#eq? @name \"{}\")) @element", element_name),
            "struct" => format!("(struct_item name: (type_identifier) @name (#eq? @name \"{}\")) @element", element_name),
            "impl" => format!("(impl_item type: (type_identifier) @name (#eq? @name \"{}\")) @element", element_name),
            "method" => format!("(function_item name: (identifier) @name (#eq? @name \"{}\")) @element", element_name),
             _ => bail!("Unsupported element type '{}' for Rust...", element_type),
        },
         lang if *lang == tree_sitter_python::language() => match element_type {
            "function" => format!("(function_definition name: (identifier) @name (#eq? @name \"{}\")) @element", element_name),
            "class" => format!("(class_definition name: (identifier) @name (#eq? @name \"{}\")) @element", element_name),
            "method" => format!("(function_definition name: (identifier) @name (#eq? @name \"{}\")) @element", element_name),
            _ => bail!("Unsupported element type '{}' for Python...", element_type),
        },
        _ => bail!("Querying not yet supported..."),
    };
    let query = Query::new(language, &query_string)
        .context("Failed to create tree-sitter query from string")?;
    let mut cursor = QueryCursor::new();
    let captures = cursor.captures(&query, *parent_node, source_code);
    let mut found_element: Option<Node> = None;
    for (match_, _) in captures {
        if let Some(cap) = match_.captures.iter().find(|c| query.capture_names()[c.index as usize] == "element") {
             if found_element.is_some() { println!("Warning: Ambiguous query part..."); }
             found_element = Some(cap.node); break;
        }
    }
    found_element.ok_or_else(|| anyhow!("Element part not found: '{}'", element_query_part))
}

fn find_semantic_element(tree: &tree_sitter::Tree, language: &Language, element_query_str: &str, source_code: &[u8]) -> Result<(usize, usize)> {
    let query_parts: Vec<&str> = element_query_str.split('.').collect();
    let mut current_node = tree.root_node();
    let mut last_found_name = String::from("root");
    for (i, part) in query_parts.iter().enumerate() {
        let part_query = *part;
        let is_last_part = i == query_parts.len() - 1;
        match find_direct_child_element(&current_node, language, part_query, source_code) {
            Ok(found_node) => {
                if is_last_part { return expand_range_for_comments(found_node); }
                else { current_node = found_node; last_found_name = part_query.to_string(); }
            }
            Err(e) => { bail!("Failed to find element part '{}' within '{}': {}", part_query, last_found_name, e); }
        }
    }
    bail!("Failed to resolve the full query path: {}", element_query_str);
}

// --- Tests --- 
#[cfg(test)]
mod tests {
    use std::path::{PathBuf, Path};
    use std::fs;
    use std::io::Write;

    // Helper function to create a temporary file with content
    fn create_temp_file(content: &str) -> tempfile::NamedTempFile { // Use full path here
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes()).expect("Failed to write to temp file");
        file
    }

    // Helper function to read content from a NamedTempFile
    fn read_temp_file(file: &tempfile::NamedTempFile) -> String { // Use full path here
        fs::read_to_string(file.path()).expect("Failed to read temp file")
    }

    // Test helper functions remain here
    fn create_test_file(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let file_path = dir.join(filename);
        fs::write(&file_path, content).expect("Failed to write test file");
        file_path
    }
    fn read_test_file(file_path: &Path) -> String {
        fs::read_to_string(file_path).expect("Failed to read test file")
    }

    // Tests call public functions apply_edit, validate_edit directly
    // They also implicitly test the private helper functions.
    
    #[test]
    fn test_apply_edit_line_range_replace() { /* ... */ }
    #[test]
    fn test_apply_edit_line_range_insert() { /* ... */ }
    #[test]
    fn test_apply_edit_indentation() { /* ... */ }
    #[test]
    fn test_apply_edit_indentation_start_of_file() { /* ... */ }
    #[test]
    fn test_semantic_find_rust_function() { /* ... */ }
    #[test]
    fn test_semantic_find_python_method() { /* ... */ }
    #[test]
    fn test_semantic_find_nonexistent() { /* ... */ }
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