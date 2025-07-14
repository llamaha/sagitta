# Adding a New Language to Sagitta

This guide provides a comprehensive step-by-step process for adding support for a new programming language to Sagitta. The guide uses the C++ implementation as a reference, showing exactly what files need to be created and modified.

## Overview

Sagitta's language support consists of several integrated components:

1. **Tree-sitter parser** - For syntax tree parsing
2. **Code parser** - Converts tree-sitter output to semantic elements  
3. **Code intelligence** - Extracts signatures, descriptions, and identifiers
4. **Repository mapper** - Line-by-line scanning for quick overview
5. **Type system** - Method/element type definitions and display

## Prerequisites

- Basic understanding of the target language's syntax
- Familiarity with tree-sitter and regular expressions
- Access to the target language's tree-sitter grammar

## Step 1: Add Tree-sitter Grammar

### 1.1 Add dependency to Cargo.toml

**File**: `crates/code-parsers/Cargo.toml`

```toml
[dependencies]
# ... existing dependencies ...
tree-sitter-your-language = "x.x.x"
```

### 1.2 Register the language

**File**: `crates/code-parsers/src/languages.rs`

```rust
pub fn get_language_from_extension(extension: &str) -> String {
    match extension.to_lowercase().as_str() {
        // ... existing mappings ...
        "your_ext" | "your_ext2" => "your_language".to_string(),
        _ => "unknown".to_string(),
    }
}
```

## Step 2: Create Core Parser

### 2.1 Create the main parser file

**File**: `crates/code-parsers/src/your_language.rs`

```rust
use crate::types::{Element, CodeChunk};
use std::collections::HashSet;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree, Node};

extern "C" {
    fn tree_sitter_your_language() -> Language;
}

pub struct YourLanguageParser {
    parser: Parser,
    query: Query,
}

impl YourLanguageParser {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut parser = Parser::new();
        let language = unsafe { tree_sitter_your_language() };
        parser.set_language(language)?;

        // Define your query to capture relevant language constructs
        let query_source = r#"
            (function_definition 
                name: (identifier) @func
            ) @func.whole

            (class_definition 
                name: (identifier) @class
            ) @class.whole

            ; Add more patterns for your language
        "#;

        let query = Query::new(language, query_source)?;

        Ok(YourLanguageParser { parser, query })
    }

    pub fn parse(&mut self, content: &str) -> Result<Vec<Element>, Box<dyn std::error::Error + Send + Sync>> {
        let tree = self.parser.parse(content, None)
            .ok_or("Failed to parse content")?;

        let mut elements = self.extract_elements(&tree, content);

        // Add fallback chunking if needed
        if elements.is_empty() && !content.trim().is_empty() {
            elements = self.create_fallback_chunks(content);
        }

        Ok(elements)
    }

    fn extract_elements(&self, tree: &Tree, content: &str) -> Vec<Element> {
        let mut elements = Vec::new();
        let mut cursor = QueryCursor::new();
        let root_node = tree.root_node();

        for query_match in cursor.matches(&self.query, root_node, content.as_bytes()) {
            for capture in query_match.captures {
                let node = capture.node;
                let capture_name = &self.query.capture_names()[capture.index as usize];

                // Extract element information
                let name = self.extract_element_name(&node, content);
                let element_type = self.map_capture_to_element_type(capture_name);

                // Calculate positions
                let start_point = node.start_position();
                let end_point = node.end_position();

                elements.push(Element {
                    name,
                    content: node.utf8_text(content.as_bytes()).unwrap_or("").to_string(),
                    element_type,
                    lang: "your_language".to_string(),
                    start_line: Some(start_point.row + 1),
                    end_line: Some(end_point.row + 1),
                    start_column: Some(start_point.column),
                    end_column: Some(end_point.column),
                    identifiers: self.extract_identifiers(&node, content),
                    outgoing_calls: self.extract_outgoing_calls(&node, content),
                    signature: None, // Will be filled by code intelligence
                    description: None, // Will be filled by code intelligence
                    parent_name: None, // Will be filled by code intelligence
                });
            }
        }

        elements
    }

    fn extract_element_name(&self, node: &Node, content: &str) -> String {
        // Language-specific name extraction logic
        // This varies greatly between languages
        "element_name".to_string() // Placeholder
    }

    fn map_capture_to_element_type(&self, capture_name: &str) -> String {
        match capture_name {
            "func" => "function",
            "class" => "class",
            // Add more mappings
            _ => "unknown",
        }.to_string()
    }

    fn extract_identifiers(&self, node: &Node, content: &str) -> Vec<String> {
        let mut identifiers = HashSet::new();
        // Extract variable names, function calls, etc.
        identifiers.into_iter().collect()
    }

    fn extract_outgoing_calls(&self, node: &Node, content: &str) -> Vec<String> {
        let mut calls = HashSet::new();
        // Extract function calls within this element
        calls.into_iter().collect()
    }

    fn create_fallback_chunks(&self, content: &str) -> Vec<Element> {
        // Create 200-line chunks as fallback
        const CHUNK_SIZE: usize = 200;
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();

        for (i, chunk_lines) in lines.chunks(CHUNK_SIZE).enumerate() {
            let chunk_content = chunk_lines.join("\n");
            let start_line = i * CHUNK_SIZE + 1;
            let end_line = start_line + chunk_lines.len() - 1;

            chunks.push(Element {
                name: format!("chunk_{}", i + 1),
                content: chunk_content,
                element_type: "chunk".to_string(),
                lang: "your_language".to_string(),
                start_line: Some(start_line),
                end_line: Some(end_line),
                start_column: Some(1),
                end_column: Some(chunk_lines.last().map(|l| l.len()).unwrap_or(1)),
                identifiers: Vec::new(),
                outgoing_calls: Vec::new(),
                signature: None,
                description: None,
                parent_name: None,
            });
        }

        chunks
    }
}

pub fn parse_your_language(content: &str) -> Result<Vec<Element>, Box<dyn std::error::Error + Send + Sync>> {
    let mut parser = YourLanguageParser::new()?;
    parser.parse(content)
}
```

