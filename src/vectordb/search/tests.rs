use crate::vectordb::{
    cache::EmbeddingCache,
    db::VectorDB,
    embedding::{EmbeddingModel, EmbeddingModelType},
    hnsw::{HNSWConfig, HNSWIndex},
    search::{ 
        bm25::{build_bm25_index, search_bm25_top_k},
        vector::search_with_limit,
    },
    snippet_extractor::SnippetExtractor,
};
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;
use anyhow;

// --- Mock Embedding Model --- 
#[derive(Clone)]
struct MockEmbeddingModel {
    embeddings: HashMap<String, Vec<f32>>,
    dimension: usize,
}

impl MockEmbeddingModel {
    fn new(dimension: usize) -> Self {
        MockEmbeddingModel { embeddings: HashMap::new(), dimension }
    }

    fn add_embedding(&mut self, text: &str, embedding: Vec<f32>) {
        assert_eq!(embedding.len(), self.dimension, "Mock embedding dimension mismatch");
        self.embeddings.insert(text.to_string(), embedding);
    }
}

impl crate::vectordb::provider::EmbeddingProvider for MockEmbeddingModel {
    fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.embeddings.get(text)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Mock embedding not found for query: {}", text))
    }

    fn embed_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        texts.iter().map(|text| self.embed(text)).collect()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

// --- Mock VectorDB Setup ---
// Function to setup mock db with embeddings and *content*
// Now accepts embeddings map directly
fn setup_mock_db_with_content(
    content_map: HashMap<String, String>, 
    embeddings: HashMap<String, Vec<f32>>, // Accept embeddings
    dimension: usize, 
    use_hnsw: bool
) -> (VectorDB, tempfile::TempDir, String) // Return db_path string
{
    let temp_dir = tempdir().expect("Failed to create temp dir for mock db");
    let db_path = temp_dir.path().join("mock_db.json").to_string_lossy().to_string();
    let cache_path = temp_dir.path().join("cache.json").to_string_lossy().to_string();
    let cache = EmbeddingCache::new(cache_path).unwrap();

    // Create files, but use provided embeddings
    let mut final_embeddings = HashMap::new();
    for (file_name, content) in &content_map {
        let file_path = temp_dir.path().join(file_name);
        fs::write(&file_path, content).expect("Failed to write mock content file");
        let path_str = file_path.to_string_lossy().into_owned();
        // Use the embedding provided for the original file name key
        if let Some(embedding) = embeddings.get(file_name) { // Look up by original filename
             final_embeddings.insert(path_str, embedding.clone());
        } else {
            // Optionally handle cases where embedding is missing for a file
            println!("Warning: No embedding provided for file: {}", file_name);
        }
    }

    let hnsw_index_opt = if use_hnsw && !final_embeddings.is_empty() {
        let mut hnsw_index = HNSWIndex::new(HNSWConfig::new(dimension));
        // Use final_embeddings for HNSW build
        let mut sorted_paths: Vec<String> = final_embeddings.keys().cloned().collect();
        sorted_paths.sort();
        for path in &sorted_paths {
            if let Some(embedding) = final_embeddings.get(path) {
                hnsw_index.insert(embedding.clone()).unwrap();
            }
        }
        Some(hnsw_index)
    } else {
        None
    };

    let db = VectorDB::new_test(
        db_path.clone(), 
        final_embeddings, // Use the map with full paths and correct embeddings
        cache,
        hnsw_index_opt,
        EmbeddingModelType::Onnx,
    );
    (db, temp_dir, db_path)
}

// --- Tests --- 

#[test]
fn test_vector_search_empty_query() {
    let dim = 4;
    let content_map: HashMap<String, String> = HashMap::new(); 
    let embeddings: HashMap<String, Vec<f32>> = HashMap::new();
    let (db, _temp_dir, _) = setup_mock_db_with_content(content_map, embeddings, dim, true);
    let mut model = EmbeddingModel::new_mock(Box::new(MockEmbeddingModel::new(dim)));
    let mut snippet_extractor = SnippetExtractor::new();
    let results = search_with_limit(&db, &mut model, &mut snippet_extractor, "", 10).unwrap();
    assert!(results.is_empty(), "Empty query should return empty results");
}

