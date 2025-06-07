# Search Ranking Improvements

## Overview

This document describes the comprehensive improvements made to the search ranking system to address poor relevance ranking, score distribution issues, and tokenization inconsistencies.

## Problems Addressed

### 1. Poor Score Distribution
- **Issue**: Scores capped at 0.5000 with large increments
- **Root Cause**: Basic RRF (Reciprocal Rank Fusion) caps scores at 0.5
- **Solution**: Switched to DBSF (Distribution-Based Score Fusion) for better score distribution

### 2. Tokenization Mismatch
- **Issue**: Index used `split_whitespace()` while search used `tokenize_code()`
- **Root Cause**: Inconsistent tokenization led to vocabulary mismatches
- **Solution**: Both index and search now use the same `tokenize_code()` function

### 3. Poor TF-IDF Scoring
- **Issue**: Raw term frequency counts without normalization
- **Root Cause**: No log normalization or IDF weighting
- **Solution**: Implemented log-normalized TF: `1 + log(freq)`

### 4. Inadequate Filename Boosting
- **Issue**: Poor ranking for filename-specific queries
- **Root Cause**: No special handling for filename matches
- **Solution**: Smart filename detection and boosting

## Key Improvements

### Enhanced Search Configuration

```rust
pub struct SearchConfig {
    pub fusion_method: FusionMethod,        // RRF vs DBSF
    pub dense_prefetch_multiplier: u64,     // Dense vector prefetch ratio
    pub sparse_prefetch_multiplier: u64,    // Sparse vector prefetch ratio
    pub use_tfidf_weights: bool,            // Enable TF-IDF scoring
    pub filename_boost: f32,                // Filename match boost factor
    pub score_threshold: Option<f32>,       // Filter low scores
}
```

### Improved Tokenization Consistency

**Before (Index)**:
```rust
let words: Vec<&str> = chunk.content.split_whitespace().collect();
for word in words {
    let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    // ...
}
```

**After (Index)**:
```rust
let tokenizer_config = crate::tokenizer::TokenizerConfig::default();
let tokens = crate::tokenizer::tokenize_code(&chunk.content, &tokenizer_config);
for token in tokens {
    if matches!(token.kind, TokenKind::Identifier | TokenKind::Literal) {
        // ...
    }
}
```

### Advanced Filename Boosting

The system now intelligently detects filename-related terms:

1. **File Extensions**: `.rs`, `.go`, `.py`, `.js`, `.ts`, etc.
2. **Filename Patterns**: Contains `_`, `-`, `.`, or CamelCase
3. **Context-Aware**: Recognizes filename-related terms in queries
4. **Double Boosting**: Both during indexing and search

### TF-IDF Scoring Improvements

**Before**:
```rust
let sparse_values: Vec<f32> = term_frequencies.values()
    .map(|&count| count as f32)
    .collect();
```

**After**:
```rust
let sparse_values: Vec<f32> = term_frequencies.values()
    .map(|&count| 1.0 + (count as f32).ln())
    .collect();
```

### Fusion Method Upgrade

- **Default**: DBSF (Distribution-Based Score Fusion)
- **Benefits**: Better score distribution, scores beyond 0.5 cap
- **Fallback**: RRF still available for compatibility

## Search Configurations

### Code Search (Optimized for programming files)
```rust
SearchConfig {
    fusion_method: FusionMethod::Dbsf,
    dense_prefetch_multiplier: 4,
    sparse_prefetch_multiplier: 6,
    use_tfidf_weights: true,
    filename_boost: 3.0,               // High boost for code files
    score_threshold: Some(0.1),        // Filter low scores
}
```

### Document Search (Optimized for text documents)
```rust
SearchConfig {
    fusion_method: FusionMethod::Dbsf,
    dense_prefetch_multiplier: 3,
    sparse_prefetch_multiplier: 4,
    use_tfidf_weights: true,
    filename_boost: 1.5,               // Moderate boost
    score_threshold: None,             // Keep all results
}
```

## Expected Results

### For Query: "file manager filename"

**Before**:
- `file_manager.rs` at rank 5+
- Scores capped at 0.5000
- Poor filename recognition

