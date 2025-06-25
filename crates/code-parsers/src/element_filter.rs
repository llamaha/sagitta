use std::collections::HashSet;

/// Core element types that should be indexed for search
/// These represent the most important code structures that users typically search for
pub fn is_core_element_type(element_type: &str, language: Option<&str>) -> bool {
    match element_type {
        // Universal core types
        "function" | "method" | "struct" | "class" | "enum" | "interface" | "trait" | "module" => true,
        
        // Special case for Go where 'type' includes struct definitions
        "type" if language == Some("go") => true,
        
        // Markdown heading sections are valuable for documentation
        s if s.starts_with("h") && s.ends_with("_section") => true,
        
        // Allow markdown root content and split sections
        s if s.starts_with("root_") || s.contains("_section_split_") => true,
        
        // Fallback chunks for unparseable content
        s if s.starts_with("fallback_chunk_") => true,
        
        // Everything else is filtered out
        _ => false,
    }
}

/// Get a set of core element types for a specific language
pub fn get_core_element_types(language: &str) -> HashSet<&'static str> {
    let mut types = HashSet::new();
    
    // Common types across most languages
    types.insert("function");
    types.insert("class");
    types.insert("module");
    
    match language {
        "rust" => {
            types.insert("struct");
            types.insert("enum");
            types.insert("trait");
            // Note: Rust methods are parsed as "function" type
        }
        "python" => {
            // Python only has function and class
        }
        "go" => {
            types.insert("method");
            types.insert("type"); // Includes struct definitions
        }
        "javascript" | "typescript" => {
            types.insert("method");
            if language == "typescript" {
                types.insert("interface");
                types.insert("enum");
            }
        }
        "ruby" => {
            types.insert("method");
            types.insert("singleton_method");
        }
        "markdown" => {
            // Clear types for markdown and add heading sections
            types.clear();
            types.insert("root_content");
            types.insert("root_plain_text");
            types.insert("h1_section");
            types.insert("h2_section");
            types.insert("h3_section");
            types.insert("h4_section");
            types.insert("h5_section");
            types.insert("h6_section");
            // Note: Split sections are handled by prefix matching in is_core_element_type
        }
        _ => {
            // For unknown languages, keep basic types
        }
    }
    
    types
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_element_types() {
        assert!(is_core_element_type("function", None));
        assert!(is_core_element_type("class", None));
        assert!(is_core_element_type("struct", None));
        assert!(is_core_element_type("enum", None));
        assert!(is_core_element_type("interface", None));
        assert!(is_core_element_type("trait", None));
        assert!(is_core_element_type("method", None));
        assert!(is_core_element_type("module", None));
        
        // Should filter out non-core types
        assert!(!is_core_element_type("use", None));
        assert!(!is_core_element_type("type_alias", None));
        assert!(!is_core_element_type("static", None));
        assert!(!is_core_element_type("const", None));
        assert!(!is_core_element_type("var", None));
        assert!(!is_core_element_type("import", None));
        assert!(!is_core_element_type("statement", None));
        assert!(!is_core_element_type("macro_definition", None));
        assert!(!is_core_element_type("macro_invocation", None));
        assert!(!is_core_element_type("extern_crate", None));
        assert!(!is_core_element_type("union", None));
        
        // Special case for Go
        assert!(is_core_element_type("type", Some("go")));
        assert!(!is_core_element_type("type", Some("typescript")));
        assert!(!is_core_element_type("type", None));
        
        // Markdown sections
        assert!(is_core_element_type("h1_section", None));
        assert!(is_core_element_type("h2_section", None));
        assert!(is_core_element_type("h3_section", None));
        assert!(is_core_element_type("h1_section_split_2", None));
        assert!(is_core_element_type("root_content", None));
        assert!(is_core_element_type("root_plain_text", None));
        assert!(is_core_element_type("root_plain_text_split_3", None));
        
        // Fallback chunks
        assert!(is_core_element_type("fallback_chunk_0", None));
        assert!(is_core_element_type("fallback_chunk_10", None));
    }
    
    #[test]
    fn test_language_specific_types() {
        let rust_types = get_core_element_types("rust");
        assert!(rust_types.contains("struct"));
        assert!(rust_types.contains("enum"));
        assert!(rust_types.contains("trait"));
        
        let ts_types = get_core_element_types("typescript");
        assert!(ts_types.contains("interface"));
        assert!(ts_types.contains("enum"));
        assert!(ts_types.contains("method"));
        
        let go_types = get_core_element_types("go");
        assert!(go_types.contains("type"));
        assert!(go_types.contains("method"));
        
        let md_types = get_core_element_types("markdown");
        assert!(md_types.contains("h1_section"));
        assert!(!md_types.contains("function"));
    }
}