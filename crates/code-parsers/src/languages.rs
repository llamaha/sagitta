use std::collections::HashSet;

lazy_static::lazy_static! {
    /// A HashSet containing the file extensions of currently supported languages.
    pub static ref SUPPORTED_LANGUAGES: HashSet<&'static str> = {
        let mut s = HashSet::new();
        s.insert("rs");
        s.insert("md");
        s.insert("go");
        s.insert("js");
        s.insert("jsx");
        s.insert("ts");
        s.insert("tsx");
        s.insert("yaml");
        s.insert("yml");
        s.insert("rb");
        s.insert("py");
        s
    };
}

/// Maps file extensions to language names for parser selection
pub fn get_language_from_extension(extension: &str) -> String {
    match extension.to_lowercase().as_str() {
        "rs" => "rust".to_string(),
        "py" => "python".to_string(),
        "js" | "jsx" => "javascript".to_string(),
        "ts" | "tsx" => "typescript".to_string(),
        "go" => "go".to_string(),
        "rb" => "ruby".to_string(),
        "md" => "markdown".to_string(),
        "yaml" | "yml" => "yaml".to_string(),
        "html" | "htm" => "html".to_string(),
        _ => "fallback".to_string(),
    }
} 