### 2.2 Register the parser

**File**: `crates/code-parsers/src/parser.rs`

```rust
pub fn parse(content: &str, language: &str) -> Result<Vec<Element>, Box<dyn std::error::Error + Send + Sync>> {
    match language {
        // ... existing languages ...
        "your_language" => crate::your_language::parse_your_language(content),
        _ => Ok(vec![]),
    }
}
```

### 2.3 Add module declaration

**File**: `crates/code-parsers/src/lib.rs`

```rust
// Language-specific parsers
pub mod rust;
pub mod python;
pub mod your_language; // Add this line
// ... other modules ...
```

## Step 3: Add Method Types

### 3.1 Define method types

**File**: `crates/repo-mapper/src/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MethodType {
    // ... existing types ...
    
    // Your Language
    YourLanguageFunction,
    YourLanguageMethod,
    YourLanguageClass,
    YourLanguageStruct,
    YourLanguageInterface,
    // Add language-specific constructs
}

impl MethodType {
    pub fn icon(&self) -> &'static str {
        match self {
            // ... existing icons ...
            MethodType::YourLanguageFunction => "⚡",
            MethodType::YourLanguageMethod => "🔧",
            MethodType::YourLanguageClass => "🏛️",
            MethodType::YourLanguageStruct => "📦",
            MethodType::YourLanguageInterface => "🔶",
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            // ... existing names ...
            MethodType::YourLanguageFunction => "Your Language Function",
            MethodType::YourLanguageMethod => "Your Language Method",
            MethodType::YourLanguageClass => "Your Language Class",
            MethodType::YourLanguageStruct => "Your Language Struct",
            MethodType::YourLanguageInterface => "Your Language Interface",
        }
    }
}
```

## Step 4: Add Code Intelligence

### 4.1 Add signature extraction

**File**: `crates/sagitta-mcp/src/code_intelligence.rs`

