use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use std::fs;
use anyhow::Result;
use log::{debug, info, warn, error};

use super::search::SearchResult;
use super::path_relevance::{PathRelevanceScorer, ParsedPath};
use super::code_structure::{CodeContext, CodeStructureAnalyzer, CodeLanguage, TypeKind};

/// Factor weights for different ranking components
pub struct RankingWeights {
    /// Weight for base semantic similarity score
    pub semantic_similarity_weight: f32,
    /// Weight for file type importance
    pub file_type_weight: f32,
    /// Weight for dependency relationships
    pub dependency_weight: f32,
    /// Weight for code complexity
    pub complexity_weight: f32,
    /// Weight for codebase centrality
    pub centrality_weight: f32,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            semantic_similarity_weight: 1.0,
            file_type_weight: 0.8,
            dependency_weight: 0.7,
            complexity_weight: 0.5,
            centrality_weight: 0.6,
        }
    }
}

/// Configuration for the ranking algorithm
#[derive(Debug, Clone)]
pub struct RankingConfig {
    /// Weights for different file types
    pub file_type_weights: HashMap<String, f32>,
    /// Weights for different file categories
    pub file_category_weights: HashMap<FileCategory, f32>,
    /// Boost factor for main implementation files
    pub main_implementation_boost: f32,
    /// Boost factor for interface definitions
    pub interface_definition_boost: f32,
    /// Penalty factor for test files
    pub test_file_penalty: f32,
    /// Penalty factor for mock implementations
    pub mock_implementation_penalty: f32,
    /// Penalty factor for documentation files
    pub documentation_penalty: f32,
    /// Threshold for considering a file as central to the codebase
    pub centrality_threshold: usize,
}

impl Default for RankingConfig {
    fn default() -> Self {
        let mut file_type_weights = HashMap::new();
        // Common implementation file extensions
        file_type_weights.insert("rs".to_string(), 1.2);   // Rust
        file_type_weights.insert("go".to_string(), 1.25);  // Go (increased from 1.2)
        file_type_weights.insert("py".to_string(), 1.2);   // Python
        file_type_weights.insert("rb".to_string(), 1.2);   // Ruby
        file_type_weights.insert("js".to_string(), 1.2);   // JavaScript
        file_type_weights.insert("ts".to_string(), 1.2);   // TypeScript
        // Interface/header files
        file_type_weights.insert("h".to_string(), 1.1);    // C/C++ header
        file_type_weights.insert("hpp".to_string(), 1.1);  // C++ header
        file_type_weights.insert("proto".to_string(), 1.1); // Protocol Buffers
        // Documentation files
        file_type_weights.insert("md".to_string(), 0.7);   // Markdown
        file_type_weights.insert("txt".to_string(), 0.7);  // Text
        file_type_weights.insert("rst".to_string(), 0.7);  // reStructuredText
        // Configuration files
        file_type_weights.insert("json".to_string(), 0.8); // JSON
        file_type_weights.insert("yaml".to_string(), 0.8); // YAML
        file_type_weights.insert("toml".to_string(), 0.8); // TOML

        let mut file_category_weights = HashMap::new();
        file_category_weights.insert(FileCategory::MainImplementation, 1.3);
        file_category_weights.insert(FileCategory::Interface, 1.1);
        file_category_weights.insert(FileCategory::Test, 0.7);
        file_category_weights.insert(FileCategory::Mock, 0.6);
        file_category_weights.insert(FileCategory::Documentation, 0.5);
        file_category_weights.insert(FileCategory::Configuration, 0.6);
        file_category_weights.insert(FileCategory::Build, 0.7);
        file_category_weights.insert(FileCategory::Unknown, 0.9);

        Self {
            file_type_weights,
            file_category_weights,
            main_implementation_boost: 1.5,
            interface_definition_boost: 1.2,
            test_file_penalty: 0.7,
            mock_implementation_penalty: 0.6,
            documentation_penalty: 0.5,
            centrality_threshold: 5,
        }
    }
}

