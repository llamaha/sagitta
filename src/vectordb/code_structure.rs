use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use std::fs;
use log::{debug, info, warn, error};
use anyhow::Result;
use serde::{Serialize, Deserialize};

/// Represents code context extracted from a file
#[derive(Debug, Clone)]
pub struct CodeContext {
    pub file_path: String,
    pub methods: Vec<MethodInfo>,
    pub types: Vec<TypeInfo>,
    pub imports: Vec<ImportInfo>,
    pub language: CodeLanguage,
}

/// Supported programming languages
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CodeLanguage {
    Rust,
    Ruby,
    Python,
    JavaScript,
    TypeScript,
    Go,
    YAML,
    Markdown,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub span: (usize, usize), // Line numbers (start, end)
    pub signature: String,
    pub containing_type: Option<String>,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    pub kind: TypeKind,
    pub span: (usize, usize), // Line numbers (start, end)
    pub containing_module: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Class,
    Struct,
    Trait,
    Enum,
    Interface,
    Module,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub module_name: String,
    pub span: (usize, usize), // Line numbers (start, end)
    pub is_external: bool,
}

/// Analyzes code structure from file content
pub struct CodeStructureAnalyzer {
    // Cache of analyzed files
    analyzed_files: HashMap<String, CodeContext>,
}

impl CodeStructureAnalyzer {
    pub fn new() -> Self {
        Self {
            analyzed_files: HashMap::new(),
        }
    }
    
    /// Analyzes a file for code structure
    pub fn analyze_file(&mut self, file_path: &str) -> Result<&CodeContext> {
        // Check cache first
        if self.analyzed_files.contains_key(file_path) {
            return Ok(&self.analyzed_files[file_path]);
        }
        
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(anyhow::anyhow!("File does not exist: {}", file_path));
        }
        
        // Read file content
        let content = fs::read_to_string(path)?;
        
        // Determine language based on file extension
        let language = self.detect_language(path);
        
        // Parse the file based on language
        let context = match language {
            CodeLanguage::Rust => self.analyze_rust_file(&content, file_path),
            CodeLanguage::Ruby => self.analyze_ruby_file(&content, file_path),
            CodeLanguage::Python => self.analyze_python_file(&content, file_path),
            CodeLanguage::JavaScript | CodeLanguage::TypeScript => 
                self.analyze_js_ts_file(&content, file_path, language),
            CodeLanguage::Go => self.analyze_go_file(&content, file_path),
            CodeLanguage::YAML => self.analyze_yaml_file(&content, file_path),
            CodeLanguage::Markdown => self.analyze_markdown_file(&content, file_path),
            CodeLanguage::Unknown => self.analyze_generic_file(&content, file_path),
        };
        
        // Cache the result
        self.analyzed_files.insert(file_path.to_string(), context);
        