```rust
fn extract_signature(content: &str, element_type: &str, language: &str) -> Option<String> {
    match language {
        // ... existing languages ...
        "your_language" => extract_your_language_signature(content, element_type),
        _ => None,
    }
}

fn extract_your_language_signature(content: &str, element_type: &str) -> Option<String> {
    match element_type {
        "function" | "method" => {
            // Extract function signatures with regex
            let re = Regex::new(r"(?m)^[\s]*function\s+(\w+)\s*\([^)]*\)").ok()?;
            if let Some(m) = re.find(content) {
                return Some(m.as_str().trim().to_string());
            }
        }
        "class" => {
            let re = Regex::new(r"(?m)^[\s]*class\s+(\w+)").ok()?;
            if let Some(m) = re.find(content) {
                return Some(m.as_str().trim().to_string());
            }
        }
        _ => {}
    }
    None
}
```

### 4.2 Add description extraction

```rust
fn extract_description(content: &str, language: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    
    match language {
        // ... existing languages ...
        "your_language" => {
            // Look for language-specific documentation comments
            for line in &lines {
                let trimmed = line.trim();
                if trimmed.starts_with("///") {
                    let desc = trimmed.trim_start_matches("///").trim();
                    if !desc.is_empty() && desc.len() > 10 {
                        return Some(desc.to_string());
                    }
                }
            }
        }
        _ => {}
    }
    None
}
```

### 4.3 Add identifier extraction

```rust
fn extract_identifiers(content: &str, language: &str) -> Vec<String> {
    let mut identifiers = HashSet::new();
    
    let identifier_patterns = match language {
        // ... existing patterns ...
        "your_language" => vec![
            r"\bfunction\s+(\w+)",
            r"\bclass\s+(\w+)",
            r"\bvar\s+(\w+)",
            // Add language-specific patterns
        ],
        _ => vec![r"\b([a-zA-Z_]\w+)\b"],
    };

    // ... rest of extraction logic
}
```

### 4.4 Add parent name extraction

```rust
fn extract_parent_name(content: &str, language: &str) -> Option<String> {
    match language {
        // ... existing languages ...
        "your_language" => {
            if let Ok(re) = Regex::new(r"class\s+(\w+)") {
                if let Some(cap) = re.captures(content) {
                    return Some(cap[1].to_string());
                }
            }
        }
        _ => {}
    }
    None
}
```

## Step 5: Add Repository Mapper Scanner

### 5.1 Create line-by-line scanner

**File**: `crates/repo-mapper/src/scanners/your_language.rs`

```rust
use crate::types::{MethodType, MethodInfo};
use regex::Regex;

/// Line-by-line scanner for your language
pub fn scan_line(
    line: &str,
    context: &str,
    docstring: Option<String>,
    methods: &mut Vec<MethodInfo>,
    line_number: usize,
    max_calls: usize,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return;
    }
    
    // Define patterns for your language constructs
    let function_pattern = Regex::new(r"^function\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap();
    let class_pattern = Regex::new(r"^class\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    
    // Check for functions
    if let Some(captures) = function_pattern.captures(trimmed) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::YourLanguageFunction,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
        return;
    }
    
    // Check for classes
    if let Some(captures) = class_pattern.captures(trimmed) {
        let params = extract_params(line);
        let method_calls = extract_method_calls(context, max_calls);
        methods.push(MethodInfo {
            name: captures[1].to_string(),
            method_type: MethodType::YourLanguageClass,
            params,
            context: context.to_string(),
            docstring: docstring.clone(),
            calls: method_calls,
            line_number: Some(line_number),
        });
    }
}

fn extract_params(line: &str) -> String {
    if let Some(start) = line.find('(') {
        if let Some(end) = line.rfind(')') {
            if end > start {
                let params = &line[start + 1..end];
                return params.trim().to_string();
            }
        }
    }
    String::new()
}

fn extract_method_calls(context: &str, max_calls: usize) -> Vec<String> {
    let mut calls = Vec::new();
    
    // Simple regex to find function calls
    let call_regex = Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap();
    
    for captures in call_regex.captures_iter(context) {
        if let Some(name) = captures.get(1) {
            let call_name = name.as_str().to_string();
            // Filter out keywords
            if !["if", "while", "for"].contains(&call_name.as_str()) {
                calls.push(call_name);
            }
        }
    }
    
    calls.sort();
    calls.dedup();
    calls.truncate(max_calls);
    calls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_language_line_scanning() {
        let mut methods = Vec::new();
        
        scan_line(
            "function testFunc() {",
            "function testFunc() {\n    return 42;\n}",
            None,
            &mut methods,
            1,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "testFunc");
        assert_eq!(methods[0].method_type, MethodType::YourLanguageFunction);
    }
}
```