/// Categorization of files in the codebase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileCategory {
    MainImplementation,
    Interface,
    Test,
    Mock,
    Documentation,
    Configuration,
    Build,
    Unknown,
}

/// Structure to hold dependency relationships for files
#[derive(Debug)]
pub struct DependencyGraph {
    /// Maps file paths to the files that import/use them
    pub incoming_deps: HashMap<String, HashSet<String>>,
    /// Maps file paths to the files they import/use
    pub outgoing_deps: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            incoming_deps: HashMap::new(),
            outgoing_deps: HashMap::new(),
        }
    }

    /// Add a dependency relationship where `from` depends on `to`
    pub fn add_dependency(&mut self, from: &str, to: &str) {
        self.outgoing_deps
            .entry(from.to_string())
            .or_insert_with(HashSet::new)
            .insert(to.to_string());
            
        self.incoming_deps
            .entry(to.to_string())
            .or_insert_with(HashSet::new)
            .insert(from.to_string());
    }

    /// Get the number of incoming dependencies for a file
    pub fn incoming_count(&self, file_path: &str) -> usize {
        self.incoming_deps
            .get(file_path)
            .map_or(0, |deps| deps.len())
    }

    /// Get the number of outgoing dependencies for a file
    pub fn outgoing_count(&self, file_path: &str) -> usize {
        self.outgoing_deps
            .get(file_path)
            .map_or(0, |deps| deps.len())
    }

    /// Calculate a centrality score for a file based on its dependencies
    pub fn calculate_centrality(&self, file_path: &str) -> f32 {
        let incoming = self.incoming_count(file_path) as f32;
        let outgoing = self.outgoing_count(file_path) as f32;
        
        // Files with both incoming and outgoing dependencies are more central
        if incoming > 0.0 && outgoing > 0.0 {
            (incoming * 1.5 + outgoing) / 2.0
        } else {
            incoming + outgoing
        }
    }
}

/// Main ranking engine for code search results
pub struct CodeRankingEngine {
    config: RankingConfig,
    weights: RankingWeights,
    dependency_graph: DependencyGraph,
    code_analyzer: CodeStructureAnalyzer,
    complexity_cache: HashMap<String, f32>,
}

impl CodeRankingEngine {
    pub fn new() -> Self {
        Self {
            config: RankingConfig::default(),
            weights: RankingWeights::default(),
            dependency_graph: DependencyGraph::new(),
            code_analyzer: CodeStructureAnalyzer::new(),
            complexity_cache: HashMap::new(),
        }
    }

    pub fn with_config(config: RankingConfig) -> Self {
        Self {
            config,
            weights: RankingWeights::default(),
            dependency_graph: DependencyGraph::new(),
            code_analyzer: CodeStructureAnalyzer::new(),
            complexity_cache: HashMap::new(),
        }
    }

    pub fn with_weights(weights: RankingWeights) -> Self {
        Self {
            config: RankingConfig::default(),
            weights,
            dependency_graph: DependencyGraph::new(),
            code_analyzer: CodeStructureAnalyzer::new(),
            complexity_cache: HashMap::new(),
        }
    }