#[test]
fn test_vector_search_hnsw_path() {
    let dim = 4;
    let mut mock_provider = MockEmbeddingModel::new(dim);
    mock_provider.add_embedding("query1", vec![1.0, 0.0, 0.0, 0.0]);
    mock_provider.add_embedding("query2", vec![0.0, 0.0, 1.0, 0.0]);

    // Define content and corresponding embeddings explicitly
    let mut content_map = HashMap::new();
    content_map.insert("file1.txt".to_string(), "content1".to_string());
    content_map.insert("file2.txt".to_string(), "content2".to_string());
    content_map.insert("file3.txt".to_string(), "content3".to_string());
    content_map.insert("file4.txt".to_string(), "content4".to_string());

    let mut embeddings = HashMap::new();
    embeddings.insert("file1.txt".to_string(), vec![0.9, 0.1, 0.0, 0.0]); // High sim query1
    embeddings.insert("file2.txt".to_string(), vec![0.0, 0.0, 0.8, 0.2]); // High sim query2
    embeddings.insert("file3.txt".to_string(), vec![0.6, 0.1, 0.3, 0.0]); // Med sim query1
    embeddings.insert("file4.txt".to_string(), vec![0.1, 0.2, 0.1, 0.6]); // Low sim both

    let (db, _temp_dir, _) = setup_mock_db_with_content(content_map, embeddings, dim, true); // Use HNSW
    let mut model = EmbeddingModel::new_mock(Box::new(mock_provider));
    let mut snippet_extractor = SnippetExtractor::new();

    let results1 = search_with_limit(&db, &mut model, &mut snippet_extractor, "query1", 10).unwrap();
    assert!(!results1.is_empty(), "Should find results for query1");
    assert!(results1[0].file_path.ends_with("file1.txt"), "Top result for query1 should be file1.txt"); 
    assert!(results1[0].similarity > 0.8, "Similarity for file1.txt should be high"); // Check similarity
    // Check the next result is file3
    if results1.len() > 1 {
        assert!(results1[1].file_path.ends_with("file3.txt"), "Second result for query1 should be file3.txt");
        assert!(results1[1].similarity < results1[0].similarity, "Result 2 sim should be < Result 1 sim");
    }
    assert!(!results1.iter().any(|r| r.file_path.ends_with("file4.txt") && r.similarity > 0.3), "file4.txt should have low similarity");

    let results2 = search_with_limit(&db, &mut model, &mut snippet_extractor, "query2", 10).unwrap();
    assert!(!results2.is_empty(), "Should find results for query2");
    assert!(results2[0].file_path.ends_with("file2.txt"), "Top result for query2 should be file2.txt");
    assert!(results2[0].similarity > 0.7, "Similarity for file2.txt should be high");
}

#[test]
fn test_vector_search_brute_force_path() {
    let dim = 4;
    let mut mock_provider = MockEmbeddingModel::new(dim);
    mock_provider.add_embedding("query1", vec![1.0, 0.0, 0.0, 0.0]);

    let mut content_map = HashMap::new();
    content_map.insert("bf_file1.txt".to_string(), "content bf1".to_string());
    content_map.insert("bf_file2.txt".to_string(), "content bf2".to_string());

    let mut embeddings = HashMap::new();
    embeddings.insert("bf_file1.txt".to_string(), vec![0.9, 0.1, 0.0, 0.0]); // High sim
    embeddings.insert("bf_file2.txt".to_string(), vec![0.1, 0.1, 0.9, 0.0]); // Low sim

    // Setup DB *without* HNSW, pass explicit embeddings
    let (db, _temp_dir, _) = setup_mock_db_with_content(content_map, embeddings, dim, false); 
    assert!(db.hnsw_index.is_none(), "HNSW index should be None for brute force test");

    let mut model = EmbeddingModel::new_mock(Box::new(mock_provider));
    let mut snippet_extractor = SnippetExtractor::new();

    let results = search_with_limit(&db, &mut model, &mut snippet_extractor, "query1", 10).unwrap();
    assert!(!results.is_empty(), "Brute force should find results");
    assert!(results[0].file_path.ends_with("bf_file1.txt"), "Top result should be bf_file1.txt");
    assert!(results[0].similarity > 0.8, "Similarity for bf_file1.txt should be high");
    // Check if low similarity result is present but ranked lower (threshold might filter it)
    if results.len() > 1 {
        assert!(results.iter().any(|r| r.file_path.ends_with("bf_file2.txt")), "bf_file2.txt should be present if not filtered");
        assert!(results.iter().find(|r| r.file_path.ends_with("bf_file2.txt")).unwrap().similarity < 0.3, "bf_file2.txt should have low similarity");
    }
}

