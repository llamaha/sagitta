use std::path::{Path, PathBuf};
use std::collections::HashMap;
use log::{debug, info, warn, error};
use super::search::SearchResult;

/// Constants for ranking adjustments
const FILENAME_EXACT_MATCH_BOOST: f32 = 2.0;
const FILENAME_PARTIAL_MATCH_BOOST: f32 = 1.5;
const FILENAME_KEYWORD_BOOST: f32 = 1.25;
const PATH_KEYWORD_BOOST: f32 = 1.15;
const MODULE_INCLUSION_BOOST: f32 = 1.3;
const METHOD_SIGNATURE_BOOST: f32 = 1.2;

/// Structure to hold term weights for different path components
#[derive(Debug, Clone)]
pub struct PathComponentWeights {
    /// Weighted terms found in file names
    pub filename_terms: HashMap<String, f32>,
    /// Weighted terms found in path components
    pub path_component_terms: HashMap<String, f32>,
    /// Special directory patterns that should be boosted for certain queries
    pub special_directories: HashMap<String, f32>,
}

impl Default for PathComponentWeights {
    fn default() -> Self {
        let mut filename_terms = HashMap::new();
        let mut path_component_terms = HashMap::new();
        let mut special_directories = HashMap::new();
        
        // Initialize with common code organization terms for filenames
        for term in &["main", "core", "lib", "utils", "helpers", "base", "common"] {
            filename_terms.insert(term.to_string(), 1.1);
        }
        
        // Initialize with code organization directory patterns
        for dir in &["src", "lib", "core", "include", "controllers", "models", "views"] {
            path_component_terms.insert(dir.to_string(), 1.05);
        }
        
        // Initialize domain-specific directory patterns
        special_directories.insert("auth".to_string(), 1.2);
        special_directories.insert("authentication".to_string(), 1.2);
        special_directories.insert("authorization".to_string(), 1.2);
        special_directories.insert("security".to_string(), 1.15);
        special_directories.insert("api".to_string(), 1.1);
        special_directories.insert("controllers".to_string(), 1.15);
        special_directories.insert("models".to_string(), 1.1);
        special_directories.insert("core".to_string(), 1.1);
        
        Self {
            filename_terms,
            path_component_terms,
            special_directories,
        }
    }
}

/// Analyzes a file path to extract important components for ranking
pub fn analyze_file_path(file_path: &str) -> (String, Vec<String>) {
    let path = Path::new(file_path);
    
    // Extract the filename without extension
    let filename = path.file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
        .to_string();
    
    // Extract path components
    let components = path.components()
        .filter_map(|comp| {
            let s = comp.as_os_str().to_string_lossy().to_lowercase();
            if !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        })
        .collect();
    
    (filename, components)
}

/// Apply filename and path-based ranking boosts
pub fn apply_path_ranking(results: &mut Vec<SearchResult>, query: &str, weights: &PathComponentWeights) {
    let query_terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    
    for result in results.iter_mut() {
        let file_path = &result.file_path;
        let (filename, path_components) = analyze_file_path(file_path);
        let mut boost_factor = 1.0;
        
        // Check for exact filename match with query terms
        for term in &query_terms {
            if filename == *term {
                boost_factor *= FILENAME_EXACT_MATCH_BOOST;
                debug!("Exact filename match boost for {}: {}", file_path, FILENAME_EXACT_MATCH_BOOST);
                break;
            }
        }
        
        // Check for partial filename matches
        for term in &query_terms {
            if filename.contains(term) {
                boost_factor *= FILENAME_PARTIAL_MATCH_BOOST;
                debug!("Partial filename match boost for {}: {}", file_path, FILENAME_PARTIAL_MATCH_BOOST);
                break;
            }
        }
        
        // Check for special terms in the filename
        for (term, weight) in &weights.filename_terms {
            if filename.contains(term) {
                boost_factor *= weight;
                debug!("Filename keyword '{}' boost for {}: {}", term, file_path, weight);
            }
        }
        
        // Check path components for special directories
        for component in &path_components {
            // Match path components against special directories
            for (dir, weight) in &weights.special_directories {
                if component.contains(dir) {
                    boost_factor *= weight;
                    debug!("Special directory '{}' boost for {}: {}", dir, file_path, weight);
                }
            }
            
            // Match path components against weighted terms
            for (term, weight) in &weights.path_component_terms {
                if component.contains(term) {
                    boost_factor *= weight;
                    debug!("Path component '{}' boost for {}: {}", term, file_path, weight);
                }
            }
            
            // Check if query terms are in path components
            for term in &query_terms {
                if component.contains(term) {
                    boost_factor *= PATH_KEYWORD_BOOST;
                    debug!("Query term '{}' in path boost for {}: {}", term, file_path, PATH_KEYWORD_BOOST);
                    break;
                }
            }
        }
        
        // Apply the calculated boost factor to the similarity score
        let original_similarity = result.similarity;
        result.similarity = (result.similarity * boost_factor).min(1.0);
        
        if (result.similarity - original_similarity).abs() > 0.01 {
            debug!("Applied path ranking to {}: {} -> {}", 
                file_path, original_similarity, result.similarity);
        }
    }
}