    /// Categorize a file based on its path and name
    pub fn categorize_file(&self, file_path: &str) -> FileCategory {
        let path = Path::new(file_path);
        let file_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        // Check for test files
        if file_name.contains("_test") || 
           file_name.contains("_spec") || 
           file_name.contains(".test") || 
           file_name.starts_with("test_") ||
           path.components().any(|c| c.as_os_str().to_string_lossy().contains("test")) {
            return FileCategory::Test;
        }
        
        // Check for mock files
        if file_name.contains("mock") || 
           file_name.contains("stub") || 
           file_name.contains("fake") {
            return FileCategory::Mock;
        }
        
        // Check for documentation
        if extension == "md" || 
           extension == "txt" || 
           extension == "rst" || 
           extension == "adoc" ||
           file_name == "README" || 
           file_name == "LICENSE" {
            return FileCategory::Documentation;
        }
        
        // Check for configuration files
        if extension == "json" || 
           extension == "yaml" || 
           extension == "yml" || 
           extension == "toml" || 
           extension == "xml" || 
           file_name == "Makefile" || 
           file_name == "Dockerfile" ||
           file_name.starts_with(".") {
            return FileCategory::Configuration;
        }
        
        // Check for build files
        if file_name == "BUILD" || 
           file_name == "WORKSPACE" || 
           file_name.ends_with("file") || // Makefile, Dockerfile, etc.
           extension == "bazel" {
            return FileCategory::Build;
        }
        
        // Check for interface files
        if extension == "h" || 
           extension == "hpp" || 
           extension == "proto" ||
           file_name.ends_with("_interface") ||
           file_name.ends_with("Interface") {
            return FileCategory::Interface;
        }
        
        // Default to main implementation
        FileCategory::MainImplementation
    }

    /// Calculate code complexity based on various metrics
    pub fn calculate_complexity(&mut self, file_path: &str) -> f32 {
        // Check cache first
        if let Some(complexity) = self.complexity_cache.get(file_path) {
            return *complexity;
        }
        
        // Read the file content
        let path = Path::new(file_path);
        if !path.exists() {
            return 0.0;
        }
        
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => return 0.0,
        };
        
        // Simple complexity metrics:
        // 1. Number of lines
        let line_count = content.lines().count() as f32;
        
        // 2. Try to analyze code structure
        let code_context = match self.code_analyzer.analyze_file(file_path) {
            Ok(context) => context,
            Err(_) => return line_count / 500.0, // Fallback to line count only
        };
        
        // 3. Method complexity - number of methods and their size
        let method_count = code_context.methods.len() as f32;
        
        // 4. Type complexity - number of types defined
        let type_count = code_context.types.len() as f32;
        
        // 5. Import complexity - number of imports
        let import_count = code_context.imports.len() as f32;
        
        // Calculate weighted complexity score
        let complexity = (line_count / 500.0) * 0.3 + 
                         (method_count / 10.0) * 0.3 + 
                         (type_count / 5.0) * 0.2 + 
                         (import_count / 20.0) * 0.2;
        
        // Normalize to a 0.0-1.0 range
        let normalized_complexity = (complexity / 2.0).min(1.0);
        
        // Cache the result
        self.complexity_cache.insert(file_path.to_string(), normalized_complexity);
        
