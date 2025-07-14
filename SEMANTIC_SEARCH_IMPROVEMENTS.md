# Semantic Search Output Improvements Plan

## **Current Issues Identified:**

### **1. Missing Information in MCP Output** ✅ **PHASE 1 - COMPLETED**
- **Problem**: MCP `SearchResultItem` only includes `filePath`, `startLine`, `endLine`, `score`, `content`, and `preview`
- **Missing**: `elementType` and `language` fields that are available in the CLI formatter but not exposed via MCP
- **Impact**: Users can't see what type of code element was matched (function, class, etc.) or the programming language

### **2. Deduplication Logic Problems** 
- **Current Logic**: `crates/sagitta-search/src/search_impl.rs:390-420` only deduplicates on `file_path:start_line:end_line`
- **Problem**: This assumes each file location is indexed only once, but:
  - Same content could be indexed with different `element_type` values (e.g., a function that's also part of a class)
  - Language-specific parsers might create overlapping semantic chunks
  - The deduplication happens AFTER search, wasting computation and potentially losing better-scoring results

### **3. Chunking Strategy Issues**
- **Fallback Parser**: Creates non-overlapping 500-line chunks (`crates/code-parsers/src/fallback.rs:6`)
- **Language Parsers**: May create overlapping chunks when semantic elements span boundaries
- **Root Cause**: Each chunk should have a unique identifier, but currently only location-based deduplication exists

### **4. Output Format Inconsistencies**
- **CLI Format**: Shows `Score`, `File`, `Lines`, `Lang`, `Type` - comprehensive information
- **MCP Format**: Missing `elementType` and `language` fields
- **Preview Generation**: Done differently in CLI vs MCP handlers

## **Implementation Progress:**

### **Phase 1: Fix MCP Output Format** ✅ **COMPLETED & TESTED**
1. **Add missing fields to `SearchResultItem`**: ✅
   - Add `elementType: String` field  
   - Add `language: String` field
   - Update MCP query handler to populate these fields from payload data

2. **Standardize preview generation**: ✅
   - Create shared preview generation logic
   - Ensure consistent truncation behavior across CLI and MCP

3. **Testing Validation**: ✅
   - Confirmed both `elementType` and `language` fields are populated correctly
   - Verified precise filtering with `elementType="function"` and `lang="rust"` parameters
   - Successfully demonstrated improved search precision in "Tell me how sagitta-cli works" test query

### **Phase 2: Fix Deduplication Strategy** ✅ **COMPLETED**
1. **✅ Improved runtime deduplication**:
   - Include `element_type` in deduplication key to allow same location with different element types
   - Implemented score-based deduplication to keep highest-scoring result for each unique key
   - Replaced HashSet with HashMap for more sophisticated deduplication logic
   
2. **🔄 Future improvements (optional)**:
   - Move deduplication to indexing time for better performance
   - Use content hash + metadata for unique chunk identification during indexing

### **Phase 3: Investigate Chunking Overlaps** 🔄 **STARTED - ISSUES FOUND**
1. **✅ Audit language parsers**:
   - **FOUND**: Python and JavaScript parsers create overlapping chunks (confirmed by existing test files)
   - **Rust parser**: Well-designed to prevent overlaps - uses semantic boundaries correctly
   - **Fallback parser**: Creates non-overlapping 500-line chunks correctly
   - **Issue confirmed in**: `tests/data/test_overlap_detection_comprehensive.rs` shows Python and JavaScript overlap problems

2. **🔄 Next steps for Phase 3**:
   - Fix Python parser overlap issues in `crates/code-parsers/src/python.rs`
   - Fix JavaScript parser overlap issues in `crates/code-parsers/src/javascript.rs`  
   - Add chunk debugging/validation during indexing
   - Create tool to visualize chunk overlaps for debugging

### **Phase 4: Rich Code Intelligence Previews** 🔄 **PENDING**

**Vision**: Transform semantic search results from simple text snippets into rich code intelligence previews that provide immediate codebase understanding.

**Key Innovation**: Integrate existing repo-mapper regex parsing capabilities to show function calls, return types, dependencies, and bidirectional call graphs.

#### **Sub-Phases:**

1. **Phase 4.1: Enhanced Preview Generation**
   - Reactivate repo-mapper functionality for real-time code analysis
   - Parse function signatures, method calls, and return types for each search result
   - Replace simple text previews with structured code intelligence displays

2. **Phase 4.2: Bidirectional Call Graph Integration**
   - Index function call relationships during chunking
   - Implement reverse lookup: "called by" relationships  
   - Display both outgoing calls and incoming references

3. **Phase 4.3: Type Flow & Import Analysis**
   - Extract and display type signatures: `String → Result<Vec<Item>, Error>`
   - Show import/dependency context for each result
   - Identify data transformation chains and patterns

4. **Phase 4.4: Pattern Recognition & Classification**
   - Detect and tag async/await patterns
   - Identify error handling approaches (Result, Option, exceptions)
   - Classify database operations, I/O patterns, and architectural patterns
   - Add semantic tags to results for better categorization

#### **Expected Transformation:**

**Before** (current):
```
"/// Formats search results for display, handling both JSON and human-readable output."
```

**After** (Phase 4 complete):
```
🔧 format_search_results(results: Vec<SearchResult>, format: OutputFormat) → Result<String, Error>
   📞 Calls: write_json, format_human_readable, truncate_preview
   📞 Called by: main, handle_query, display_results  
   📦 Uses: serde_json, anyhow, clap
   🏷️  Tags: [formatting, output, serialization, error-handling]
   📝 Formats search results for display, handling both JSON and human-readable output
```

**Benefits**: 
- Immediate code understanding with minimal additional requests
- Strong dependency graph visibility
- Pattern recognition for architectural understanding
- Rich context that accelerates development

### **Phase 5: Output Quality Improvements** 🔄 **PENDING**
1. **Add Type information prominently**:
   - Show `Type: function|class|struct|etc.` in human-readable output
   - Consider color-coding different element types

2. **Improve relevance display**:
   - Show context about why this result matched
   - Better formatting for different element types

## **Specific Files Modified:**
- ✅ `crates/sagitta-mcp/src/mcp/types.rs` - Added elementType and language fields to SearchResultItem
- ✅ `crates/sagitta-mcp/src/handlers/query.rs` - Updated to populate new fields from payload data  
- ✅ `crates/sagitta-search/src/search_impl.rs` - Improved deduplication logic with element_type and score-based selection
- 🔄 `crates/sagitta-search/src/indexing.rs` - Add chunk uniqueness validation (Phase 2)
- 🔄 `crates/code-parsers/src/*.rs` - Audit for overlapping chunks (Phase 3)

## **Testing & Validation:**
- ✅ MCP tests pass (84 tests passed, 0 failed)
- ✅ Full compilation successful
- ✅ Git commit created: `0df3bb2` 
- ✅ Repository synced with updated indexing

## **Next Steps:**
- ✅ **Phase 1**: COMPLETED - elementType/language fields working and tested
- ✅ **Phase 2**: COMPLETED - deduplication logic improved
- 🎯 **Phase 3**: Fix Python & JavaScript parser overlap issues - specific files identified 
- 🆕 **Phase 4**: Rich Code Intelligence Previews - integrate repo-mapper for enhanced previews
- 🔄 **Phase 5**: Output quality improvements (pending completion of Phase 3 & 4)

## **Expected User Impact:**
After restart, MCP search results will include:
- `elementType`: Shows code construct type (function, class, struct, etc.)
- `language`: Shows programming language (rust, python, javascript, etc.)
This provides the same rich information available in CLI to MCP users.