        Ok(&self.analyzed_files[file_path])
    }
    
    /// Determine the code language based on file extension
    fn detect_language(&self, path: &Path) -> CodeLanguage {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => CodeLanguage::Rust,
            Some("rb") => CodeLanguage::Ruby,
            Some("py") => CodeLanguage::Python,
            Some("js") => CodeLanguage::JavaScript,
            Some("ts") | Some("tsx") => CodeLanguage::TypeScript,
            Some("go") => CodeLanguage::Go,
            Some("yml") | Some("yaml") => CodeLanguage::YAML,
            Some("md") => CodeLanguage::Markdown,
            _ => CodeLanguage::Unknown,
        }
    }
    
    /// Extract methods from file content using regex patterns
    fn extract_methods(&self, content: &str, language: &CodeLanguage) -> Vec<MethodInfo> {
        use regex::Regex;
        let mut methods = Vec::new();
        
        // Different regex patterns based on language
        let pattern = match language {
            CodeLanguage::Rust => {
                r"(?m)^\s*(pub(\s*\(.*\))?)?\s*fn\s+([a-zA-Z0-9_]+)\s*(\([^)]*\)).*\{"
            },
            CodeLanguage::Ruby => {
                r"(?m)^\s*(def|class\s+method)\s+([a-zA-Z0-9_?!]+)(?:\s*\(([^)]*)\))?"
            },
            CodeLanguage::Python => {
                r"(?m)^\s*def\s+([a-zA-Z0-9_]+)\s*\(([^)]*)\)"
            },
            CodeLanguage::JavaScript | CodeLanguage::TypeScript => {
                r"(?m)(?:function\s+([a-zA-Z0-9_]+)|([a-zA-Z0-9_]+)\s*=\s*function|\s*([a-zA-Z0-9_]+)\s*\([^)]*\)\s*{)"
            },
            CodeLanguage::Go => {
                r"(?m)^\s*func\s+(?:\([^)]+\)\s+)?([a-zA-Z0-9_]+)\s*\("
            },
            CodeLanguage::YAML => {
                // YAML doesn't have methods in the traditional sense,
                // but we extract keys with function-like values
                r"(?m)^\s*([a-zA-Z0-9_-]+):\s*\|\s*$"
            },
            CodeLanguage::Markdown => {
                // Markdown doesn't have methods in the traditional sense,
                // but we extract headings
                r"(?m)^#+\s+([a-zA-Z0-9_]+)"
            },
            CodeLanguage::Unknown => {
                // Generic pattern that might work across multiple languages
                r"(?m)(?:function|def|fn)\s+([a-zA-Z0-9_]+)"
            },
        };
        
        let regex = Regex::new(pattern).unwrap_or_else(|_| {
            warn!("Failed to compile regex pattern for method extraction");
            Regex::new(r"x^").unwrap() // Regex that won't match anything
        });
        
        for cap in regex.captures_iter(content) {
            // Extract method information based on language
            match language {
                CodeLanguage::Rust => {
                    if let Some(name) = cap.get(3) {
                        let is_public = cap.get(1).is_some();
                        let signature = cap.get(0).map_or("", |m| m.as_str()).to_string();
                        
                        methods.push(MethodInfo {
                            name: name.as_str().to_string(),
                            span: (0, 0), // We'll calculate line numbers later
                            signature,
                            containing_type: None, // Would require deeper parsing
                            is_public,
                        });
                    }
                },
                CodeLanguage::Ruby => {
                    if let Some(name) = cap.get(2) {
                        let signature = cap.get(0).map_or("", |m| m.as_str()).to_string();
                        
                        methods.push(MethodInfo {
                            name: name.as_str().to_string(),
                            span: (0, 0),
                            signature,
                            containing_type: None,
                            is_public: true, // Ruby methods are public by default
                        });
                    }
                },
                CodeLanguage::Python => {
                    if let Some(name) = cap.get(1) {
                        let signature = cap.get(0).map_or("", |m| m.as_str()).to_string();
                        
                        methods.push(MethodInfo {
                            name: name.as_str().to_string(),
                            span: (0, 0),
                            signature,
                            containing_type: None,
                            is_public: !name.as_str().starts_with('_'), // Python convention
                        });
                    }
                },
                CodeLanguage::JavaScript | CodeLanguage::TypeScript => {
                    let name = cap.get(1).or_else(|| cap.get(2)).or_else(|| cap.get(3))
                        .map(|m| m.as_str().to_string());
                    
                    if let Some(name) = name {
                        let signature = cap.get(0).map_or("", |m| m.as_str()).to_string();
                        
                        methods.push(MethodInfo {
                            name,
                            span: (0, 0),
                            signature,
                            containing_type: None,
                            is_public: true, // Simplified assumption
                        });
                    }
                },
                CodeLanguage::Go => {
                    if let Some(name) = cap.get(1) {
                        let signature = cap.get(0).map_or("", |m| m.as_str()).to_string();
                        
                        methods.push(MethodInfo {
                            name: name.as_str().to_string(),
                            span: (0, 0),
                            signature,
                            containing_type: None,
                            is_public: true,
                        });
                    }
                },
                CodeLanguage::YAML => {
                    if let Some(name) = cap.get(1) {
                        methods.push(MethodInfo {
                            name: name.as_str().to_string(),
                            span: (0, 0),
                            signature: cap.get(0).map_or("", |m| m.as_str()).to_string(),
                            containing_type: None,
                            is_public: true,
                        });
                    }
                },
                CodeLanguage::Markdown => {
                    if let Some(name) = cap.get(1) {
                        methods.push(MethodInfo {
                            name: name.as_str().to_string(),
                            span: (0, 0),
                            signature: cap.get(0).map_or("", |m| m.as_str()).to_string(),
                            containing_type: None,
                            is_public: true,
                        });
                    }
                },
                CodeLanguage::Unknown => {
                    if let Some(name) = cap.get(1) {
                        methods.push(MethodInfo {
                            name: name.as_str().to_string(),
                            span: (0, 0),
                            signature: cap.get(0).map_or("", |m| m.as_str()).to_string(),
                            containing_type: None,
                            is_public: true,
                        });
                    }
                },
            }
        }
        
        // Calculate line numbers for methods
        let lines: Vec<&str> = content.lines().collect();
        for method in &mut methods {
            for (i, line) in lines.iter().enumerate() {
                if line.contains(&method.signature) {
                    method.span = (i + 1, i + 1); // Use 1-indexed line numbers
                    break;
                }
            }
        }
        
        methods
    }
    
    /// Extract types (classes, structs, etc.) from file content
    fn extract_types(&self, content: &str, language: &CodeLanguage) -> Vec<TypeInfo> {
        use regex::Regex;
        let mut types = Vec::new();
        
        // Different regex patterns based on language
        let (pattern, kind) = match language {
            CodeLanguage::Rust => {
                (r"(?m)^\s*(pub\s+)?(struct|enum|trait)\s+([a-zA-Z0-9_]+)", TypeKind::Struct)
            },
            CodeLanguage::Ruby => {
                (r"(?m)^\s*(class|module)\s+([a-zA-Z0-9_:]+)(?:\s*<\s*([a-zA-Z0-9_:]+))?", TypeKind::Class)
            },
            CodeLanguage::Python => {
                (r"(?m)^\s*class\s+([a-zA-Z0-9_]+)(?:\s*\(([^)]*)\))?:", TypeKind::Class)
            },
            CodeLanguage::JavaScript | CodeLanguage::TypeScript => {
                (r"(?m)(?:class|interface|enum)\s+([a-zA-Z0-9_]+)(?:\s+extends\s+([a-zA-Z0-9_]+))?", TypeKind::Class)
            },
            CodeLanguage::Go => {
                (r"(?m)^\s*type\s+([a-zA-Z0-9_]+)\s+(struct|interface|\w+)", TypeKind::Struct)
            },
            CodeLanguage::YAML => {
                // For YAML, treat top-level keys as "types"
                (r"(?m)^([a-zA-Z0-9_-]+):\s*(?:$|\n|[\{\[])", TypeKind::Struct)
            },
            CodeLanguage::Markdown => {
                // Markdown doesn't have types in the traditional sense,
                // but we extract headings as "types"
                (r"(?m)^#+\s+([a-zA-Z0-9_]+)", TypeKind::Unknown)
            },
            CodeLanguage::Unknown => {
                (r"(?m)(?:class|struct|interface|enum)\s+([a-zA-Z0-9_]+)", TypeKind::Unknown)
            },
        };
        
        let regex = Regex::new(pattern).unwrap_or_else(|_| {
            warn!("Failed to compile regex pattern for type extraction");
            Regex::new(r"x^").unwrap() // Regex that won't match anything
        });
        
        for cap in regex.captures_iter(content) {
            // Extract type information based on language
            match language {
                CodeLanguage::Rust => {
                    if let (Some(type_kind), Some(name)) = (cap.get(2), cap.get(3)) {
                        let kind = match type_kind.as_str() {
                            "struct" => TypeKind::Struct,
                            "enum" => TypeKind::Enum,
                            "trait" => TypeKind::Trait,
                            _ => TypeKind::Unknown,
                        };
                        
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind,
                            span: (0, 0),
                            containing_module: None,
                        });
                    }
                },
                CodeLanguage::Ruby => {
                    if let (Some(type_kind), Some(name)) = (cap.get(1), cap.get(2)) {
                        let kind = match type_kind.as_str() {
                            "class" => TypeKind::Class,
                            "module" => TypeKind::Module,
                            _ => TypeKind::Unknown,
                        };
                        
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind,
                            span: (0, 0),
                            containing_module: cap.get(3).map(|m| m.as_str().to_string()),
                        });
                    }
                },
                CodeLanguage::Python => {
                    if let Some(name) = cap.get(1) {
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind: TypeKind::Class,
                            span: (0, 0),
                            containing_module: cap.get(2).map(|m| m.as_str().to_string()),
                        });
                    }
                },
                CodeLanguage::JavaScript | CodeLanguage::TypeScript => {
                    if let Some(name) = cap.get(1) {
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind: TypeKind::Class, // Simplified
                            span: (0, 0),
                            containing_module: cap.get(2).map(|m| m.as_str().to_string()),
                        });
                    }
                },
                CodeLanguage::Go => {
                    if let Some(name) = cap.get(1) {
                        let kind = if let Some(type_kind) = cap.get(2) {
                            match type_kind.as_str() {
                                "struct" => TypeKind::Struct,
                                "interface" => TypeKind::Interface,
                                _ => TypeKind::Unknown,
                            }
                        } else {
                            TypeKind::Unknown
                        };
                        
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind,
                            span: (0, 0),
                            containing_module: None,
                        });
                    }
                },
                CodeLanguage::YAML => {
                    if let Some(name) = cap.get(1) {
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind: TypeKind::Struct,
                            span: (0, 0),
                            containing_module: None,
                        });
                    }
                },
                CodeLanguage::Markdown => {
                    if let Some(name) = cap.get(1) {
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind: TypeKind::Unknown,
                            span: (0, 0),
                            containing_module: None,
                        });
                    }
                },
                CodeLanguage::Unknown => {
                    if let Some(name) = cap.get(1) {
                        types.push(TypeInfo {
                            name: name.as_str().to_string(),
                            kind: TypeKind::Unknown,
                            span: (0, 0),
                            containing_module: None,
                        });
                    }
                },
            }
        }
        
        // Calculate line numbers for types
        let lines: Vec<&str> = content.lines().collect();
        for type_info in &mut types {
            for (i, line) in lines.iter().enumerate() {
                if line.contains(&format!(" {}", type_info.name)) {
                    type_info.span = (i + 1, i + 1); // Use 1-indexed line numbers
                    break;
                }
            }
        }
        
        types
    }
    
    /// Extract imports from file content
    fn extract_imports(&self, content: &str, language: &CodeLanguage) -> Vec<ImportInfo> {
        use regex::Regex;
        let mut imports = Vec::new();
        
        // Different regex patterns based on language
        let pattern = match language {
            CodeLanguage::Rust => {
                r"(?m)^\s*use\s+([a-zA-Z0-9_:]+)"
            },
            CodeLanguage::Ruby => {
                r#"(?m)^\s*(?:require|include|extend)\s+['"]?([a-zA-Z0-9_/]+)['"]?"#
            },
            CodeLanguage::Python => {
                r"(?m)^\s*(?:import|from)\s+([a-zA-Z0-9_.]+)"
            },
            CodeLanguage::JavaScript | CodeLanguage::TypeScript => {
                r#"(?m)(?:import\s+.*\s+from\s+['"]([a-zA-Z0-9_@/.-]+)['"]|require\s*\(\s*['"]([a-zA-Z0-9_@/.-]+)['"])"#
            },
            CodeLanguage::Go => {
                r#"(?m)^\s*import\s+(?:"([^"]+)"|(\((?:\s*"[^"]+"\s*)+\)))"#
            },
            CodeLanguage::YAML => {
                // Extract "import", "include" or "!include" in YAML
                r#"(?m)^(?:import|include|!include):\s*['"]?([a-zA-Z0-9_./]+)['"]?"#
            },
            CodeLanguage::Markdown => {
                // Markdown doesn't have imports in the traditional sense,
                // but we extract links as "imports"
                r#"(?m)\[([^\]]+)\]\(([^)]+)\)"#
            },
            CodeLanguage::Unknown => {
                r#"(?m)(?:import|require|use|include)\s+['"]?([a-zA-Z0-9_./]+)['"]?"#
            },
        };
        
        let regex = Regex::new(pattern).unwrap_or_else(|_| {
            warn!("Failed to compile regex pattern for import extraction");
            Regex::new(r"x^").unwrap() // Regex that won't match anything
        });
        
        for cap in regex.captures_iter(content) {
            if let Some(module_name) = cap.get(1).or_else(|| cap.get(2)) {
                imports.push(ImportInfo {
                    module_name: module_name.as_str().to_string(),
                    span: (0, 0),
                    is_external: false, // Would require deeper analysis
                });
            }
        }
        
        // Calculate line numbers for imports and determine if external
        let lines: Vec<&str> = content.lines().collect();
        for import in &mut imports {
            for (i, line) in lines.iter().enumerate() {
                if line.contains(&import.module_name) {
                    import.span = (i + 1, i + 1); // Use 1-indexed line numbers
                    
                    // Simple heuristic for external imports
                    import.is_external = match language {
                        CodeLanguage::Rust => !import.module_name.starts_with("crate::")
                            && !import.module_name.starts_with("self::"),
                        CodeLanguage::Ruby => !import.module_name.starts_with("."),
                        CodeLanguage::Python => !import.module_name.starts_with("."),
                        CodeLanguage::JavaScript | CodeLanguage::TypeScript => 
                            !import.module_name.starts_with(".") && !import.module_name.starts_with("/"),
                        CodeLanguage::Go => !import.module_name.starts_with("."),
                        CodeLanguage::YAML => !import.module_name.starts_with("."),
                        CodeLanguage::Markdown => !import.module_name.starts_with("."),
                        CodeLanguage::Unknown => false,
                    };
                    
                    break;
                }
            }
        }
        
        imports
    }
    
    /// Analyze Rust files for code structure
    fn analyze_rust_file(&self, content: &str, file_path: &str) -> CodeContext {
        let methods = self.extract_methods(content, &CodeLanguage::Rust);
        let types = self.extract_types(content, &CodeLanguage::Rust);
        let imports = self.extract_imports(content, &CodeLanguage::Rust);
        
        CodeContext {
            file_path: file_path.to_string(),
            methods,
            types,
            imports,
            language: CodeLanguage::Rust,
        }
    }
    
    /// Analyze Ruby files for code structure
    fn analyze_ruby_file(&self, content: &str, file_path: &str) -> CodeContext {
        let methods = self.extract_methods(content, &CodeLanguage::Ruby);
        let types = self.extract_types(content, &CodeLanguage::Ruby);
        let imports = self.extract_imports(content, &CodeLanguage::Ruby);
        
        CodeContext {
            file_path: file_path.to_string(),
            methods,
            types,
            imports,
            language: CodeLanguage::Ruby,
        }
    }
    
    /// Analyze Python files for code structure
    fn analyze_python_file(&self, content: &str, file_path: &str) -> CodeContext {
        let methods = self.extract_methods(content, &CodeLanguage::Python);
        let types = self.extract_types(content, &CodeLanguage::Python);
        let imports = self.extract_imports(content, &CodeLanguage::Python);
        
        CodeContext {
            file_path: file_path.to_string(),
            methods,
            types,
            imports,
            language: CodeLanguage::Python,
        }
    }
    
    /// Analyze JavaScript/TypeScript files for code structure
    fn analyze_js_ts_file(&self, content: &str, file_path: &str, language: CodeLanguage) -> CodeContext {
        let methods = self.extract_methods(content, &language);
        let types = self.extract_types(content, &language);
        let imports = self.extract_imports(content, &language);
        
        CodeContext {
            file_path: file_path.to_string(),
            methods,
            types,
            imports,
            language,
        }
    }
    
    /// Analyze Go files for code structure
    fn analyze_go_file(&self, content: &str, file_path: &str) -> CodeContext {
        let methods = self.extract_methods(content, &CodeLanguage::Go);
        let mut types = self.extract_types(content, &CodeLanguage::Go);
        let imports = self.extract_imports(content, &CodeLanguage::Go);
        
        // Process import blocks separately with a more specialized regex
        let mut additional_imports = Vec::new();
        let import_block_regex = regex::Regex::new(r#"(?m)import\s+\(\s*((?:"[^"]+"\s*)+)\)"#).unwrap_or_else(|_| {
            warn!("Failed to compile Go import block regex");
            regex::Regex::new(r"x^").unwrap()
        });
        
        let single_import_regex = regex::Regex::new(r#"(?m)"([^"]+)""#).unwrap_or_else(|_| {
            warn!("Failed to compile Go single import regex");
            regex::Regex::new(r"x^").unwrap()
        });
        
        for block_cap in import_block_regex.captures_iter(content) {
            if let Some(block_content) = block_cap.get(1) {
                // Extract each import from the block
                for import_cap in single_import_regex.captures_iter(block_content.as_str()) {
                    if let Some(module_name) = import_cap.get(1) {
                        additional_imports.push(ImportInfo {
                            module_name: module_name.as_str().to_string(),
                            span: (0, 0), // We'll calculate these later
                            is_external: true, // Assume external by default
                        });
                    }
                }
            }
        }
        
        // Calculate line numbers for the additional imports
        let lines: Vec<&str> = content.lines().collect();
        for import in &mut additional_imports {
            for (i, line) in lines.iter().enumerate() {
                if line.contains(&format!("\"{}\"", import.module_name)) {
                    import.span = (i + 1, i + 1); // Use 1-indexed line numbers
                    break;
                }
            }
        }
        
        // Combine the imports
        let mut all_imports = imports;
        all_imports.extend(additional_imports);
        
        // Also look for struct methods (methods with receivers)
        let struct_method_regex = regex::Regex::new(
            r"(?m)^\s*func\s+\(([a-zA-Z0-9_]+)\s+\*?([a-zA-Z0-9_]+)\)\s+([a-zA-Z0-9_]+)\s*\("
        ).unwrap_or_else(|_| {
            warn!("Failed to compile Go struct method regex");
            regex::Regex::new(r"x^").unwrap()
        });
        
        let mut struct_methods = Vec::new();
        for cap in struct_method_regex.captures_iter(content) {
            if let (Some(_receiver_name), Some(receiver_type), Some(method_name)) = (cap.get(1), cap.get(2), cap.get(3)) {
                struct_methods.push(MethodInfo {
                    name: method_name.as_str().to_string(),
                    span: (0, 0), // We'll calculate line numbers later
                    signature: cap.get(0).map_or("", |m| m.as_str()).to_string(),
                    containing_type: Some(receiver_type.as_str().to_string()),
                    is_public: method_name.as_str().chars().next().map_or(false, |c| c.is_uppercase()),
                });
            }
        }
        
        // Calculate line numbers for struct methods
        for method in &mut struct_methods {
            for (i, line) in lines.iter().enumerate() {
                if line.contains(&method.signature) {
                    method.span = (i + 1, i + 1); // Use 1-indexed line numbers
                    break;
                }
            }
        }
        
        // Combine regular functions and struct methods
        let mut all_methods = methods;
        all_methods.extend(struct_methods);
        
        // Extract interface methods
        let interface_regex = regex::Regex::new(
            r"(?m)^\s*type\s+([a-zA-Z0-9_]+)\s+interface\s*\{([^}]*)\}"
        ).unwrap_or_else(|_| {
            warn!("Failed to compile Go interface regex");
            regex::Regex::new(r"x^").unwrap()
        });
        
        let interface_method_regex = regex::Regex::new(
            r"(?m)^\s*([a-zA-Z0-9_]+)\s*\([^)]*\)"
        ).unwrap_or_else(|_| {
            warn!("Failed to compile Go interface method regex");
            regex::Regex::new(r"x^").unwrap()
        });
        
        let mut interface_methods = Vec::new();
        for cap in interface_regex.captures_iter(content) {
            if let (Some(interface_name), Some(interface_body)) = (cap.get(1), cap.get(2)) {
                let interface_type = interface_name.as_str().to_string();
                
                // Extract interface methods
                for line in interface_body.as_str().lines() {
                    if let Some(cap) = interface_method_regex.captures(line) {
                        if let Some(method_name) = cap.get(1) {
                            interface_methods.push(MethodInfo {
                                name: method_name.as_str().to_string(),
                                span: (0, 0), // Calculate later
                                signature: cap.get(0).map_or("", |m| m.as_str()).to_string(),
                                containing_type: Some(interface_type.clone()),
                                is_public: method_name.as_str().chars().next().map_or(false, |c| c.is_uppercase()),
                            });
                        }
                    }
                }
            }
        }
        
        // Calculate line numbers for interface methods (approximate)
        for method in &mut interface_methods {
            if let Some(containing_type) = &method.containing_type {
                for (i, line) in lines.iter().enumerate() {
                    if line.contains(&format!("type {} interface", containing_type)) {
                        // Interface definition found - check next few lines for method
                        for j in i+1..std::cmp::min(i+20, lines.len()) {
                            if lines[j].contains(&method.name) {
                                method.span = (j + 1, j + 1);
                                break;
                            }
                        }
                        break;
                    }
                }
            }
        }
        
        // Add interface methods to the results
        all_methods.extend(interface_methods);
        
        // Handle exported identifiers properly - in Go, uppercase first letter means public
        for method in &mut all_methods {
            method.is_public = method.name.chars().next().map_or(false, |c| c.is_uppercase());
        }
        
        for type_info in &mut types {
            type_info.kind = if type_info.name.contains("interface") {
                TypeKind::Interface
            } else if type_info.name.contains("struct") {
                TypeKind::Struct
            } else {
                TypeKind::Unknown
            };
        }
        
        CodeContext {
            file_path: file_path.to_string(),
            methods: all_methods,
            types,
            imports: all_imports,
            language: CodeLanguage::Go,
        }
    }
    
    /// Analyze YAML files for code structure
    fn analyze_yaml_file(&self, content: &str, file_path: &str) -> CodeContext {
        let methods = self.extract_methods(content, &CodeLanguage::Unknown);
        let types = self.extract_types(content, &CodeLanguage::Unknown);
        let imports = self.extract_imports(content, &CodeLanguage::Unknown);
        
        CodeContext {
            file_path: file_path.to_string(),
            methods,
            types,
            imports,
            language: CodeLanguage::YAML,
        }
    }
    
    /// Analyze Markdown files for code structure
    fn analyze_markdown_file(&self, content: &str, file_path: &str) -> CodeContext {
        let methods = self.extract_methods(content, &CodeLanguage::Markdown);
        let types = self.extract_types(content, &CodeLanguage::Markdown);
        let imports = self.extract_imports(content, &CodeLanguage::Markdown);
        
        CodeContext {
            file_path: file_path.to_string(),
            methods,
            types,
            imports,
            language: CodeLanguage::Markdown,
        }
    }
    
    /// Analyze generic files for code structure (best effort)
    fn analyze_generic_file(&self, content: &str, file_path: &str) -> CodeContext {
        let methods = self.extract_methods(content, &CodeLanguage::Unknown);
        let types = self.extract_types(content, &CodeLanguage::Unknown);
        let imports = self.extract_imports(content, &CodeLanguage::Unknown);
        
        CodeContext {
            file_path: file_path.to_string(),
            methods,
            types,
            imports,
            language: CodeLanguage::Unknown,
        }
    }

    /// Check if the file contains code elements related to the query
    pub fn contains_relevant_code(&self, context: &CodeContext, query_terms: &[String]) -> bool {
        // Check methods
        for method in &context.methods {
            for term in query_terms {
                if method.name.to_lowercase().contains(&term.to_lowercase()) {
                    return true;
                }
            }
        }
        
        // Check types
        for type_info in &context.types {
            for term in query_terms {
                if type_info.name.to_lowercase().contains(&term.to_lowercase()) {
                    return true;
                }
            }
        }
        
        // Check imports
        for import in &context.imports {
            for term in query_terms {
                if import.module_name.to_lowercase().contains(&term.to_lowercase()) {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Get a relevance score for the file based on code structure
    pub fn calculate_code_relevance(&self, context: &CodeContext, query_terms: &[String]) -> f32 {
        let mut score = 1.0;
        
        // Method match score
        let method_match_count = context.methods.iter()
            .filter(|m| query_terms.iter().any(|term| 
                m.name.to_lowercase().contains(&term.to_lowercase())))
            .count();
            
        if method_match_count > 0 {
            score *= 1.0 + (method_match_count as f32 * 0.2);
        }
        
        // Type match score
        let type_match_count = context.types.iter()
            .filter(|t| query_terms.iter().any(|term| 
                t.name.to_lowercase().contains(&term.to_lowercase())))
            .count();
            
        if type_match_count > 0 {
            score *= 1.0 + (type_match_count as f32 * 0.3);
        }
        
        // Import match score (smaller effect)
        let import_match_count = context.imports.iter()
            .filter(|i| query_terms.iter().any(|term| 
                i.module_name.to_lowercase().contains(&term.to_lowercase())))
            .count();
            
        if import_match_count > 0 {
            score *= 1.0 + (import_match_count as f32 * 0.1);
        }
        
        score
    }
    
    /// Build a relationship graph between files based on imports
    pub fn build_relationship_graph(&mut self, file_paths: &[String]) -> HashMap<String, Vec<String>> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        
        // First pass: analyze all files
        for file_path in file_paths {
            let _ = self.analyze_file(file_path);
        }
        
        // Second pass: build import relationships
        for file_path in file_paths {
            if let Some(context) = self.analyzed_files.get(file_path) {
                // Extract base module name from the file path
                let path = Path::new(file_path);
                let module_name = path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                
                // Find files that import this module
                let dependents: Vec<String> = self.analyzed_files.values()
                    .filter(|ctx| ctx.imports.iter().any(|import| 
                        import.module_name.contains(&module_name)))
                    .map(|ctx| ctx.file_path.clone())
                    .collect();
                
                if !dependents.is_empty() {
                    graph.insert(file_path.clone(), dependents);
                }
            }
        }
        
        graph
    }
    
    /// Clear the analyzer cache
    pub fn clear_cache(&mut self) {
        self.analyzed_files.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_rust_methods() {
        let content = r#"
pub fn hello_world() {
    println!("Hello, world!");
}

fn private_function(x: i32) -> i32 {
    x * 2
}
        "#;
        
        let analyzer = CodeStructureAnalyzer::new();
        let methods = analyzer.extract_methods(content, &CodeLanguage::Rust);
        
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].name, "hello_world");
        assert!(methods[0].is_public);
        assert_eq!(methods[1].name, "private_function");
        assert!(!methods[1].is_public);
    }
    
    #[test]
    fn test_extract_ruby_classes() {
        let content = r#"
class User < ApplicationRecord
  def initialize(name)
    @name = name
  end
  
  def say_hello
    puts "Hello, #{@name}!"
  end
end
        "#;
        
        let analyzer = CodeStructureAnalyzer::new();
        let types = analyzer.extract_types(content, &CodeLanguage::Ruby);
        
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "User");
        assert_eq!(types[0].kind, TypeKind::Class);
        assert_eq!(types[0].containing_module, Some("ApplicationRecord".to_string()));
    }
    
    #[test]
    fn test_extract_imports() {
        let rust_content = r#"
use std::path::Path;
use crate::utils::helpers;
        "#;
        
        let analyzer = CodeStructureAnalyzer::new();
        let imports = analyzer.extract_imports(rust_content, &CodeLanguage::Rust);
        
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].module_name, "std::path::Path");
        assert!(imports[0].is_external);
        assert_eq!(imports[1].module_name, "crate::utils::helpers");
        assert!(!imports[1].is_external);
    }
} 