#[test]
fn test_vector_search_max_results_limit() {
    let dim = 2;
    let mut mock_provider = MockEmbeddingModel::new(dim);
    mock_provider.add_embedding("query", vec![1.0, 0.0]);

    let mut content_map = HashMap::new();
    let mut embeddings = HashMap::new();
    for i in 0..5 {
        let filename = format!("limit_file_{}.txt", i);
        content_map.insert(filename.clone(), format!("content {}", i));
        // Vary similarity slightly
        let sim = 0.9 - (i as f32 * 0.1);
        embeddings.insert(filename, vec![sim, (1.0 - sim*sim).sqrt()]);
    }

    // Pass embeddings to setup
    let (db, _temp_dir, _) = setup_mock_db_with_content(content_map, embeddings, dim, true); 
    let mut model = EmbeddingModel::new_mock(Box::new(mock_provider));
    let mut snippet_extractor = SnippetExtractor::new();

    let limit = 3;
    let results = search_with_limit(&db, &mut model, &mut snippet_extractor, "query", limit).unwrap();
    assert_eq!(results.len(), limit, "Number of results should be equal to the limit");
    // Verify descending order (highest similarity first)
    assert!(results[0].similarity > results[1].similarity);
    assert!(results[1].similarity > results[2].similarity);
}

#[test]
fn test_vector_search_similarity_threshold() {
    let dim = 2;
    let mut mock_provider = MockEmbeddingModel::new(dim);
    mock_provider.add_embedding("query", vec![1.0, 0.0]);

    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("thresh_db.json").to_string_lossy().to_string();
    let cache_path = temp_dir.path().join("thresh_cache.json").to_string_lossy().to_string();
    let cache = EmbeddingCache::new(cache_path).unwrap();
    let mut embeddings: HashMap<String, Vec<f32>> = HashMap::new();
    embeddings.insert("high_sim.txt".to_string(), vec![0.9, 0.435]);
    embeddings.insert("low_sim.txt".to_string(), vec![0.1, 0.995]);
    embeddings.insert("medium_sim.txt".to_string(), vec![0.5, 0.866]);
    
    let db = VectorDB::new_test(
        db_path, 
        embeddings, 
        cache, 
        None,
        EmbeddingModelType::Onnx
    );

    let mut model = EmbeddingModel::new_mock(Box::new(mock_provider));
    let mut snippet_extractor = SnippetExtractor::new();

    let results = search_with_limit(&db, &mut model, &mut snippet_extractor, "query", 10).unwrap();
    
    assert!(results.iter().any(|r| r.file_path == "high_sim.txt"), "High similarity file should be present");
    assert!(results.iter().any(|r| r.file_path == "medium_sim.txt"), "Medium similarity file should be present");
    assert!(!results.iter().any(|r| r.file_path == "low_sim.txt"), "Low similarity file should be filtered out by threshold");
    assert_eq!(results.len(), 2, "Only results above threshold should remain");
}

// --- BM25 Tests --- 