        normalized_complexity
    }

    /// Analyze imports to build a dependency graph
    pub fn build_dependency_graph(&mut self, file_paths: &[String]) -> Result<()> {
        for file_path in file_paths {
            if let Ok(context) = self.code_analyzer.analyze_file(file_path) {
                for import in &context.imports {
                    // For now, we'll use a simple heuristic to match imports to files
                    // A more sophisticated approach would involve resolving module paths
                    for potential_dep in file_paths {
                        if potential_dep == file_path {
                            continue;
                        }
                        
                        let dep_path = Path::new(potential_dep);
                        if let Some(stem) = dep_path.file_stem().and_then(|s| s.to_str()) {
                            if import.module_name.contains(stem) {
                                self.dependency_graph.add_dependency(file_path, potential_dep);
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Apply enhanced ranking to search results
    pub fn rank_results(&mut self, results: &mut Vec<SearchResult>, _query: &str) -> Result<()> {
        // Build a dependency graph if we don't have one yet
        if self.dependency_graph.incoming_deps.is_empty() {
            let file_paths: Vec<String> = results.iter()
                .map(|r| r.file_path.clone())
                .collect();
            
            self.build_dependency_graph(&file_paths)?;
        }
        
        // Apply ranking adjustments to each result
        for result in results.iter_mut() {
            let file_path = &result.file_path;
            let original_score = result.similarity;
            
            // 1. Adjust based on file category
            let category = self.categorize_file(file_path);
            // Store the weight in a local variable to avoid borrow issues
            let category_weight = *self.config.file_category_weights
                .get(&category)
                .unwrap_or(&0.9);
                
            // 2. Adjust based on file extension
            let extension = Path::new(file_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");
                
            // Store the weight in a local variable to avoid borrow issues
            let extension_weight = *self.config.file_type_weights
                .get(extension)
                .unwrap_or(&1.0);
            
            // Store weight values in local variables to avoid borrow issues later
            let file_type_weight = self.weights.file_type_weight;
            let complexity_weight = self.weights.complexity_weight;
            let centrality_weight = self.weights.centrality_weight;
            let main_impl_boost = self.config.main_implementation_boost;
            let interface_boost = self.config.interface_definition_boost;
            let test_penalty = self.config.test_file_penalty;
            let mock_penalty = self.config.mock_implementation_penalty;
            let doc_penalty = self.config.documentation_penalty;
            let centrality_threshold = self.config.centrality_threshold as f32;
                
            // 3. Calculate complexity
            let complexity = self.calculate_complexity(file_path);
            
            // 4. Calculate centrality
            let centrality = self.dependency_graph.calculate_centrality(file_path);
            let is_central = centrality >= centrality_threshold;
            
            // Apply the relevant boosts or penalties
            let mut adjusted_score = original_score;
            
            // File category adjustment
            adjusted_score *= category_weight * file_type_weight;
            
            // File extension adjustment
            adjusted_score *= extension_weight * file_type_weight;
            
            // Apply specific boosts/penalties based on file category
            match category {
                FileCategory::MainImplementation => {
                    adjusted_score *= main_impl_boost;
                }
                FileCategory::Interface => {
                    adjusted_score *= interface_boost;
                }
                FileCategory::Test => {
                    adjusted_score *= test_penalty;
                }
                FileCategory::Mock => {
                    adjusted_score *= mock_penalty;
                }
                FileCategory::Documentation => {
                    adjusted_score *= doc_penalty;
                }
                _ => {}
            }
            
            // Complexity adjustment - prefer moderately complex files over very simple or very complex ones
            let complexity_factor = if complexity > 0.3 && complexity < 0.7 {
                1.0 + (0.5 - (complexity - 0.5).abs()) * complexity_weight
            } else {
                1.0 - (0.5 - (complexity - 0.5).abs()) * complexity_weight * 0.5
            };
            
            adjusted_score *= complexity_factor;
            
            // Centrality adjustment - boost central files
            if is_central {
                adjusted_score *= 1.0 + (centrality / 10.0).min(0.5) * centrality_weight;
            }
            
            // Ensure the score stays in a reasonable range
            result.similarity = adjusted_score.min(1.0).max(0.0);
            
            // Log significant adjustments for debugging
            if (result.similarity - original_score).abs() > 0.1 {
                debug!(
                    "Adjusted score for {}: {} -> {} (category={:?}, complexity={:.2}, centrality={:.2})",
                    file_path, original_score, result.similarity, category, complexity, centrality
                );
            }
        }
        
        // Re-sort results by the adjusted scores
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        
        Ok(())
    }

    /// Analyze and add explanation factors to search results
    pub fn add_explanation_factors(&self, results: &mut Vec<SearchResult>) {
        for result in results.iter_mut() {
            let file_path = &result.file_path;
            let category = self.categorize_file(file_path);
            
            // Create a string explaining why this result was ranked highly
            let mut factors = Vec::new();
            
            // Add category information
            match category {
                FileCategory::MainImplementation => {
                    factors.push("Main implementation file".to_string());
                }
                FileCategory::Interface => {
                    factors.push("Interface/definition file".to_string());
                }
                FileCategory::Test => {
                    factors.push("Test file".to_string());
                }
                FileCategory::Mock => {
                    factors.push("Mock implementation".to_string());
                }
                FileCategory::Documentation => {
                    factors.push("Documentation file".to_string());
                }
                FileCategory::Configuration => {
                    factors.push("Configuration file".to_string());
                }
                FileCategory::Build => {
                    factors.push("Build file".to_string());
                }
                FileCategory::Unknown => {}
            }
            
            // Add centrality information
            let centrality = self.dependency_graph.calculate_centrality(file_path);
            if centrality >= self.config.centrality_threshold as f32 {
                factors.push(format!("Central file ({} references)", centrality as usize));
            }
            
            // Add complexity information if interesting
            if let Some(complexity) = self.complexity_cache.get(file_path) {
                if *complexity > 0.7 {
                    factors.push("High complexity".to_string());
                } else if *complexity < 0.3 {
                    factors.push("Low complexity".to_string());
                }
            }
            
            // Store the explanation in the code_context field if it's not already used
            if factors.is_empty() {
                continue;
            }
            
            let explanation = format!("Ranking factors: {}", factors.join(", "));
            
            if let Some(context) = &result.code_context {
                result.code_context = Some(format!("{}\n{}", context, explanation));
            } else {
                result.code_context = Some(explanation);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_file_categorization() {
        let engine = CodeRankingEngine::new();
        
        // Test files
        assert_eq!(engine.categorize_file("src/test/test_parser.rs"), FileCategory::Test);
        assert_eq!(engine.categorize_file("tests/integration_tests/parser_test.go"), FileCategory::Test);
        assert_eq!(engine.categorize_file("spec/models/user_spec.rb"), FileCategory::Test);
        
        // Mock files
        assert_eq!(engine.categorize_file("src/mocks/mock_database.rs"), FileCategory::Mock);
        assert_eq!(engine.categorize_file("test/stubs/stub_client.rb"), FileCategory::Test);
        
        // Documentation
        assert_eq!(engine.categorize_file("docs/API.md"), FileCategory::Documentation);
        assert_eq!(engine.categorize_file("README.md"), FileCategory::Documentation);
        
        // Configuration
        assert_eq!(engine.categorize_file("config/app.yaml"), FileCategory::Configuration);
        assert_eq!(engine.categorize_file(".gitignore"), FileCategory::Configuration);
        assert_eq!(engine.categorize_file("Dockerfile"), FileCategory::Configuration);
        
        // Implementation
        assert_eq!(engine.categorize_file("src/models/user.rb"), FileCategory::MainImplementation);
        assert_eq!(engine.categorize_file("lib/parser.rs"), FileCategory::MainImplementation);
    }
    
    #[test]
    fn test_dependency_graph() {
        let mut graph = DependencyGraph::new();
        
        graph.add_dependency("a.rs", "b.rs");
        graph.add_dependency("a.rs", "c.rs");
        graph.add_dependency("b.rs", "c.rs");
        graph.add_dependency("d.rs", "a.rs");
        
        assert_eq!(graph.incoming_count("a.rs"), 1);
        assert_eq!(graph.outgoing_count("a.rs"), 2);
        
        assert_eq!(graph.incoming_count("b.rs"), 1);
        assert_eq!(graph.outgoing_count("b.rs"), 1);
        
        assert_eq!(graph.incoming_count("c.rs"), 2);
        assert_eq!(graph.outgoing_count("c.rs"), 0);
        
        assert_eq!(graph.incoming_count("d.rs"), 0);
        assert_eq!(graph.outgoing_count("d.rs"), 1);
        
        // c.rs should have the highest centrality due to incoming references
        assert!(graph.calculate_centrality("c.rs") > graph.calculate_centrality("a.rs"));
        assert!(graph.calculate_centrality("c.rs") > graph.calculate_centrality("b.rs"));
        assert!(graph.calculate_centrality("c.rs") > graph.calculate_centrality("d.rs"));
    }
} 