**After**:
- `file_manager.rs` should rank #1 or #2
- Better score distribution (> 0.5)
- Intelligent filename boosting

### General Improvements

1. **Better Relevance**: More accurate ranking based on semantic + lexical similarity
2. **Consistent Tokenization**: Same vocabulary between index and search
3. **Smart Filename Handling**: Automatic detection and boosting of filename matches
4. **Configurable Behavior**: Different strategies for code vs document search
5. **Score Distribution**: DBSF provides better score ranges and discrimination

## Backward Compatibility

- Legacy search function `search_collection_legacy()` maintains old behavior
- Default configuration uses improved settings
- Existing vocabularies are compatible (will be enhanced on re-indexing)

## Usage Examples

### Using Code Search Configuration
```rust
let config = code_search_config();
let results = search_collection(
    client, collection_name, embedding_pool, 
    "file manager filename", limit, filter, app_config, Some(config)
).await?;
```

### Using Custom Configuration
```rust
let config = SearchConfig {
    fusion_method: FusionMethod::Dbsf,
    filename_boost: 5.0,  // Very high filename boost
    score_threshold: Some(0.2),
    ..Default::default()
};
```

## Testing

Comprehensive tests cover:
- Filename boost logic
- Search configuration variations
- Tokenization consistency
- TF-IDF scoring improvements

Run tests:
```bash
cargo test search_impl --lib
cargo test indexing --lib
```

## Migration Notes

For best results after updating:
1. **Re-index repositories** to use improved tokenization
2. **Update search calls** to use new search configurations
3. **Monitor search performance** for ranking improvements

The system maintains backward compatibility, but re-indexing will provide the full benefits of these improvements.

## Implementation Status

### ✅ Completed and Tested

1. **Enhanced Search Configuration System**
   - `SearchConfig` struct with configurable fusion methods, prefetch multipliers, TF-IDF scoring, filename boosting, and score thresholds
   - Specialized configurations for code search and document search
   - Default DBSF fusion method for better score distribution

2. **Improved Tokenization Consistency**
   - Updated indexing to use the same `tokenize_code()` function as search
   - Consistent vocabulary generation between index and search phases
   - Proper filtering of token types (identifiers and literals only)

3. **Advanced TF-IDF Scoring**
   - Log-normalized term frequency: `1 + log(freq)`
   - Consistent scoring between indexing and search
   - Replaced raw frequency counts with proper TF weighting

4. **Smart Filename Boosting**
   - Detection of file extensions (.rs, .go, .py, etc.)
   - Recognition of filename patterns (underscores, dashes, CamelCase)
   - Context-aware boosting for filename-related queries
   - Filename term boosting during both indexing and search

5. **Backward Compatibility**
   - All existing call sites updated to include new search_config parameter
   - Legacy `search_collection_legacy()` function maintains old behavior
   - Default configurations provide improved behavior without breaking changes

6. **Comprehensive Testing**
   - Unit tests for filename boost logic
   - Tests for all search configuration variants
   - Validation of tokenization consistency
   - Verification of TF-IDF improvements

### 🔧 Integration Complete

All crates in the workspace have been updated and compile successfully:
- ✅ `sagitta-search` (core library)
- ✅ `sagitta-cli` (command line interface)
- ✅ `sagitta-mcp` (Model Context Protocol server)
- ✅ `sagitta-code` (GUI application)

### 📊 Expected Performance Improvements

For the specific case mentioned in the original issue:

**Query: "file manager filename"**
- **Before**: `file_manager.rs` ranked at position 5+, scores capped at 0.5000
- **After**: Expected to rank at position 1-2 with DBSF scores > 0.5000

**General Improvements**:
- Better score distribution beyond 0.5 cap
- More accurate ranking for filename-related queries
- Consistent sparse vector matching between index and search
- Improved relevance through proper TF-IDF weighting

### 🚀 Next Steps

To realize the full benefits of these improvements:

1. **Re-index existing repositories** to apply new tokenization and scoring
2. **Use specialized search configs** for code vs document search
3. **Monitor search performance** to validate ranking improvements
4. **Consider additional tuning** of filename boost factors based on usage patterns 