/// Detects significant terms in paths that may indicate module importance
pub fn detect_module_inclusion(file_path: &str, query_terms: &[String]) -> f32 {
    let path = Path::new(file_path);
    let content = std::fs::read_to_string(path).unwrap_or_default();
    
    // Use a simplified approach to detect module inclusion
    // In a more sophisticated version, this would use language-specific parsers
    let mut boost = 1.0;
    
    // Check for module inclusion patterns in different languages
    let inclusion_patterns = [
        "include ", "require ", "import ", "use ", "from ", "#include", 
        "extend ", "implements ", "extends "
    ];
    
    for pattern in &inclusion_patterns {
        if content.contains(pattern) {
            for term in query_terms {
                // Check if any query term appears in a line with an inclusion pattern
                let pattern_with_term = format!("{}{}", pattern, term);
                if content.contains(&pattern_with_term) {
                    boost *= MODULE_INCLUSION_BOOST;
                    debug!("Module inclusion boost for {}: {}", file_path, MODULE_INCLUSION_BOOST);
                    break;
                }
            }
        }
    }
    
    boost
}

/// Extracts and analyzes method signatures for relevance ranking
pub fn analyze_method_signatures(file_content: &str, query_terms: &[String]) -> f32 {
    let mut boost = 1.0;
    
    // Simple patterns to detect method signatures in various languages
    let method_patterns = [
        "fn ", "def ", "function ", "sub ", "method ", "pub fn", "void ", "int ", "string ",
        "class ", "struct ", "trait ", "interface ", "module "
    ];
    
    for term in query_terms {
        for pattern in &method_patterns {
            // Check for method signatures containing query terms
            let signature_pattern = format!("{}{}", pattern, term);
            if file_content.contains(&signature_pattern) {
                boost *= METHOD_SIGNATURE_BOOST;
                debug!("Method signature boost for term '{}': {}", term, METHOD_SIGNATURE_BOOST);
                break;
            }
        }
    }
    
    boost
}

/// Apply code structure analysis ranking improvements
pub fn apply_code_structure_ranking(results: &mut Vec<SearchResult>, query: &str) {
    let query_terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    
    for result in results.iter_mut() {
        let file_path = &result.file_path;
        let path = Path::new(file_path);
        
        // Skip if file doesn't exist or can't be read
        if !path.exists() {
            continue;
        }
        
        let mut boost_factor = 1.0;
        
        // Apply module inclusion boost
        boost_factor *= detect_module_inclusion(file_path, &query_terms);
        
        // Read file content for method signature analysis
        if let Ok(content) = std::fs::read_to_string(path) {
            boost_factor *= analyze_method_signatures(&content, &query_terms);
        }
        
        // Apply the boost factor
        let original_similarity = result.similarity;
        result.similarity = (result.similarity * boost_factor).min(1.0);
        
        if (result.similarity - original_similarity).abs() > 0.01 {
            debug!("Applied code structure ranking to {}: {} -> {}", 
                file_path, original_similarity, result.similarity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_analyze_file_path() {
        let (filename, components) = analyze_file_path("src/controllers/auth_controller.rb");
        assert_eq!(filename, "auth_controller");
        assert_eq!(components, vec!["src", "controllers", "auth_controller.rb"]);
    }
    
    #[test]
    fn test_path_ranking() {
        let mut results = vec![
            SearchResult {
                file_path: "src/controllers/auth_controller.rb".to_string(),
                similarity: 0.5,
                snippet: "class AuthController".to_string(),
                code_context: None,
            },
            SearchResult {
                file_path: "src/models/user.rb".to_string(),
                similarity: 0.5,
                snippet: "class User".to_string(),
                code_context: None,
            },
        ];
        
        let weights = PathComponentWeights::default();
        apply_path_ranking(&mut results, "auth", &weights);
        
        // The auth_controller.rb file should have a higher similarity score
        assert!(results[0].similarity > 0.5);
    }
} 