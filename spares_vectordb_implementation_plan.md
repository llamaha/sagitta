# Sparse Vector (TF-IDF) Integration Plan

This plan outlines the steps to integrate sparse vectors using Term Frequency (TF) vectors into the existing hybrid search functionality, leveraging Qdrant's built-in Inverse Document Frequency (IDF) calculation (`Modifier::Idf`).

**Note:** Throughout all phases, ensure tests are run frequently after incremental changes. Commit working changes regularly with descriptive messages.

## Phase 1: Setup & Vocabulary `[DONE]`

**Goal:** Establish the foundational components for handling code terms and configure Qdrant correctly.

1.  **Task 1.1: Implement Code Tokenizer:** `[DONE]`
    *   Develop or adapt a tokenizer specifically for Rust code.
    *   It should handle identifiers, keywords, symbols, comments, and strings appropriately.
    *   Consider edge cases and normalization (e.g., lowercasing).
    *   *Output:* A function `fn tokenize_code(code: &str) -> Vec<String>`.
2.  **Task 1.2: Implement Vocabulary Building & Persistence:** `[DONE]`
    *   Create a struct/module (e.g., `VocabularyManager`) to map unique tokens to `u32` IDs.
    *   Use a `HashMap<String, u32>` internally and potentially a `Vec<String>` for reverse lookups if needed.
    *   Implement methods like `add_token(&mut self, token: &str) -> u32` and `get_id(&self, token: &str) -> Option<u32>`.
    *   Implement persistence (e.g., saving/loading the map/vec to/from a file using `serde`).
    *   This needs to be built/loaded during the indexing process.
3.  **Task 1.3: Update Collection Creation Schema:** `[DONE]`
    *   Modify the Qdrant collection creation logic (likely in `crates/vectordb-core/src/qdrant_client_trait.rs` or setup code).
    *   In the `CreateCollectionBuilder`:
        *   Ensure existing dense vector params are named (e.g., "dense") within `vectors_config`.
        *   Add `.sparse_vectors_config(SparseVectorsConfigBuilder::default().add_named_vector_params("sparse_tf", SparseVectorParamsBuilder::default().modifier(Modifier::Idf)))`.
4.  **Task 1.4: Add Basic Tests:** `[DONE]`
    *   Write unit tests for `tokenize_code`.
    *   Write tests for `VocabularyManager` (add, get, save, load).

## Phase 2: Indexing Pipeline Integration `[WIP]`

**Goal:** Modify the indexing process to generate and store sparse TF vectors alongside dense vectors.

1.  **Task 2.1: Integrate Tokenizer & Vocabulary into Indexing:** `[DONE]`
    *   Inject/use the `VocabularyManager` and `tokenize_code` within the embedding/indexing logic (e.g., `EmbeddingHandler`).
2.  **Task 2.2: Calculate Term Frequencies (TF):** `[DONE]`
    *   For each document, call `tokenize_code`.
    *   Iterate through tokens, get/add IDs using `VocabularyManager`.
    *   Count frequencies of each `token_id` (e.g., using a `HashMap<u32, u32>`).
3.  **Task 2.3: Create Sparse TF Vectors:** `[DONE]`
    *   Convert the TF map into `indices: Vec<u32>` and `values: Vec<f32>` (casting counts to `f32`).
    *   *Result:* `(Vec<u32>, Vec<f32>)` for each document.
4.  **Task 2.4: Update Upsert Logic:** `[DONE]`
    *   Modify the creation of `PointStruct` instances during upsertion.
    *   Use `NamedVectors` to hold both vector types:
        ```rust
        let dense_vector_data = /* ... generate dense vector ... */;
        let (sparse_indices, sparse_values) = /* ... generate sparse TF vector ... */;

        let vectors = NamedVectors::default()
            .add_vector("dense", Vector::new_dense(dense_vector_data))
            .add_vector("sparse_tf", Vector::new_sparse(sparse_indices, sparse_values));

        let point = PointStruct::new(point_id, vectors, payload);
        ```
    *   Upsert using `UpsertPointsBuilder::new("{collection_name}", vec![point])`.
5.  **Task 2.5: Add Indexing Tests:** `[TODO]`
    *   Write tests verifying that `PointStruct`s contain correctly structured `NamedVectors` with both "dense" and "sparse_tf" entries before upsertion.

