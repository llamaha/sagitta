use async_trait::async_trait;
use std::sync::Arc;
use log::{debug, warn, trace};
use serde_json::Value;
use tokio::runtime::Handle;

// Corrected paths to refer to sagitta_search library
use sagitta_embed::provider::EmbeddingProvider; // The trait
use sagitta_embed::provider::onnx::OnnxEmbeddingModel;

// Corrected ReasoningError import
use reasoning_engine::traits::{IntentAnalyzer, DetectedIntent, LlmMessage};
use reasoning_engine::ReasoningError; // Import ReasoningError directly

use std::collections::HashMap; // For more complex prototype storage if needed

use crate::utils::errors::SagittaCodeError;

// Basic cosine similarity function (can be moved to a shared utility module)
fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let dot_product: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm_v1: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_v2: f32 = v2.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_v1 == 0.0 || norm_v2 == 0.0 {
        0.0
    } else {
        dot_product / (norm_v1 * norm_v2)
    }
}

#[derive(Debug)] // Added Debug derive
pub struct SagittaCodeIntentAnalyzer {
    embedding_provider: Arc<dyn EmbeddingProvider + Send + Sync + 'static>,
    intent_prototypes: Vec<(DetectedIntent, Vec<f32>)>, // Store pre-embedded prototypes
    rt_handle: Handle,
}

impl SagittaCodeIntentAnalyzer {
    /// Creates a new `SagittaCodeIntentAnalyzer`.
    pub fn new(embedding_provider: Arc<dyn EmbeddingProvider + Send + Sync + 'static>) -> Self {
        let prototypes_phrases = vec![
            // More specific final answer patterns
            (DetectedIntent::ProvidesFinalAnswer, "The entire task is now complete. I have finished everything you requested."),
            (DetectedIntent::ProvidesFinalAnswer, "I have completed all the requested actions and provided the final answer."),
            (DetectedIntent::ProvidesFinalAnswer, "That concludes everything you asked for. The task is fully complete."),
            
            // Clarifying questions
            (DetectedIntent::AsksClarifyingQuestion, "Could you please clarify what you mean by that?"),
            (DetectedIntent::AsksClarifyingQuestion, "What exactly are you asking for?"),
            (DetectedIntent::AsksClarifyingQuestion, "I need clarification on this point."),
            
            // Requests for more input
            (DetectedIntent::RequestsMoreInput, "I need more information to proceed. What else should I do?"),
            (DetectedIntent::RequestsMoreInput, "Please tell me more so I can help."),
            (DetectedIntent::RequestsMoreInput, "What would you like me to do next?"),
            
            // Inability to proceed
            (DetectedIntent::StatesInabilityToProceed, "I'm sorry, I cannot fulfill that request at this time."),
            (DetectedIntent::StatesInabilityToProceed, "I am unable to do that."),
            (DetectedIntent::StatesInabilityToProceed, "This is not something I can accomplish."),
            
            // Plans without action (should trigger nudging)
            (DetectedIntent::ProvidesPlanWithoutExplicitAction, "Okay, first I will do X, then I will do Y, and finally Z."),
            (DetectedIntent::ProvidesPlanWithoutExplicitAction, "Here is my plan of action: step 1, step 2, step 3."),
            (DetectedIntent::ProvidesPlanWithoutExplicitAction, "My approach will be to first analyze, then implement, then test."),
            
            // General conversation
            (DetectedIntent::GeneralConversation, "Hello! How are you today?"),
            (DetectedIntent::GeneralConversation, "Hi there, what can I do for you?"),
            (DetectedIntent::GeneralConversation, "Okay, sounds good."),
        ];

        let mut intent_prototypes = Vec::new();
        debug!("SagittaCodeIntentAnalyzer: Embedding intent prototypes...");
        for (intent, phrase) in prototypes_phrases {
            // Use a block to ensure the lock is released after embedding
            let embedding_result = {
                embedding_provider.embed_batch(&[phrase])
            };

            match embedding_result {
                Ok(mut embeddings) if !embeddings.is_empty() => {
                    intent_prototypes.push((intent.clone(), embeddings.remove(0)));
                    debug!("Successfully embedded prototype for {:?}: '{}'", intent, phrase);
                }
                Ok(_) => {
                    warn!("SagittaCodeIntentAnalyzer: Failed to embed prototype (empty result) for {:?}: '{}'", intent, phrase);
                }
                Err(e) => {
                    warn!("SagittaCodeIntentAnalyzer: Embedding failed for prototype '{}' ({:?}): {:?}", phrase, intent, e);
                }
            }
        }
        if intent_prototypes.is_empty() {
            warn!("SagittaCodeIntentAnalyzer: No intent prototypes were successfully embedded. Intent analysis will be impaired.");
        }
        debug!("SagittaCodeIntentAnalyzer: Embedded {} intent prototypes successfully.", intent_prototypes.len());

        Self {
            embedding_provider,
            intent_prototypes,
            rt_handle: Handle::current(),
        }
    }
}

