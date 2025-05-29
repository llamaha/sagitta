use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Information about a discovered method/function/element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodInfo {
    /// The name of the method/function
    pub name: String,
    /// The type of method (function, class, etc.)
    pub method_type: MethodType,
    /// Parameter signature
    pub params: String,
    /// Surrounding context (a few lines around the method)
    pub context: String,
    /// Documentation string if available
    pub docstring: Option<String>,
    /// List of method calls found within this method
    pub calls: Vec<String>,
    /// Line number where this element starts
    pub line_number: Option<usize>,
}

/// Types of code elements that can be discovered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MethodType {
    // Ruby
    RubyInstance,
    RubyClass,
    RubyModule,
    
    // JavaScript
    JsFunction,
    JsArrow,
    JsClass,
    JsObject,
    
    // TypeScript
    TsFunction,
    TsArrow,
    TsClass,
    TsMethod,
    TsInterface,
    TsType,
    
    // Vue.js
    VueMethod,
    VueComputed,
    VueComponent,
    VueProp,
    
    // Go
    GoFunc,
    GoMethod,
    GoInterface,
    GoInterfaceMethod,
    
    // Rust
    RustFn,
    RustImpl,
    RustTrait,
    RustTraitMethod,
    
    // Python
    PythonFunction,
    PythonAsyncFunction,
    PythonMethod,
    PythonStaticMethod,
    PythonClassMethod,
    PythonClass,
    
    // YAML
    YamlDef,
    YamlValue,
    YamlTemplate,
    
    // Markdown
    MarkdownHeader,
}

impl MethodType {
    /// Get the icon representation for this method type
    pub fn icon(&self) -> &'static str {
        match self {
            MethodType::RubyInstance => "↳",
            MethodType::RubyClass => "⚡",
            MethodType::RubyModule => "⭐",
            MethodType::JsFunction => "𝒇",
            MethodType::JsArrow => "→",
            MethodType::JsClass => "🔷",
            MethodType::JsObject => "🔹",
            MethodType::TsFunction => "𝒇",
            MethodType::TsArrow => "→",
            MethodType::TsClass => "🔷",
            MethodType::TsMethod => "🔧",
            MethodType::TsInterface => "🔶",
            MethodType::TsType => "📝",
            MethodType::VueMethod => "🔧",
            MethodType::VueComputed => "💫",
            MethodType::VueComponent => "🎯",
            MethodType::VueProp => "🎲",
            MethodType::GoFunc => "🔸",
            MethodType::GoMethod => "📎",
            MethodType::GoInterface => "🔶",
            MethodType::GoInterfaceMethod => "🔗",
            MethodType::RustFn => "⚙️",
            MethodType::RustImpl => "🔨",
            MethodType::RustTrait => "⭐",
            MethodType::RustTraitMethod => "🛠️",
            MethodType::PythonFunction => "🐍",
            MethodType::PythonAsyncFunction => "🔄",
            MethodType::PythonMethod => "🔧",
            MethodType::PythonStaticMethod => "📌",
            MethodType::PythonClassMethod => "🏷️",
            MethodType::PythonClass => "🏛️",
            MethodType::YamlDef => "📄",
            MethodType::YamlValue => "🔖",
            MethodType::YamlTemplate => "📋",
            MethodType::MarkdownHeader => "📑",
        }
    }
    
    /// Get a human-readable name for this method type
    pub fn display_name(&self) -> &'static str {
        match self {
            MethodType::RubyInstance => "Ruby Instance Method",
            MethodType::RubyClass => "Ruby Class Method",
            MethodType::RubyModule => "Ruby Module",
            MethodType::JsFunction => "JavaScript Function",
            MethodType::JsArrow => "JavaScript Arrow Function",
            MethodType::JsClass => "JavaScript Class",
            MethodType::JsObject => "JavaScript Object Method",
            MethodType::TsFunction => "TypeScript Function",
            MethodType::TsArrow => "TypeScript Arrow Function",
            MethodType::TsClass => "TypeScript Class",
            MethodType::TsMethod => "TypeScript Method",
            MethodType::TsInterface => "TypeScript Interface",
            MethodType::TsType => "TypeScript Type",
            MethodType::VueMethod => "Vue Method",
            MethodType::VueComputed => "Vue Computed Property",
            MethodType::VueComponent => "Vue Component",
            MethodType::VueProp => "Vue Prop",
            MethodType::GoFunc => "Go Function",
            MethodType::GoMethod => "Go Method",
            MethodType::GoInterface => "Go Interface",
            MethodType::GoInterfaceMethod => "Go Interface Method",
            MethodType::RustFn => "Rust Function",
            MethodType::RustImpl => "Rust Implementation",
            MethodType::RustTrait => "Rust Trait",
            MethodType::RustTraitMethod => "Rust Trait Method",
            MethodType::PythonFunction => "Python Function",
            MethodType::PythonAsyncFunction => "Python Async Function",
            MethodType::PythonMethod => "Python Method",
            MethodType::PythonStaticMethod => "Python Static Method",
            MethodType::PythonClassMethod => "Python Class Method",
            MethodType::PythonClass => "Python Class",
            MethodType::YamlDef => "YAML Definition",
            MethodType::YamlValue => "YAML Value",
            MethodType::YamlTemplate => "YAML Template",
            MethodType::MarkdownHeader => "Markdown Header",
        }
    }
}

/// Configuration options for repository mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMapOptions {
    /// Verbosity level (0=minimal, 1=normal, 2=detailed)
    pub verbosity: u8,
    /// Optional file extension filter (e.g., "rs", "js", "py")
    pub file_extension: Option<String>,
    /// Optional content pattern to filter files by
    pub content_pattern: Option<String>,
    /// Specific paths within the repository to scan
    pub paths: Option<Vec<String>>,
    /// Maximum number of method calls to include per method
    pub max_calls_per_method: usize,
    /// Whether to include context lines around methods
    pub include_context: bool,
    /// Whether to extract and include docstrings
    pub include_docstrings: bool,
}

impl Default for RepoMapOptions {
    fn default() -> Self {
        Self {
            verbosity: 1,
            file_extension: None,
            content_pattern: None,
            paths: None,
            max_calls_per_method: 10,
            include_context: true,
            include_docstrings: true,
        }
    }
}

/// Result of a repository mapping operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMapResult {
    /// The formatted repository map as text
    pub map_content: String,
    /// Summary statistics about the mapping
    pub summary: RepoMapSummary,
    /// Raw method information organized by file
    pub methods_by_file: HashMap<String, Vec<MethodInfo>>,
}

/// Summary statistics about a repository mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMapSummary {
    /// Total number of files scanned
    pub files_scanned: usize,
    /// Total number of methods/elements found
    pub total_methods: usize,
    /// Breakdown by file type
    pub file_type_counts: HashMap<String, usize>,
    /// Breakdown by method type
    pub method_type_counts: HashMap<String, usize>,
    /// List of supported languages found
    pub languages_found: Vec<String>,
} 