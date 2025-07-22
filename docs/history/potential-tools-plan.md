# Potential Tools for Future Sagitta MCP Server Enhancement

*Generated from user feedback and analysis of current tool effectiveness during complex software engineering tasks.*

## 🚀 **High-Priority Tools**

### 1. **AST/Code Structure Analysis**
**Purpose**: Deep structural understanding of code elements

**Capabilities**:
- Parse and analyze Abstract Syntax Trees for supported languages
- Extract type definitions, function signatures, struct fields
- Understand inheritance hierarchies and trait implementations
- Map complex enum variants and their associated data

**Use Cases**:
- Understanding the complete structure of `AppState` or `AppEvent` enums
- Navigating large structs to find specific fields
- Understanding trait relationships and implementations
- Analyzing function parameters and return types

**Example API**:
```
mcp__sagitta-mcp__analyze_code_structure
- file_path: Path to file to analyze
- element_type: "struct" | "enum" | "function" | "trait" | "impl"
- element_name: Optional specific element to focus on
```

### 2. **Find All References/Usages**
**Purpose**: Locate every usage of a specific code element across the codebase

**Capabilities**:
- Find all references to functions, variables, types, traits
- Show context around each usage
- Filter by usage type (definition, call, implementation, etc.)
- Cross-file reference tracking

**Use Cases**:
- Finding all places where `CreateNewConversation` event is handled
- Locating every usage of a specific configuration field
- Understanding impact of changing a function signature
- Refactoring support and impact analysis

**Example API**:
```
mcp__sagitta-mcp__find_all_references
- symbol_name: Name of the symbol to find
- symbol_type: "function" | "variable" | "type" | "trait" | "field"
- include_definitions: Include definition locations
- context_lines: Number of context lines around each usage
```

### 3. **Diff Preview**
**Purpose**: Preview changes before applying them

**Capabilities**:
- Show unified diff of proposed changes
- Multiple file diff preview
- Syntax highlighting in diffs
- Conflict detection and resolution hints

**Use Cases**:
- Preview complex multi-file changes before applying
- Reduce errors in large refactors
- Understand impact of changes across files
- Code review preparation

**Example API**:
```
mcp__sagitta-mcp__preview_changes
- changes: Array of proposed file changes
- format: "unified" | "side-by-side" | "context"
- show_line_numbers: Boolean for line number display
```

## 🎯 **Medium-Priority Tools**

### 4. **Interactive Code Navigation**
**Purpose**: IDE-like navigation capabilities

**Capabilities**:
- Go to definition functionality
- Go to implementation for trait methods
- Find implementations of traits
- Navigate to type definitions from usage
- Follow import chains

**Use Cases**:
- Quick navigation from trait method to implementations
- Understanding where types are defined
- Following complex import dependencies
- Rapid codebase exploration

**Example API**:
```
mcp__sagitta-mcp__navigate_code
- action: "go_to_definition" | "find_implementations" | "go_to_type"
- file_path: Current file location
- line_number: Line number of symbol
- column_number: Column number of symbol
```

### 5. **Pattern-Based Code Generation**
**Purpose**: Generate code following existing patterns in the codebase

**Capabilities**:
- Analyze existing code patterns
- Generate new code following established conventions
- Template-based code generation with pattern matching
- Maintain consistency with existing style

**Use Cases**:
- Generate new event handlers following existing patterns
- Create new configuration options following established structure
- Generate boilerplate code consistent with project style
- Maintain architectural consistency

**Example API**:
```
mcp__sagitta-mcp__generate_code_pattern
- pattern_type: "event_handler" | "config_option" | "test_case"
- reference_examples: Array of existing examples to follow
- parameters: Pattern-specific parameters
- output_location: Where to generate the code
```

### 6. **Dependency/Module Graph**
**Purpose**: Understand architectural relationships

**Capabilities**:
- Generate dependency graphs for modules/crates
- Show circular dependency detection
- Visualize import relationships
- Analyze coupling and cohesion metrics

**Use Cases**:
- Understanding why certain imports are needed
- Architecture decisions and refactoring planning
- Identifying tightly coupled modules
- Dependency injection planning

**Example API**:
```
mcp__sagitta-mcp__analyze_dependencies
- scope: "crate" | "module" | "file"
- include_external: Include external crate dependencies
- format: "graph" | "tree" | "metrics"
- filter_patterns: Optional patterns to filter results
```

## 🔧 **Enhancement to Existing Tools**

### **Enhanced Semantic Search**
**Current**: Already excellent
**Enhancements**:
- Add filters for "recent changes" or "modified in last N commits"
- Include confidence scores for search results
- Add "search within results" capability
- Support for natural language queries ("find error handling patterns")

### **Multi-Edit Improvements**
**Current**: Very effective
**Enhancements**:
- Add conditional edits based on file content patterns
- Support for regex-based replacements
- Add "dry-run" mode for preview
- Better error handling for partial failures

## 📊 **Implementation Priority**

1. **AST/Code Structure Analysis** - Most impactful for understanding complex codebases
2. **Find All References/Usages** - Critical for refactoring and impact analysis
3. **Diff Preview** - High safety value, reduces errors
4. **Interactive Code Navigation** - Quality of life improvement
5. **Pattern-Based Code Generation** - Automation and consistency
6. **Dependency/Module Graph** - Architecture and planning tool

## 🎯 **Success Metrics**

- **Reduced Context Switching**: Less need to manually navigate between files
- **Faster Onboarding**: Quicker understanding of unfamiliar codebases
- **Fewer Errors**: Better preview and analysis before changes
- **Improved Confidence**: Better understanding of change impact
- **Architectural Clarity**: Clear understanding of system structure

---

*This document represents potential enhancements based on real-world usage patterns and identified gaps in current tooling capabilities.*