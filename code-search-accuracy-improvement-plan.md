# Code Search Accuracy Improvement Project Plan - Hybrid Regex + Tree-sitter Architecture
**Target: BGE-Large-EN-v1.5 + Hybrid Parsing on 8GB VRAM → Jetson Thor Migration**

## **🎯 Project Goals**

### **Primary Objectives:**
- **150-200% improvement** in search accuracy over current st-codesearch-distilroberta-base
- **Sub-150ms search response time** with regex-optimized preprocessing
- **3-5x faster tokenization** through regex-based implementation
- **20-30% faster overall indexing** with hybrid parsing approach
- **Seamless migration path** to Jetson Thor for production deployment

### **Success Metrics:**
- Search relevance score > 90% on test queries
- Memory usage < 6.5GB during peak operations  
- Indexing throughput > 1500 chunks/minute (vs 1000 in previous plan)
- Tokenization speed > 50,000 tokens/second
- Zero GPU OOM errors during normal operation

---

## **📋 Phase 1: Hybrid Architecture Foundation (Week 1-2)**

### **Milestone 1.1: Regex-Based Fast Tokenizer**
**Tasks:**
- Build regex-based tokenizer to replace current implementation
- Implement camelCase/snake_case splitting with compiled regex patterns
- Add programming keyword extraction with weighted importance
- Create language-specific tokenization patterns (Rust, Python, JS, etc.)
- Add fast identifier compound splitting and normalization
- Benchmark against current tokenizer for 5-10x speedup validation

**Deliverables:**
- High-performance regex tokenizer
- Language-specific pattern databases
- Tokenization benchmarking suite

**Acceptance Criteria:**
- 5-10x faster tokenization than current implementation
- Handles compound identifiers correctly (getUserName → get, user, name)
- Language-specific keywords properly weighted
- Memory usage lower than current tokenizer

### **Milestone 1.2: BGE-Large-EN-v1.5 Integration**
**Tasks:**
- Optimize for 512-token context window with regex preprocessing
- Implement memory monitoring and auto-batch adjustment
- Test embedding generation with new tokenizer integration

**Deliverables:**
- Updated embedding provider with BGE-Large-EN-v1.5
- Integrated regex tokenizer + embedding pipeline
- Memory usage baseline with new architecture

**Acceptance Criteria:**
- Model loads in <5 seconds with <2GB VRAM footprint
- Tokenizer + embedding pipeline 20% faster overall
- Memory usage optimized for hybrid approach

---

## **📋 Phase 2: Hybrid Parsing System (Week 3-4)**

### **Milestone 2.1: Regex Preprocessing Layer**
**Tasks:**
- Build fast regex preprocessing for code pattern extraction
- Implement function/method name extraction with regex
- Add import/export statement parsing
- Create comment and string literal identification
- Build code structure recognition (classes, interfaces, etc.)
- Add programming language detection and classification

**Deliverables:**
- Regex preprocessing engine
- Code pattern extraction system
- Language detection module

**Acceptance Criteria:**
- Preprocessing completes in <2ms for typical files
- 95%+ accuracy on function/method name extraction
- Correct identification of comments vs code
- Language detection accuracy >98%

### **Milestone 2.2: Enhanced Tree-sitter Integration**
**Tasks:**
- Modify existing tree-sitter chunking to use regex preprocessing
- Enhance chunks with regex-extracted metadata
- Implement adaptive parsing (pure regex for simple files)
- Add chunk quality scoring based on both regex and tree-sitter data
- Create hybrid chunking strategy selection logic

**Deliverables:**
- Hybrid chunking system
- Adaptive parsing logic
- Enhanced chunk metadata

**Acceptance Criteria:**
- 20-30% faster chunking through hybrid approach
- Chunks contain rich metadata from both parsing methods
- Simple files processed with regex-only path
- Complex files get full tree-sitter + regex treatment

---

## **📋 Phase 3: Advanced TF-IDF + Search (Week 5-6)**

### **Milestone 3.1: TF-IDF with Regex-Enhanced Terms**
**Tasks:**
- Implement document frequency tracking with regex-extracted terms
- Build TF-IDF calculation using enhanced tokenization
- Create term importance weighting based on code patterns
- Add programming construct importance boosting (function names, classes)
- Implement vocabulary manager with regex pattern-based grouping

**Deliverables:**
- Enhanced TF-IDF system
- Pattern-aware term weighting
- Optimized vocabulary manager

