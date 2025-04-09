use log::debug;
use std::collections::HashMap;

/// Structure to hold query analysis results
#[derive(Debug)]
pub(crate) struct QueryAnalysis {
    pub(crate) query_type: QueryType,
    pub(crate) language_hints: Vec<String>,
}

/// Types of queries that can be handled differently
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum QueryType {
    Generic,
    Definition,     // Looking for definitions, e.g., "what is a trait"
    Usage,          // Looking for usages, e.g., "how to use Option"
    Implementation, // Looking for implementations, e.g., "how to implement Display"
    Function,       // Looking for functions, e.g., "function search_parallel"
    Type,           // Looking for types, e.g., "struct SearchResult"
}

/// Preprocess and analyze the query to improve search results
pub(crate) fn preprocess_query(query: &str) -> QueryAnalysis {
    let query_lower = query.to_lowercase();

    // Language-specific keywords
    let rust_keywords = [
        "rust", "cargo", "crate", "mod", "impl", "trait", "struct", "enum", "fn",
    ];
    let ruby_keywords = ["ruby", "gem", "class", "module", "def", "end", "attr"];
    let go_keywords = [
        "go",
        "golang",
        "func",
        "interface",
        "struct",
        "package",
        "import",
        "goroutine",
        "chan",
        "select",
        "go fmt",
        "gofmt",
        "gomod",
        "receiver",
        "slices",
        "map[",
        "type ",
        "defer",
    ];

    // Detect language hints
    let mut language_hints = Vec::new();
    for &keyword in &rust_keywords {
        if query_lower.contains(keyword) {
            language_hints.push("rust".to_string());
            break;
        }
    }
    for &keyword in &ruby_keywords {
        if query_lower.contains(keyword) {
            language_hints.push("ruby".to_string());
            break;
        }
    }
    for &keyword in &go_keywords {
        if query_lower.contains(keyword) {
            language_hints.push("go".to_string());
            break;
        }
    }

    // Determine query type
    let query_type = if query_lower.contains("what is") || query_lower.contains("definition") {
        QueryType::Definition
    } else if query_lower.contains("how to use")
        || query_lower.contains("usage")
        || query_lower.contains("example")
    {
        QueryType::Usage
    } else if query_lower.contains("how to implement") || query_lower.contains("implementation")
    {
        QueryType::Implementation
    } else if query_lower.contains("function")
        || query_lower.contains("method")
        || query_lower.contains("fn ")
    {
        QueryType::Function
    } else if query_lower.contains("struct")
        || query_lower.contains("trait")
        || query_lower.contains("enum")
        || query_lower.contains("class")
        || query_lower.contains("type")
    {
        QueryType::Type
    } else {
        QueryType::Generic
    };

    QueryAnalysis {
        query_type,
        language_hints,
    }
}

