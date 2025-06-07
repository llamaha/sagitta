use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use regex::Regex;

use crate::tools::registry::ToolRegistry;
use crate::tools::types::{Tool, ToolDefinition, ToolResult, ToolCategory};
use crate::utils::errors::SagittaCodeError;
// Import Qdrant client trait from sagitta_search
use sagitta_search::qdrant_client_trait::QdrantClientTrait;
// Import Qdrant types needed for search
use qdrant_client::qdrant::{SearchPoints, PointStruct, RecommendPoints, Condition, Filter, FieldCondition, Match, Range, GeoRadius, GeoPoint }; // Added more qdrant types
use qdrant_client::qdrant::{
    CreateCollection, VectorParams, VectorsConfig, Distance, UpsertPoints, GetCollectionInfoRequest,
    vectors_config::Config as VectorsConfigEnum,
    PointId, NamedVectors, Vectors, vectors::VectorsOptions // Added PointStruct, PointId, NamedVectors, Vectors, VectorsOptions to imports
};
use sagitta_embed::provider::{EmbeddingProvider, onnx::OnnxEmbeddingModel};


// #[derive(Debug)] // Removed derive Debug
pub struct AnalyzeInputTool {
    tool_registry: Arc<ToolRegistry>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
    qdrant_client: Arc<dyn QdrantClientTrait>, // Added Qdrant client
}

// Manual Debug implementation for AnalyzeInputTool
impl std::fmt::Debug for AnalyzeInputTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyzeInputTool")
         .field("tool_registry", &self.tool_registry)
         .field("embedding_provider", &"Arc<dyn EmbeddingProvider>")
         // Not including qdrant_client directly as it may not be Debug
         .field("qdrant_client", &"Arc<dyn QdrantClientTrait>") 
         .finish()
    }
}

impl AnalyzeInputTool {
    // Updated constructor
    pub fn new(
        tool_registry: Arc<ToolRegistry>, 
        embedding_provider: Arc<dyn EmbeddingProvider>,
        qdrant_client: Arc<dyn QdrantClientTrait> // Added Qdrant client to constructor
    ) -> Self {
        Self { tool_registry, embedding_provider, qdrant_client }
    }
}