**Acceptance Criteria:**
- TF-IDF vectors include programming construct importance
- Function names and classes get higher term weights
- Vocabulary efficiently groups related programming terms
- Sparse vectors show improved precision on code queries

### **Milestone 3.2: Hybrid Dense + Sparse Retrieval**
**Tasks:**
- Design dual-vector Qdrant schema for enhanced vectors
- Implement query preprocessing with regex pattern detection
- Build query-adaptive dense/sparse weighting
- Add regex-based query expansion (synonyms, case variations)
- Create RRF fusion with pattern-aware ranking

**Deliverables:**
- Advanced hybrid retrieval system
- Query pattern detection
- Intelligent query expansion

**Acceptance Criteria:**
- Queries automatically expanded with programming synonyms
- Query type detection drives retrieval strategy
- Pattern-aware ranking improves relevance scores
- Combined approach outperforms either method alone

---

## **📋 Phase 4: Performance Optimization (Week 7)**

### **Milestone 4.1: Regex-Optimized Pipeline**
**Tasks:**
- Optimize batch processing with regex preprocessing
- Implement streaming regex processing for large files
- Add regex pattern compilation caching
- Create memory-efficient regex pattern matching
- Build concurrent regex processing for parallel files

**Deliverables:**
- Optimized processing pipeline
- Streaming regex processor
- Pattern compilation cache

**Acceptance Criteria:**
- Regex preprocessing adds <5% overhead to total processing time
- Memory usage stays flat during regex processing
- Pattern compilation cached for repeated use
- Concurrent processing scales with available CPU cores

### **Milestone 4.2: Production Performance Tuning**
**Tasks:**
- Implement optimal batch sizing for hybrid approach
- Add dynamic parsing strategy selection
- Create performance monitoring for regex vs tree-sitter paths
- Optimize Qdrant operations with enhanced metadata
- Build auto-scaling batch sizes based on file complexity

**Deliverables:**
- Auto-tuning performance system
- Dynamic strategy selection
- Performance monitoring dashboard

**Acceptance Criteria:**
- System automatically chooses optimal parsing strategy
- Batch sizes adapt to content complexity
- Performance monitoring shows regex optimization gains
- Peak memory usage <6.5GB maintained

---

## **📋 Phase 5: Advanced Search Features (Week 8)**

### **Milestone 5.1: Regex-Powered Query Enhancement**
**Tasks:**
- Build programming pattern query expansion
- Implement regex-based query normalization
- Add code snippet query understanding
- Create API/framework name resolution
- Build context-aware query preprocessing

**Deliverables:**
- Advanced query processing system
- Programming pattern recognition
- Code snippet query handler

**Acceptance Criteria:**
- Code snippet queries (partial functions) handled correctly
- Framework/library names automatically expanded
- Query normalization improves match rates
- Context-aware preprocessing boosts relevance

### **Milestone 5.2: Enhanced Result Ranking**
**Tasks:**
- Implement regex pattern-based result scoring
- Add code complexity analysis using regex metrics
- Create file type and language importance weighting
- Build result diversity with pattern-aware grouping
- Add freshness scoring with git metadata integration

**Deliverables:**
- Pattern-aware ranking system
- Code complexity analysis
- Multi-factor result scoring

**Acceptance Criteria:**
- Results ranked by code pattern importance
- Complex functions ranked higher than simple variables
- Result diversity maintained across different code patterns
- Recent changes get appropriate freshness boost

---

## **📋 Phase 6: Testing & Production (Week 9-10)**

### **Milestone 6.1: Comprehensive Testing**
**Tasks:**
- Create test dataset with regex pattern coverage
- Build automated relevance testing for programming queries
- Implement performance regression testing
- Add stress testing for regex processing limits
- Create A/B testing framework for hybrid vs single approach

**Deliverables:**
- Comprehensive test suite
- Automated quality measurement
- Performance regression tests

**Acceptance Criteria:**
- Test queries achieve >90% relevance (vs 85% in tree-sitter-only plan)
- Performance tests show expected regex speedup gains
- Stress tests validate memory constraints maintained
- A/B tests prove hybrid approach superiority

### **Milestone 6.2: Production Deployment**
**Tasks:**
- Add production-ready error handling for regex failures
- Implement fallback mechanisms (tree-sitter-only mode)
- Create configuration management for regex patterns
- Build monitoring for regex performance metrics
- Prepare Jetson Thor migration with regex optimizations

**Deliverables:**
- Production-ready hybrid system
- Complete deployment documentation
- Thor migration plan

