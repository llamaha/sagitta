# repo-mapper

A Rust crate for generating comprehensive repository structure maps, designed specifically for AI-assisted development workflows.

## Overview

`repo-mapper` provides functionality to scan repositories and generate structured maps of code elements like functions, classes, structs, traits, and more. It's designed to be used by both the MCP server and sagitta-code for consistent repository mapping.

## Features

- **Multi-language support**: Rust, JavaScript, TypeScript, Python, Go, Ruby, Vue.js, YAML, and Markdown
- **Parallel processing**: Fast scanning using Rayon for concurrent file processing
- **Content filtering**: Filter files by content patterns before scanning
- **Flexible output**: Multiple verbosity levels (0=minimal, 1=normal, 2=detailed)
- **Rich metadata**: Extract function signatures, documentation, method calls, and context
- **No "unnamed" objects**: Reliable name extraction using targeted regex patterns

## Supported Languages

| Language   | Elements Detected |
|------------|-------------------|
| Rust       | Functions, implementations, traits, trait methods |
| JavaScript | Functions, arrow functions, classes, object methods |
| TypeScript | Functions, arrow functions, classes, methods, interfaces, types |
| Python     | Functions, async functions, methods, static methods, class methods, classes |
| Ruby       | Instance methods, class methods, modules |
| Go         | Functions, methods, interfaces, interface methods |
| Vue.js     | Methods, computed properties, components, props |
| YAML       | Definitions, values, templates |
| Markdown   | Headers (all levels) |

## Usage

### Basic Usage

```rust
use repo_mapper::{generate_repo_map, RepoMapOptions};
use std::path::Path;

// Generate a map with default options
let options = RepoMapOptions::default();
let result = generate_repo_map(Path::new("."), options)?;
println!("{}", result.map_content);
```

### Advanced Usage

```rust
use repo_mapper::{RepoMapOptions, RepoMapper};
use std::path::Path;

// Create custom options
let options = RepoMapOptions {
    verbosity: 2,                                    // Detailed output
    file_extension: Some("rs".to_string()),          // Only Rust files
    content_pattern: Some("async".to_string()),      // Only files containing "async"
    paths: Some(vec!["src/".to_string()]),          // Only scan src directory
    max_calls_per_method: 5,                        // Limit method calls shown
    include_context: true,                          // Include surrounding code
    include_docstrings: true,                       // Extract documentation
};

// Generate the map
let mut mapper = RepoMapper::new(options);
let result = mapper.scan_repository(Path::new("."))?;

// Access structured data
for (file, methods) in &result.methods_by_file {
    println!("File: {}", file);
    for method in methods {
        println!("  {} {}", method.method_type.icon(), method.name);
        if let Some(doc) = &method.docstring {
            println!("    Doc: {}", doc);
        }
    }
}

// Print summary
println!("Scanned {} files, found {} methods", 
         result.summary.files_scanned, 
         result.summary.total_methods);
```

## Configuration Options

- `verbosity`: Output detail level (0=minimal, 1=normal, 2=detailed)
- `file_extension`: Filter by specific file extension (e.g., "rs", "js", "py")
- `content_pattern`: Only scan files containing this pattern
- `paths`: Specific paths within the repository to scan
- `max_calls_per_method`: Maximum number of method calls to extract per method
- `include_context`: Whether to include surrounding code context
- `include_docstrings`: Whether to extract and include documentation

## Output Format

The generated map uses intuitive icons for different code elements:

- ⚙️ Rust functions
- 🔨 Rust implementations  
- ⭐ Rust traits
- 🛠️ Rust trait methods
- 𝒇 JavaScript/TypeScript functions
- → Arrow functions
- 🔷 Classes
- 🔧 Methods
- 🔶 Interfaces
- 🐍 Python functions
- 🔄 Python async functions
- 📌 Python static methods
- 🏷️ Python class methods
- 🏛️ Python classes
- ↳ Ruby instance methods
- ⚡ Ruby class methods
- 🔸 Go functions
- 📎 Go methods
- 🎯 Vue components
- 💫 Vue computed properties
- 📄 YAML definitions
- 📑 Markdown headers

## Integration

This crate is designed to be used by:

- **sagitta-mcp**: MCP server for repository mapping functionality
- **sagitta-code**: AI agent for code analysis and repository understanding
- **sagitta-cli**: Command-line tools for repository analysis

## Performance

- **Parallel processing**: Uses Rayon for concurrent file scanning
- **Smart filtering**: Content pattern matching before full parsing
- **Efficient regex**: Targeted patterns for reliable name extraction
- **Memory efficient**: Streaming file processing without loading entire repository

## Comparison with Tree-sitter Approach

Unlike complex tree-sitter based parsing, `repo-mapper` uses:

- **Simple regex patterns**: Easier to maintain and extend
- **Language-specific scanners**: Tailored patterns for each language
- **Reliable name extraction**: No "unnamed" objects
- **Fast performance**: Lightweight parsing without AST overhead
- **Easy debugging**: Clear, readable pattern matching logic

## Error Handling

The crate provides comprehensive error handling for:

- Missing repository paths
- IO errors during file reading
- Regex compilation errors
- Pattern search failures
- Invalid file extensions

## License

MIT License 