#[async_trait]
impl IntentAnalyzer for SagittaCodeIntentAnalyzer {
    async fn analyze_intent(
        &self,
        text: &str,
        conversation_context: Option<&[LlmMessage]>, // Context can be used for more advanced rules later
    ) -> Result<DetectedIntent, ReasoningError> {
        if text.trim().is_empty() {
            debug!("SagittaCodeIntentAnalyzer: Received empty text, returning Ambiguous intent.");
            return Ok(DetectedIntent::Ambiguous);
        }

        debug!("SagittaCodeIntentAnalyzer: Analyzing intent for text: \"{}\"", text);

        // CRITICAL: Check for intermediate summaries that should NOT be treated as final answers
        let is_intermediate_summary = text.contains("I've finished those tasks") ||
                                    text.contains("Successfully completed:") ||
                                    text.contains("What would you like to do next?") ||
                                    text.contains("Now I'll") ||
                                    text.contains("Next, I'll") ||
                                    text.contains("Following that") ||
                                    text.contains("After that") ||
                                    text.contains("Then I'll") ||
                                    text.contains("Let me") ||
                                    text.contains("I'll now") ||
                                    text.contains("I'll proceed") ||
                                    text.contains("I'll continue") ||
                                    text.contains("Moving on") ||
                                    text.contains("repository_map") ||
                                    text.contains("targeted_view") ||
                                    text.contains("view_file") ||
                                    text.contains("search_code") ||
                                    text.contains("add_repository") ||
                                    text.contains("sync_repository") ||
                                    text.contains("I need to") ||
                                    text.contains("I should") ||
                                    text.contains("I will") ||
                                    text.contains("Let me start by") ||
                                    text.contains("First, I'll") ||
                                    text.contains("To help you") ||
                                    text.contains("I can help") ||
                                    text.contains("Here's what I'll do") ||
                                    text.contains("My approach will be") ||
                                    text.contains("I'll help you with that");

        if is_intermediate_summary {
            debug!("SagittaCodeIntentAnalyzer: Detected intermediate summary, returning RequestsMoreInput to continue processing.");
            return Ok(DetectedIntent::RequestsMoreInput);
        }

        // Check for explicit completion indicators - be more strict about what constitutes completion
        let has_strong_completion_indicators = text.contains("task is fully complete") ||
                                      text.contains("everything you requested") ||
                                      text.contains("concludes everything") ||
                                      text.contains("all requested actions") ||
                                      text.contains("completely finished") ||
                                             text.contains("entirely done") ||
                                             text.contains("That's all") ||
                                             text.contains("Nothing more to do") ||
                                             text.contains("Task completed successfully");

        // Check for weak completion indicators that might be false positives
        let has_weak_completion_indicators = text.contains("completed") ||
                                           text.contains("finished") ||
                                           text.contains("done");

        // Check for plan indicators
        let has_plan_indicators = text.contains("Here's my plan") ||
                                text.contains("I'll help you with that! Here's my plan") ||
                                text.contains("my plan:") ||
                                text.contains("approach will be") ||
                                text.contains("steps I'll take") ||
                                text.contains("Here's what I'll do") ||
                                (text.contains("First,") && text.contains("Then,") && text.contains("Finally,"));

        // Check for continuation indicators
        let has_continuation_indicators = text.contains("What would you like") ||
                                        text.contains("How can I help") ||
                                        text.contains("Is there anything else") ||
                                        text.contains("What's next") ||
                                        text.contains("What should I do next") ||
                                        text.contains("Any other") ||
                                        text.contains("Would you like me to");

        // Prioritize continuation over weak completion
        if has_continuation_indicators {
            debug!("SagittaCodeIntentAnalyzer: Detected continuation indicators, returning RequestsMoreInput.");
            return Ok(DetectedIntent::RequestsMoreInput);
        }

        if has_plan_indicators && !has_strong_completion_indicators {
            debug!("SagittaCodeIntentAnalyzer: Detected plan without strong completion, returning ProvidesPlanWithoutExplicitAction.");
            return Ok(DetectedIntent::ProvidesPlanWithoutExplicitAction);
        }

        // Only treat as final answer if we have strong completion indicators and no continuation indicators
        if has_strong_completion_indicators && !has_continuation_indicators {
            debug!("SagittaCodeIntentAnalyzer: Detected strong completion indicators without continuation, returning ProvidesFinalAnswer.");
            return Ok(DetectedIntent::ProvidesFinalAnswer);
        }

        // If we have weak completion indicators but also continuation indicators, prefer continuation
        if has_weak_completion_indicators && has_continuation_indicators {
            debug!("SagittaCodeIntentAnalyzer: Detected weak completion with continuation indicators, returning RequestsMoreInput.");
            return Ok(DetectedIntent::RequestsMoreInput);
        }

        if self.intent_prototypes.is_empty() {
            warn!("SagittaCodeIntentAnalyzer: No intent prototypes available for comparison. Returning RequestsMoreInput to be safe.");
            return Ok(DetectedIntent::RequestsMoreInput); // Changed from Ambiguous to be less conservative
        }

        let text_embedding = match self.embedding_provider.embed_batch(&[text]) {
            Ok(mut embeddings) if !embeddings.is_empty() => embeddings.remove(0),
            Ok(_) => {
                warn!("SagittaCodeIntentAnalyzer: Could not get embedding for text (empty result): \"{}\"", text);
                return Ok(DetectedIntent::RequestsMoreInput); // Changed from Ambiguous to be less conservative
            }
            Err(e) => {
                warn!("SagittaCodeIntentAnalyzer: Embedding failed for text \"{}\": {:?}", text, e);
                return Err(ReasoningError::intent_analysis(format!("Embedding failed for intent analysis: {}", e)));
            }
        };

        let mut best_match = DetectedIntent::RequestsMoreInput; // Changed default from Ambiguous
        let mut highest_similarity = -1.0_f32; // Initialize with a value lower than any possible cosine similarity

        for (intent, prototype_embedding) in &self.intent_prototypes {
            let similarity = cosine_similarity(&text_embedding, prototype_embedding);
            // trace! used for potentially very verbose logging
            trace!("SagittaCodeIntentAnalyzer: Similarity of '{}' with {:?}: {:.4}", text, intent, similarity);
            if similarity > highest_similarity {
                highest_similarity = similarity;
                best_match = intent.clone();
            }
        }
        
        // Lower threshold for most intents to be less conservative, but higher for final answers
        let similarity_threshold = match best_match {
            DetectedIntent::ProvidesFinalAnswer => 0.80, // Higher threshold for final answers
            DetectedIntent::StatesInabilityToProceed => 0.75, // Higher threshold for inability
            _ => 0.55, // Lower threshold for other intents to be less conservative
        };
        
        if highest_similarity < similarity_threshold {
            debug!(
                "SagittaCodeIntentAnalyzer: Highest similarity {:.4} for '{}' is below threshold {}. Returning RequestsMoreInput to continue.",
                highest_similarity,
                text,
                similarity_threshold
            );
            return Ok(DetectedIntent::RequestsMoreInput); // Changed from Ambiguous to be less conservative
        }

        // Additional safety check: if we detected a final answer but there are continuation indicators, override it
        if best_match == DetectedIntent::ProvidesFinalAnswer && (is_intermediate_summary || has_continuation_indicators) {
            debug!("SagittaCodeIntentAnalyzer: Overriding ProvidesFinalAnswer due to intermediate summary or continuation indicators.");
            return Ok(DetectedIntent::RequestsMoreInput);
        }

        debug!("SagittaCodeIntentAnalyzer: Best match intent for '{}': {:?} with similarity {:.4}", text, best_match, highest_similarity);
        Ok(best_match)
    }
} 