**Acceptance Criteria:**
- System handles regex compilation failures gracefully
- Fallback modes ensure service availability
- All regex patterns externally configurable
- Thor migration plan accounts for regex optimizations

---

## **⚙️ Technical Architecture**

### **Hybrid Processing Pipeline:**
```
Input Code File
    ↓
[Regex Preprocessing] (1-2ms)
    ├── Pattern Detection
    ├── Language Classification  
    ├── Fast Metadata Extraction
    └── Complexity Assessment
    ↓
[Strategy Selection] (<1ms)
    ├── Simple File → Pure Regex Path
    └── Complex File → Hybrid Path
    ↓
[Tokenization] (Regex-based, 5-10x faster)
    ├── Compound Identifier Splitting
    ├── Keyword Recognition
    └── Pattern-based Weighting
    ↓
[Semantic Chunking] (Tree-sitter + Regex Metadata)
    ├── Boundary Detection (Tree-sitter)
    ├── Metadata Enhancement (Regex)
    └── Quality Scoring (Both)
    ↓
[Embedding Generation] (BGE-Large-EN-v1.5)
    ↓
[TF-IDF Enhancement] (Pattern-aware)
    ↓
[Vector Storage] (Qdrant Dual Schema)
```

### **Performance Targets:**
```
Memory Usage:      < 6.5GB peak (8GB VRAM constraint)
Search Latency:    < 150ms average (vs 200ms tree-sitter only)  
Index Throughput:  > 1500 chunks/minute (vs 1000 baseline)
Tokenization:      > 50,000 tokens/second (5-10x improvement)
Regex Preprocessing: < 2ms per file
Pattern Compilation: Cached, <1ms lookup
```

---

## **🧪 Testing Strategy**

### **Performance Testing:**
- Regex vs tree-sitter tokenization speed comparison
- Memory usage profiling for hybrid approach
- Throughput testing on large codebases
- Concurrent processing scalability testing

### **Accuracy Testing:**
- Programming pattern recognition accuracy
- Search relevance on code-specific queries
- Language detection accuracy across 10+ languages
- Query expansion effectiveness measurement

### **Integration Testing:**
- End-to-end hybrid pipeline testing
- Fallback mechanism validation
- Error handling for malformed regex patterns
- Memory constraint compliance under load

---

## **⚠️ Risk Mitigation**

### **High Risk - Regex Complexity:**
- **Risk:** Complex regex patterns cause performance degradation
- **Mitigation:** Pattern compilation caching, complexity limits, fallback to tree-sitter

### **Medium Risk - Pattern Maintenance:**
- **Risk:** Language-specific regex patterns require constant updates
- **Mitigation:** External pattern configuration, automated pattern testing

### **Low Risk - Memory Overhead:**
- **Risk:** Regex pattern compilation increases memory usage
- **Mitigation:** Pattern sharing, lazy compilation, memory monitoring

---

## **📅 Enhanced Timeline**

| Phase | Duration | Key Deliverable | Success Metric |
|-------|----------|----------------|----------------|
| **Phase 1** | Week 1-2 | Regex tokenizer + BGE-Large-EN | 70% accuracy + 5x tokenization speed |
| **Phase 2** | Week 3-4 | Hybrid parsing system | 100% accuracy + 25% indexing speed |
| **Phase 3** | Week 5-6 | Enhanced TF-IDF + retrieval | 150% accuracy improvement |
| **Phase 4** | Week 7 | Performance optimization | 1500+ chunks/minute throughput |
| **Phase 5** | Week 8 | Advanced search features | 180% accuracy improvement |
| **Phase 6** | Week 9-10 | Testing + production | 200% accuracy + production ready |

**Total Project Duration: 10 weeks**

---

## **🎯 Enhanced Success Criteria**

### **Quantitative Goals:**
- **Search Accuracy:** >90% relevance on test queries (vs ~45% current)
- **Performance:** <150ms average search time (25% improvement over tree-sitter only)
- **Tokenization Speed:** 5-10x faster than current implementation  
- **Indexing Throughput:** >1500 chunks/minute (50% improvement)
- **Memory Efficiency:** <80% of 8GB VRAM usage maintained

### **Qualitative Goals:**
- Reliable hybrid parsing with graceful fallbacks
- Language-agnostic pattern recognition system
- Maintainable regex pattern configuration
- Production-ready error handling and monitoring

**Project Success = All quantitative targets exceeded + qualitative deliverables complete + regex optimization proven + seamless Thor migration capability**

**Expected Total Improvement: 200% better search accuracy with 25-50% better performance through regex optimizations**