/// Determine optimal weights for hybrid search based on query analysis
pub(crate) fn determine_optimal_weights(
    query: &str,
    query_analysis: &QueryAnalysis,
    initial_vector_weight: f32,
    initial_bm25_weight: f32,
) -> (f32, f32) {
    // Get query characteristics
    let query_lower = query.to_lowercase();
    let term_count = query_lower.split_whitespace().count();

    // Start with initial weights (e.g., the constants)
    let mut vector_weight = initial_vector_weight;
    let mut bm25_weight = initial_bm25_weight;

    // 1. Query length and complexity adjustments - shorter queries benefit from lexical search
    if term_count <= 2 {
        // Short queries likely benefit from higher lexical matching
        vector_weight = 0.4;
        bm25_weight = 0.6;
        debug!(
            "Short query ({}), increasing BM25 weight: vector={:.2}, bm25={:.2}",
            term_count, vector_weight, bm25_weight
        );
    } else if term_count >= 6 {
        // Long queries likely benefit from higher semantic matching
        vector_weight = 0.8;
        bm25_weight = 0.2;
        debug!(
            "Long query ({}), increasing vector weight: vector={:.2}, bm25={:.2}",
            term_count, vector_weight, bm25_weight
        );
    }

    // 2. Check for language-specific hints
    if !query_analysis.language_hints.is_empty() {
        for lang in &query_analysis.language_hints {
            match lang.as_str() {
                "go" | "golang" => {
                    // For Go queries, improve accuracy by using a more balanced approach
                    // with slightly higher vector weight than before
                    vector_weight = 0.5; // Previously was vector_weight * 0.9 (about 0.45)
                    bm25_weight = 0.5; // Previously was bm25_weight * 1.1 (about 0.55)
                    debug!("Detected Go language in query, using balanced weights: vector={:.2}, bm25={:.2}",
                          vector_weight, bm25_weight);
                }
                "rust" => {
                    // For Rust, balanced weights work well
                    vector_weight = 0.6;
                    bm25_weight = 0.4;
                    debug!(
                        "Detected Rust language, adjusted weights: vector={:.2}, bm25={:.2}",
                        vector_weight, bm25_weight
                    );
                }
                "ruby" | "rails" => {
                    // For Ruby queries, slightly increase vector weight
                    vector_weight = (vector_weight * 1.1).min(0.75);
                    bm25_weight = (bm25_weight * 0.9).max(0.25);
                    debug!(
                        "Detected Ruby language, adjusted weights: vector={:.2}, bm25={:.2}",
                        vector_weight, bm25_weight
                    );
                }
                _ => {}
            }
        }
    }

    // 3. Check for code-specific patterns that benefit from lexical search
    let code_patterns = [
        "fn ",
        "pub fn",
        "func ",
        "function ",
        "def ",
        "class ",
        "struct ",
        "enum ",
        "trait ",
        "impl ",
        "interface ",
        "#[",
        "import ",
        "require ",
    ];

    let contains_code_patterns = code_patterns
        .iter()
        .any(|&pattern| query_lower.contains(pattern));

    if contains_code_patterns {
        // Code patterns benefit from stronger lexical matching
        vector_weight = (vector_weight * 0.85).max(0.3);
        bm25_weight = (bm25_weight * 1.15).min(0.7);
        debug!(
            "Query contains code patterns, adjusting weights: vector={:.2}, bm25={:.2}",
            vector_weight, bm25_weight
        );
    }

    // 4. Query type-based adjustments
    match query_analysis.query_type {
        QueryType::Function | QueryType::Type => {
            // Code structural queries often need stronger BM25 matching
            vector_weight = (vector_weight * 0.9).max(0.3);
            bm25_weight = (bm25_weight * 1.1).min(0.7);
            debug!(
                "Function/Type query detected, adjusting weights: vector={:.2}, bm25={:.2}",
                vector_weight, bm25_weight
            );
        }
        QueryType::Usage => {
            // Usage examples might be better found with semantic search
            vector_weight = (vector_weight * 1.1).min(0.8);
            bm25_weight = (bm25_weight * 0.9).max(0.2);
            debug!(
                "Usage query detected, adjusting weights: vector={:.2}, bm25={:.2}",
                vector_weight, bm25_weight
            );
        }
        QueryType::Definition => {
            // Definitions benefit from balanced approach
            vector_weight = 0.55;
            bm25_weight = 0.45;
            debug!(
                "Definition query detected, using balanced weights: vector={:.2}, bm25={:.2}",
                vector_weight, bm25_weight
            );
        }
        QueryType::Implementation => {
            // Implementation queries benefit from more lexical search
            vector_weight = 0.45;
            bm25_weight = 0.55;
            debug!("Implementation query detected, increasing BM25 weight: vector={:.2}, bm25={:.2}",
                  vector_weight, bm25_weight);
        }
        _ => {}
    }

    // Ensure weights sum to 1.0
    let total = vector_weight + bm25_weight;
    vector_weight = vector_weight / total;
    bm25_weight = bm25_weight / total;

    debug!(
        "Final weights: vector={:.2}, bm25={:.2}",
        vector_weight, bm25_weight
    );
    (vector_weight, bm25_weight)
} 