#[test]
fn test_bm25_index_building() {
    let mut content_map = HashMap::new();
    content_map.insert("doc1.txt".to_string(), "the quick brown fox".to_string());
    content_map.insert("doc2.txt".to_string(), "jumps over the lazy fox".to_string());
    content_map.insert("doc3.txt".to_string(), "the lazy dog".to_string());

    // Pass an empty embeddings map
    let empty_embeddings: HashMap<String, Vec<f32>> = HashMap::new();
    let (_db, _temp_dir, db_path) = setup_mock_db_with_content(content_map.clone(), empty_embeddings.clone(), 4, false); 
    
    let temp_path = _temp_dir.path();
    let adjusted_embeddings = content_map.keys().map(|fname| {
        let full_path = temp_path.join(fname).to_string_lossy().into_owned();
        (full_path, vec![0.0; 4])
    }).collect();
    let cache_path = temp_path.join("bm25_cache.json").to_string_lossy().to_string();
    let cache = EmbeddingCache::new(cache_path).unwrap();
    let db_for_bm25 = VectorDB::new_test(
        db_path, 
        adjusted_embeddings,
        cache,
        None,
        EmbeddingModelType::Onnx,
    );

    let bm25_result = build_bm25_index(&db_for_bm25);
    assert!(bm25_result.is_ok(), "BM25 index build failed: {:?}", bm25_result.err());
    let bm25_index = bm25_result.unwrap();

    assert_eq!(bm25_index.total_docs, 3);
    assert_eq!(bm25_index.doc_data.len(), 3);
    assert!(bm25_index.avg_doc_length > 0.0);
    assert!(bm25_index.idf.contains_key("fox"));
    assert!(bm25_index.idf.contains_key("lazy"));
    assert!(bm25_index.idf.contains_key("the"));
    assert!(bm25_index.idf["quick"] > bm25_index.idf["fox"]);
    assert!(bm25_index.idf["dog"] > bm25_index.idf["lazy"]);
    assert!(bm25_index.idf["lazy"] > bm25_index.idf["the"]); 
}

#[test]
fn test_bm25_search() {
    let mut content_map = HashMap::new();
    content_map.insert("bm_doc1.txt".to_string(), "search algorithms are fun".to_string());
    content_map.insert("bm_doc2.txt".to_string(), "fun search index test".to_string());
    content_map.insert("bm_doc3.txt".to_string(), "another test document".to_string());

    // Pass an empty embeddings map
    let empty_embeddings: HashMap<String, Vec<f32>> = HashMap::new();
    let (_db, _temp_dir, db_path) = setup_mock_db_with_content(content_map.clone(), empty_embeddings.clone(), 4, false);
    let temp_path = _temp_dir.path();
    let adjusted_embeddings = content_map.keys().map(|fname| {
        (temp_path.join(fname).to_string_lossy().into_owned(), vec![0.0; 4])
    }).collect();
    let cache_path = temp_path.join("bm25_s_cache.json").to_string_lossy().to_string();
    let cache = EmbeddingCache::new(cache_path).unwrap();
    let db_for_bm25 = VectorDB::new_test(
        db_path, adjusted_embeddings, cache, None, EmbeddingModelType::Onnx
    );

    let bm25_index = build_bm25_index(&db_for_bm25).unwrap();

    let results1 = search_bm25_top_k("fun search", &bm25_index, 10).unwrap();
    assert_eq!(results1.len(), 2);
    assert!(results1.iter().any(|(p, _)| p.ends_with("bm_doc1.txt")), "bm_doc1 should be present for 'fun search'");
    assert!(results1.iter().any(|(p, _)| p.ends_with("bm_doc2.txt")), "bm_doc2 should be present for 'fun search'");
    assert!(results1[0].1 >= results1[1].1, "Scores should be non-increasing for 'fun search'"); 

    let results2 = search_bm25_top_k("algorithms", &bm25_index, 10).unwrap();
    assert_eq!(results2.len(), 1);
    assert!(results2[0].0.ends_with("bm_doc1.txt"));

    let results3 = search_bm25_top_k("test", &bm25_index, 10).unwrap();
    assert_eq!(results3.len(), 2);
    assert!(results3.iter().any(|(p, _)| p.ends_with("bm_doc2.txt")), "bm_doc2 should be present for 'test'");
    assert!(results3.iter().any(|(p, _)| p.ends_with("bm_doc3.txt")), "bm_doc3 should be present for 'test'");
    assert!(results3[0].1 >= results3[1].1, "Scores should be non-increasing for 'test'");
    
    let results4 = search_bm25_top_k("", &bm25_index, 10).unwrap();
    assert!(results4.is_empty());

    let results5 = search_bm25_top_k("nonexistent term", &bm25_index, 10).unwrap();
    assert!(results5.is_empty());
}

// TODO: Add test for specialized search threshold 