### 5.2 Register the scanner module

**File**: `crates/repo-mapper/src/scanners/mod.rs`

```rust
pub mod go;
pub mod javascript;
pub mod markdown;
pub mod python;
pub mod cpp;
pub mod your_language; // Add this line
pub mod ruby;
pub mod rust;
pub mod typescript;
```

### 5.3 Add file extension mapping

**File**: `crates/repo-mapper/src/mapper.rs`

In the `scan_file_for_methods` function, add your extensions:

```rust
match ext {
    // ... existing extensions ...
    "your_ext" | "your_ext2" => {
        scanners::your_language::scan_line(
            line,
            &context,
            current_docstring.clone(),
            &mut methods,
            i + 1,
            self.options.max_calls_per_method,
        );
    }
    // ... rest of the match arms ...
}
```

## Step 6: Add Element Filter Support

**File**: `crates/code-parsers/src/element_filter.rs`

```rust
fn get_core_element_types_for_language(language: &str) -> HashSet<String> {
    let mut core_types = HashSet::new();
    
    match language {
        // ... existing languages ...
        "your_language" => {
            core_types.insert("function".to_string());
            core_types.insert("class".to_string());
            core_types.insert("interface".to_string());
            // Add your language's core types
        }
        _ => {}
    }
    
    core_types
}
```

## Step 7: Integration Testing

### 7.1 Create test content

Create a test file in your repository with typical language constructs:

```your_language
// test_your_language_support.ext

/// This is a test class
class TestClass {
    function testMethod() {
        return "Hello World";
    }
}

/// A utility function
function utilityFunction(param1, param2) {
    testMethod();
    return param1 + param2;
}
```

### 7.2 Test semantic search

```bash
# Sync repository to index new content
cargo run --bin sagitta-mcp

# Test queries through MCP
query="class definition" elementType="class" lang="your_language"
query="function" elementType="function" lang="your_language"
query="TestClass" lang="your_language"
```

## Step 8: Documentation and Examples

### 8.1 Update main documentation

Add your language to:
- **Main README.md supported languages list** in `crates/sagitta-code/README.md`
- Configuration examples
- Query examples

**Important:** Update the supported languages list in the README:

```markdown
## Supported Languages

- Rust
- Python
- JavaScript
- TypeScript
- Go
- Ruby
- C++        # <-- Add your new language here
- Markdown
- YAML
- HTML
```

### 8.2 Create language-specific examples

Document language-specific features and patterns in your implementation.

## Best Practices

### Performance
- Use efficient regex patterns
- Implement fallback chunking for complex files
- Keep identifier extraction focused

### Accuracy
- Test with real-world code samples
- Handle edge cases (nested functions, complex syntax)
- Validate tree-sitter queries carefully

### Maintainability
- Follow existing naming conventions
- Document language-specific decisions
- Add comprehensive tests

## Common Pitfalls

1. **Overly complex tree-sitter queries** - Start simple and iterate
2. **Missing fallback chunking** - Always provide fallback for unparseable content
3. **Inconsistent element types** - Use standardized names when possible
4. **Poor regex performance** - Test with large files
5. **Missing identifier filtering** - Filter out language keywords

## Testing Checklist

- [ ] Tree-sitter parsing works for basic constructs
- [ ] Element extraction captures relevant code structures
- [ ] Signature extraction works for functions/methods
- [ ] Description extraction finds documentation comments
- [ ] Identifier extraction captures relevant names
- [ ] Line-by-line scanner works for quick overview
- [ ] File extension mapping is correct
- [ ] Semantic search returns relevant results
- [ ] Performance is acceptable on large files
- [ ] Edge cases are handled gracefully
- [ ] **Update supported languages list in `crates/sagitta-code/README.md`**

## Conclusion

This guide provides a complete template for adding new language support to Sagitta. The modular architecture ensures that languages can be added incrementally, with each component building on the previous ones.

For questions or contributions, refer to the main project documentation and existing language implementations as examples.