// New output structures
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SemanticAnalysisResult {
    pub original_input: String,
    pub main_goal_description: Option<String>,
    pub extracted_entities: HashMap<String, Value>,
    pub proposed_sub_goals: Vec<ProposedSubGoal>,
    pub confidence: f32,
    pub analysis_message: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ProposedSubGoal {
    pub description: String,
    pub action_type: String, // e.g., "tool_execution", "llm_query"
    pub suggested_tool_name: Option<String>,
    pub inferred_tool_parameters: Option<Value>,
    // pub confidence_per_sub_goal: f32, // Future enhancement
}


// Old AnalysisResult - can be removed or kept if there's a transition period
/*
#[derive(serde::Serialize)]
struct AnalysisResult {
    original_input: String,
    processed_input: String,
    primary_intent: String,
    suggested_tool: Option<String>,
    tool_parameters: Option<Value>,
    confidence: f32,
    analysis_message: String,
}
*/

// New constant for the tools collection name
pub const TOOLS_COLLECTION_NAME: &str = "sagitta_code_tools_collection";
const DEFAULT_STRING_PARAM_KEY: &str = "inferred_text_input"; // Default key for text when no specific param name found

// Helper function for parameter extraction using Regex
fn extract_parameters_regex(input_str: &str, tool_name: &str, schema_val: &Value) -> Result<(Value, Vec<String>), SagittaCodeError> {
    let mut processed_input = input_str.trim().to_string();
    let schema_properties = schema_val.get("properties").and_then(|p| p.as_object());
    let schema_has_properties = schema_properties.map_or(false, |props| !props.is_empty());

    if schema_has_properties && !tool_name.is_empty() && processed_input.to_lowercase().starts_with(&tool_name.to_lowercase()) {
        let tool_name_len = tool_name.len();
        if processed_input.len() == tool_name_len {
            processed_input.clear();
        } else if processed_input.len() > tool_name_len {
            if let Some(char_after) = processed_input.chars().nth(tool_name_len) {
                if !char_after.is_alphanumeric() {
                    processed_input = processed_input[tool_name_len..].trim().to_string();
                }
            } else {
                processed_input = processed_input[tool_name_len..].trim().to_string(); 
            }
        } 
    }

    let mut extracted_params = serde_json::Map::new();
    let mut text_segments_for_default: Vec<String> = vec![processed_input.clone()];
    let mut missing_required_params = Vec::new();

    let required_params_set: std::collections::HashSet<String> = schema_val
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    if let Some(properties) = schema_val.get("properties").and_then(|p| p.as_object()) {
        for (param_name_key, param_schema) in properties {
            let param_name = param_name_key.as_str();
            let param_type_str = param_schema.get("type").and_then(|t| t.as_str());

            let patterns = vec![
                Regex::new(&format!("(?:^|\\s)--{}=(?:\"([^\"]*)\"|'([^']*)'|([^\\s'\"]+))", regex::escape(param_name))).unwrap(),
                Regex::new(&format!("(?:^|\\s)--{}(\\s+)(?:\"([^\"]*)\"|'([^']*)'|([^\\s'\"]+))", regex::escape(param_name))).unwrap(),
                Regex::new(&format!("(?:^|\\b){}=(?:\"([^\"]*)\"|'([^']*)'|([^\\s'\"]+))", regex::escape(param_name))).unwrap(),
                Regex::new(&format!("(?:^|\\b){}:(?:\"([^\"]*)\"|'([^']*)'|([^\\s'\"]+))", regex::escape(param_name))).unwrap(),
            ];

            let mut found_value_str: Option<String> = None;
            let mut param_matched_in_any_segment = false;
            let mut temp_next_segments = Vec::new();

            for segment in text_segments_for_default.iter() {
                if param_matched_in_any_segment {
                    temp_next_segments.push(segment.clone());
                    continue;
                }
                let mut current_segment_remaining = segment.clone();
                for (pattern_idx, re) in patterns.iter().enumerate() {
                    if let Some(caps) = re.captures(&current_segment_remaining) {
                        let val_match_idx_offset = if pattern_idx == 1 { 1 } else { 0 };
                        
                        if let Some(val_match) = caps.get(val_match_idx_offset + 1)
                            .or_else(|| caps.get(val_match_idx_offset + 2))
                            .or_else(|| caps.get(val_match_idx_offset + 3)) {
                            found_value_str = Some(val_match.as_str().to_string());
                            param_matched_in_any_segment = true;
                            let full_match_span = caps.get(0).unwrap();
                            // Trim sub-segments before adding to temp_next_segments
                            let before_match = current_segment_remaining[..full_match_span.start()].trim();
                            if !before_match.is_empty() {
                                temp_next_segments.push(before_match.to_string());
                            }
                            let after_match = current_segment_remaining[full_match_span.end()..].trim();
                            if !after_match.is_empty() {
                                temp_next_segments.push(after_match.to_string());
                            }
                            current_segment_remaining.clear(); // Processed this part of the segment
                            break; // Found value for this param in this segment with this pattern
                        }
                    }
                }
                if !current_segment_remaining.is_empty() { // If current segment had no match for this param with any pattern
                    temp_next_segments.push(current_segment_remaining); // Add it as is (it might be processed by other params later)
                }
            }
            if param_matched_in_any_segment {
                // Replace old segments with new (potentially more fragmented) ones
                text_segments_for_default = temp_next_segments.into_iter().filter(|s| !s.is_empty()).collect();
            }

            if !param_matched_in_any_segment && param_type_str == Some("boolean") {
                let flag_patterns = vec![
                    Regex::new(&format!(r"(?:^|\s)--{}(\b)", regex::escape(param_name))).unwrap(),
                    Regex::new(&format!(r"(?:^|\b){}(\b)", regex::escape(param_name))).unwrap(),
                ];
                let mut next_text_segments_for_bool = Vec::new();
                let mut bool_flag_found_overall = false;

                for segment in text_segments_for_default.iter() {
                    if bool_flag_found_overall { 
                        next_text_segments_for_bool.push(segment.clone());
                        continue;
                    }
                    let mut current_segment_remaining_for_bool = segment.clone();
                    for re_flag in &flag_patterns {
                        if let Some(flag_match) = re_flag.find(&current_segment_remaining_for_bool) {
                            found_value_str = Some("true".to_string());
                            bool_flag_found_overall = true;
                            if flag_match.start() > 0 {
                                next_text_segments_for_bool.push(current_segment_remaining_for_bool[..flag_match.start()].to_string());
                            }
                            if flag_match.end() < current_segment_remaining_for_bool.len() {
                                next_text_segments_for_bool.push(current_segment_remaining_for_bool[flag_match.end()..].to_string());
                            }
                            current_segment_remaining_for_bool.clear();
                            break;
                        }
                    }
                     if !current_segment_remaining_for_bool.is_empty() {
                        next_text_segments_for_bool.push(current_segment_remaining_for_bool);
                    }
                }
                if bool_flag_found_overall {
                     text_segments_for_default = next_text_segments_for_bool.into_iter().filter(|s| !s.trim().is_empty()).collect();
                     param_matched_in_any_segment = true;
                }
            }

            if let Some(value_str) = found_value_str {
                match param_type_str {
                    Some("string") => extracted_params.insert(param_name.to_string(), Value::String(value_str)),
                    Some("integer") => {
                        if let Ok(val_i) = value_str.parse::<i64>() { extracted_params.insert(param_name.to_string(), Value::Number(val_i.into())) }
                        else { log::warn!("Failed to parse integer for param '{}' from '{}'", param_name, value_str); None }
                    }
                    Some("number") => {
                        if let Ok(val_f) = value_str.parse::<f64>() {
                            if let Some(num) = serde_json::Number::from_f64(val_f) { extracted_params.insert(param_name.to_string(), Value::Number(num)) }
                            else { log::warn!("Failed to create JSON number for param '{}' from f64 '{}'", param_name, val_f); None }
                        } else if let Ok(val_i) = value_str.parse::<i64>() { extracted_params.insert(param_name.to_string(), Value::Number(val_i.into())) }
                        else { log::warn!("Failed to parse number for param '{}' from '{}'", param_name, value_str); None }
                    }
                    Some("boolean") => {
                        match value_str.to_lowercase().as_str() {
                            "true" | "yes" | "1" | "on" => extracted_params.insert(param_name.to_string(), Value::Bool(true)),
                            "false" | "no" | "0" | "off" => extracted_params.insert(param_name.to_string(), Value::Bool(false)),
                            _ => { log::warn!("Failed to parse boolean for param '{}' from '{}'", param_name, value_str); None }
                        }
                    }
                    _ => { log::debug!("Unhandled param type '{:?}' for '{}'", param_type_str, param_name); None }
                };
            }

            if required_params_set.contains(param_name) && !extracted_params.contains_key(param_name) {
                missing_required_params.push(param_name.to_string());
            }
        }
    }

    let final_remaining_text = text_segments_for_default.join(" ").trim().to_string();
    if !final_remaining_text.is_empty() {
        let mut use_default_key = true;
        if let Some(properties) = schema_val.get("properties").and_then(|p| p.as_object()) {
            // Fallback logic: try common names first, then default key.
            // The more specific single-param fallback (if props.len() == 1 and processed_input == final_remaining_text) is removed for now.
            let mut common_name_found_and_used = false;
            for default_candidate in ["input", "query", "text", "message", "content"] {
                if properties.contains_key(default_candidate) && 
                   properties[default_candidate].get("type").and_then(|t| t.as_str()) == Some("string") &&
                   !extracted_params.contains_key(default_candidate) {
                       extracted_params.insert(default_candidate.to_string(), Value::String(final_remaining_text.clone()));
                       use_default_key = false;
                       common_name_found_and_used = true; // Mark that common name was used
                       if let Some(pos) = missing_required_params.iter().position(|x| x == default_candidate) {
                           missing_required_params.remove(pos);
                       }
                       break;
                   }
            }
            // Note: The original `else if properties.len() == 1` block that would have been here is omitted for this test.
        }
        if use_default_key { // This will be true if common_name_fallback didn't run or found no suitable key.
            extracted_params.insert(DEFAULT_STRING_PARAM_KEY.to_string(), Value::String(final_remaining_text));
        }
    }

    Ok((Value::Object(extracted_params), missing_required_params))
}

#[async_trait]
impl Tool for AnalyzeInputTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "analyze_input".to_string(),
            description: "Analyzes the initial user input to determine intent, extract entities, and suggest initial actions using semantic search and schema-based parameter extraction.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The user input to analyze."
                    }
                },
                "required": ["input"]
            }),
            is_required: true,
            category: ToolCategory::Core,
            metadata: HashMap::new(),
        }
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResult, SagittaCodeError> {
        log::debug!("[SemanticAnalyzeInputTool]: executing with parameters: {:?}", parameters);
        let input_str = match parameters.get("input").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(ToolResult::error("Missing or invalid 'input' parameter for AnalyzeInputTool")),
        };

        let input_embedding = match self.embedding_provider.embed_batch(&[input_str]) {
            Ok(mut embeddings) => match embeddings.pop() {
                Some(emb) => Ok(emb),
                None => Err(SagittaCodeError::ToolError("Embedding batch returned empty for single input".to_string())),
            },
            Err(e) => Err(SagittaCodeError::SagittaDbError(format!("Embedding provider error for input '{}': {}", input_str, e))),
        }?; // Use ? to propagate SagittaCodeError

        let search_request = SearchPoints { 
            collection_name: TOOLS_COLLECTION_NAME.to_string(),
            vector: input_embedding,
            limit: 1, 
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            score_threshold: Some(0.6), 
            offset: None, 
            filter: None, 
            params: None, 
            vector_name: Some("dense".to_string()),
            read_consistency: None, 
            timeout: None, 
            shard_key_selector: None, 
            sparse_indices: None,
        };

        match self.qdrant_client.search_points(search_request).await {
            Ok(search_response) => {
                if let Some(hit) = search_response.result.into_iter().next() {
                    let tool_name_val = hit.payload.get("tool_name").and_then(|v| v.as_str().map(String::from));
                    let schema_str = hit.payload.get("parameter_schema").and_then(|v| v.as_str());
                    let score = hit.score;

                    if let (Some(tn), Some(sch_str)) = (tool_name_val, schema_str) {
                        log::info!("Qdrant matched tool '{}' with score: {}", tn, score);
                        
                        let mut input_for_extraction = input_str.trim().to_string();
                        if input_for_extraction.to_lowercase().starts_with(&tn.to_lowercase()) {
                            let tool_name_len = tn.len();
                            if input_for_extraction.len() == tool_name_len {
                                input_for_extraction.clear();
                            } else if input_for_extraction.len() > tool_name_len {
                                let char_after_tool_name = input_for_extraction.chars().nth(tool_name_len).unwrap_or(' ');
                                if !char_after_tool_name.is_alphanumeric() {
                                    input_for_extraction = input_for_extraction[tool_name_len..].trim().to_string();
                                }
                            }
                        }
                        
                        match serde_json::from_str(sch_str) {
                            Ok(schema_json_val) => {
                                let (inferred_params, missing_required) = extract_parameters_regex(&input_for_extraction, &tn, &schema_json_val)?;
                                
                                let mut analysis_msg = format!("Semantically matched tool '{}' via Qdrant with score {:.2}. Parameters extracted.", tn, score);
                                if !missing_required.is_empty() {
                                    analysis_msg.push_str(&format!(" Missing required parameters: {}.", missing_required.join(", ")));
                                }

                                let analysis_result = SemanticAnalysisResult {
                                    original_input: input_str.to_string(),
                                    main_goal_description: Some(format!("Process user input related to tool: {}", tn)),
                                    extracted_entities: HashMap::new(), 
                                    proposed_sub_goals: vec![
                                        ProposedSubGoal {
                                            description: format!("Execute tool '{}' based on semantic match (score: {:.2}).", tn, score),
                                            action_type: "tool_execution".to_string(),
                                            suggested_tool_name: Some(tn.clone()),
                                            inferred_tool_parameters: Some(inferred_params),
                                        }
                                    ],
                                    confidence: if missing_required.is_empty() { score } else { score * 0.7 },
                                    analysis_message: analysis_msg,
                                };
                                Ok(ToolResult::Success(serde_json::to_value(analysis_result)?))
                            }
                            Err(e) => {
                                log::error!("Failed to parse parameter schema for tool '{}': {}. Schema string: {}", tn, e, sch_str);
                                let fallback_result = SemanticAnalysisResult { original_input: input_str.to_string(), main_goal_description: Some("Fallback: Schema parsing error".to_string()), extracted_entities: HashMap::new(), proposed_sub_goals: vec![], confidence: 0.2, analysis_message: format!("Schema parsing error for {}: {}", tn, e) };
                                Ok(ToolResult::Success(serde_json::to_value(fallback_result)?))
                            }
                        }
                    } else {
                        log::warn!("Qdrant hit missing tool_name or parameter_schema in payload: {:?}", hit.payload);
                        let fallback_result = SemanticAnalysisResult { original_input: input_str.to_string(), main_goal_description: Some("Fallback: Malformed tool data in DB".to_string()), extracted_entities: HashMap::new(), proposed_sub_goals: vec![], confidence: 0.2, analysis_message: "Malformed tool data from Qdrant".to_string() };
                        Ok(ToolResult::Success(serde_json::to_value(fallback_result)?))
                    }
                } else {
                    log::info!("No semantic tool match from Qdrant for input: {}", input_str);
                    let fallback_result = SemanticAnalysisResult {
                        original_input: input_str.to_string(),
                        main_goal_description: Some("Default to LLM chat/general processing due to no strong semantic tool match.".to_string()),
                        extracted_entities: HashMap::new(),
                        proposed_sub_goals: vec![
                            ProposedSubGoal {
                                description: "Engage in general conversation or await further instructions.".to_string(),
                                action_type: "llm_query".to_string(),
                                suggested_tool_name: None,
                                inferred_tool_parameters: None,
                            }
                        ],
                        confidence: 0.3, 
                        analysis_message: "No specific tool semantically matched above threshold. Defaulting to LLM.".to_string(),
                    };
                    Ok(ToolResult::Success(serde_json::to_value(fallback_result)?))
                }
            }
            Err(e) => {
                log::error!("Qdrant search_points failed for AnalyzeInputTool: {}", e);
                let fallback_result = SemanticAnalysisResult {
                    original_input: input_str.to_string(),
                    main_goal_description: Some("Error in analysis: Qdrant search failed.".to_string()),
                    extracted_entities: HashMap::new(),
                    proposed_sub_goals: vec![],
                    confidence: 0.1,
                    analysis_message: format!("Qdrant search failed: {}", e),
                };
                Ok(ToolResult::Success(serde_json::to_value(fallback_result)?))
            }
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Import items from parent module
    use serde_json::json;
    use std::sync::Arc;
    use qdrant_client::Qdrant;
    use qdrant_client::qdrant::{
        CreateCollection, VectorParams, VectorsConfig, Distance, UpsertPoints, GetCollectionInfoRequest,
        vectors_config::Config as VectorsConfigEnum
    };
    use sagitta_search::config::AppConfig as SagittaAppConfig;
    use sagitta_embed::provider::onnx::OnnxEmbeddingModel;
    use sagitta_search::qdrant_client_trait::QdrantClientTrait;
    use sagitta_embed::provider::EmbeddingProvider;
    use std::path::Path;
    use tempfile::TempDir;

    // Helper to set up a Qdrant client for tests
    // This assumes Qdrant is running at the default localhost:6334 for testing purposes
    async fn setup_test_qdrant_client() -> Result<Arc<dyn QdrantClientTrait>, anyhow::Error> {
        let qdrant_url = "http://localhost:6334"; // Standard test Qdrant URL
        let qdrant_client = Arc::new(Qdrant::from_url(qdrant_url).build()?);
        Ok(qdrant_client)
    }

    #[tokio::test]
    #[ignore] // This test requires a running Qdrant instance
    async fn test_tools_collection_creation_and_named_vector_upsert() -> Result<(), anyhow::Error> {
        let qdrant_client = setup_test_qdrant_client().await?;

        // 1. Ensure collection is deleted (clean state)
        let _ = qdrant_client.delete_collection(TOOLS_COLLECTION_NAME.to_string()).await; // Ignore error if not found

        // 2. Create the collection (mimicking logic from main.rs/gui/app/initialization.rs)
        let vector_size = 384u64; // Standard embedding dimension
        let create_collection_request = CreateCollection {
            collection_name: TOOLS_COLLECTION_NAME.to_string(),
            vectors_config: Some(VectorsConfig {
                config: Some(VectorsConfigEnum::ParamsMap(
                    qdrant_client::qdrant::VectorParamsMap {
                        map: std::collections::HashMap::from([
                            ("dense".to_string(), VectorParams {
                                size: vector_size,
                                distance: Distance::Cosine.into(),
                                hnsw_config: None,
                                quantization_config: None,
                                on_disk: None,
                                datatype: None,
                                multivector_config: None,
                            })
                        ])
                    }
                ))
            }),
            shard_number: None,
            sharding_method: None,
            replication_factor: None,
            write_consistency_factor: None,
            on_disk_payload: None,
            hnsw_config: None,
            wal_config: None,
            optimizers_config: None,
            init_from_collection: None,
            quantization_config: None,
            sparse_vectors_config: None, 
            timeout: None,
            strict_mode_config: None,
        };

        qdrant_client.create_collection_detailed(create_collection_request).await
            .map_err(|e| anyhow::anyhow!("Failed to create collection for test: {}", e))?;

        // 3. Verify collection configuration
        let collection_info = qdrant_client.get_collection_info(TOOLS_COLLECTION_NAME.to_string()).await
            .map_err(|e| anyhow::anyhow!("Failed to get collection info: {}", e))?;
        
        assert!(collection_info.config.is_some(), "Collection config should not be None");
        let config_params = collection_info.config.unwrap().params.unwrap();

        assert!(config_params.vectors_config.is_some(), "Vectors config in params should not be None");
        match config_params.vectors_config.unwrap().config.unwrap() {
            VectorsConfigEnum::Params(params) => {
                panic!("Expected VectorsConfigEnum::ParamsMap (named vectors) but found Params (unnamed vector)");
            },
            VectorsConfigEnum::ParamsMap(params_map) => {
                assert!(params_map.map.contains_key("dense"), "Dense vector should be configured");
                let dense_params = params_map.map.get("dense").unwrap();
                assert_eq!(dense_params.size, vector_size, "Vector size in config mismatch");
                assert_eq!(dense_params.distance, Distance::Cosine as i32, "Distance in config mismatch");
            }
        }

        // 4. Attempt to upsert a point with a named vector (using dummy embedding)
        let dummy_embedding = vec![0.1f32; vector_size as usize]; // Create a dummy embedding vector

        let point = PointStruct {
            id: Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(0)) }),
            vectors: Some(Vectors { // The enum wrapper
                vectors_options: Some(VectorsOptions::Vectors(
                    NamedVectors::default()
                        .add_vector("dense".to_string(), dummy_embedding.clone())
                ))
            }),
            payload: Default::default(),
        };
        let upsert_request = UpsertPoints {
            collection_name: TOOLS_COLLECTION_NAME.to_string(),
            points: vec![point],
            wait: Some(true),
            ordering: None,
            shard_key_selector: None,
        };

        qdrant_client.upsert_points(upsert_request).await
            .map_err(|e| anyhow::anyhow!("Upsert with named vector failed: {}", e))?;

        // 5. Clean up: Delete the collection
        qdrant_client.delete_collection(TOOLS_COLLECTION_NAME.to_string()).await
            .map_err(|e| anyhow::anyhow!("Failed to delete collection post-test: {}", e))?;

        Ok(())
    }
}