## Phase 3: Query Pipeline & Hybrid Search `[WIP]`

**Goal:** Update the search functionality to perform hybrid queries using both vector types.

1.  **Task 3.1: Update Search Logic (`search_collection`):** `[DONE]`
    *   Modify the main search function (likely in `crates/vectordb-core/src/search_impl.rs`).
2.  **Task 3.2: Generate Sparse Query Vector:** `[DONE]`
    *   Tokenize the input query text using `tokenize_code`.
    *   Look up token IDs in the loaded `VocabularyManager`. Ignore unknown tokens.
    *   Create `query_indices: Vec<u32>` and `query_values: Vec<f32>`. For each known token ID, add it to `query_indices` and add `1.0f32` to `query_values`.
3.  **Task 3.3: Implement Hybrid Query:** `[DONE]`
    *   Generate the dense query vector as before.
    *   Use `QueryPointsBuilder::new("{collection_name}")`:
        ```rust
        let query_builder = QueryPointsBuilder::new("{collection_name}")
            .add_prefetch(
                PrefetchQueryBuilder::default()
                    .query(Query::new_nearest(dense_query_vector)) // Use actual dense vector
                    .using("dense") // Name of dense vector
                    .limit(limit * 2) // Prefetch more results for fusion
            )
            .add_prefetch(
                PrefetchQueryBuilder::default()
                    .query(Query::new_nearest(
                        // Construct sparse query vector using indices/values from Task 3.2
                        qdrant_client::qdrant::VectorInput::new_sparse(query_indices, query_values) 
                    ))
                    .using("sparse_tf") // Name of sparse vector
                    .limit(limit * 2) // Prefetch more results for fusion
            )
            .query(Query::new_fusion(Fusion::Rrf)) // Use RRF fusion
            .limit(limit); // Apply final limit AFTER fusion

        let results = client.query(query_builder).await?;
        ```
    *   Note: The exact structure for `Query::new_nearest` with sparse might need slight adjustment based on client version specifics (e.g., using `VectorInput::new_sparse`). The example used `[(idx, val)]` slice, but the upsert used `Vector::new_sparse`. `VectorInput` seems more likely for queries. Prefetching more results (`limit * 2`) is recommended before fusion.
4.  **Task 3.4: Add Hybrid Search Integration Tests:** `[TODO]`
    *   Create integration tests that set up a small collection via the updated indexing logic.
    *   Run queries targeting specific keywords present in sparse vectors.
    *   Assert that the correct documents are returned and ranked appropriately by the RRF fusion.

## Phase 4: Refinement & Optimization `[WIP]`

**Goal:** Ensure code quality, performance, and evaluate the effectiveness of the changes.

1.  **Task 4.1: Review Code for Modularity & Size:** `[DONE]`
    *   Ensure changes adhere to code quality standards (modularity, file size limits).
    *   Refactor if necessary (e.g., tokenizer, vocabulary manager).
2.  **Task 4.2: Performance Testing:** `[TODO]`
    *   Benchmark indexing and query performance before and after the changes.
    *   Identify potential bottlenecks (e.g., tokenization, vocabulary lookup, Qdrant query structure).
3.  **Task 4.3: Evaluate Search Relevance:** `[TODO]`
    *   Qualitatively and potentially quantitatively evaluate if the hybrid search provides better results for queries involving specific code items compared to the previous dense-only search.
    *   Consider adjustments to query vector weighting or tokenization if needed.
4.  **Task 4.4: Tokenizer Refinements:** `[TODO]`
    *   Improve regex patterns (e.g., floats, hex/octal numbers, more symbols).
    *   Make filtering/normalization configurable via AppConfig if needed.
5.  **Task 4.5: Vocabulary Path Refinement:** `[DONE]`
    *   Use AppConfig or defaults for vocab path.
    *   Ensure directory creation/error handling is robust.
6.  **Task 4.6: Error Handling:** `[TODO]`
    *   Improve error handling around vocab load/save, indexing failures.
7.  **Task 4.7: Configuration:** `[TODO]`
    *   Expose relevant options (e.g., RRF params, prefetch limits, tokenizer settings) via AppConfig. 