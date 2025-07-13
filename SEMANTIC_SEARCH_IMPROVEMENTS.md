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

### **Phase 1: Fix MCP Output Format** ✅ **COMPLETED**
1. **Add missing fields to `SearchResultItem`**: ✅
   - Add `elementType: String` field  
   - Add `language: String` field
   - Update MCP query handler to populate these fields from payload data

2. **Standardize preview generation**: ✅
   - Create shared preview generation logic
   - Ensure consistent truncation behavior across CLI and MCP

### **Phase 2: Fix Deduplication Strategy** 🔄 **PENDING**
1. **Move deduplication to indexing time**:
   - Assign unique IDs to chunks during parsing/indexing
   - Prevent duplicate chunks from being indexed in the first place
   
2. **Improve chunk ID strategy**:
   - Use content hash + metadata for unique chunk identification
   - Consider semantic boundaries when creating chunk IDs

3. **Fix runtime deduplication**:
   - If keeping runtime deduplication, include `elementType` in the deduplication key
   - Consider score-based deduplication (keep highest scoring result for same location)

### **Phase 3: Investigate Chunking Overlaps** 🔄 **PENDING**
1. **Audit language parsers**:
   - Check if language-specific parsers create overlapping chunks
   - Ensure semantic element boundaries don't cause duplicates

2. **Add chunk debugging**:
   - Add logging to show chunk boundaries during indexing
   - Create tool to visualize chunk overlaps for debugging

### **Phase 4: Output Quality Improvements** 🔄 **PENDING**
1. **Add Type information prominently**:
   - Show `Type: function|class|struct|etc.` in human-readable output
   - Consider color-coding different element types

2. **Improve relevance display**:
   - Show context about why this result matched
   - Better formatting for different element types

## **Specific Files Modified:**
- ✅ `crates/sagitta-mcp/src/mcp/types.rs` - Added elementType and language fields to SearchResultItem
- ✅ `crates/sagitta-mcp/src/handlers/query.rs` - Updated to populate new fields from payload data
- 🔄 `crates/sagitta-search/src/search_impl.rs` - Fix deduplication logic (Phase 2)
- 🔄 `crates/sagitta-search/src/indexing.rs` - Add chunk uniqueness validation (Phase 2)
- 🔄 `crates/code-parsers/src/*.rs` - Audit for overlapping chunks (Phase 3)

## **Next Steps:**
- Test Phase 1 changes after restart
- Proceed with Phase 2 if Phase 1 works correctly
- Continue with remaining phases based on testing results