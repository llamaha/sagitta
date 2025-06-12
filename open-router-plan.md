# OpenRouter Migration Plan

## Overview
This plan outlines the migration from Google Gemini LLM to OpenRouter, which provides unified access to multiple AI models through a single API. This will enable access to hundreds of models including OpenAI GPT, Anthropic Claude, Meta Llama, and many others.

**NOTE**: Since the tool is not yet in use, we will perform a complete replacement of Gemini with OpenRouter - no backwards compatibility or gradual migration needed.

## ✅ CURRENT STATUS: Phase 1 COMPLETED ✅

**Overall Progress: ~25-30% Complete**

- ✅ **Phase 1: Configuration Migration** - COMPLETED
- 🚧 **Phase 2: LLM Client Implementation** - IN PROGRESS (Basic structure created, API implementation needed)
- ⏳ **Phase 3: GUI Integration** - READY (Basic UI completed, dynamic model selection needed)
- ⏳ **Phase 4: Reasoning Engine Integration** - PENDING
- ⏳ **Phase 5: Testing and Validation** - PENDING
- ⏳ **Phase 6: Documentation and Cleanup** - PENDING

## Key Benefits of OpenRouter
- **Unified API**: Access 400+ models through one interface
- **Cost Optimization**: Automatic routing to cheapest providers
- **High Availability**: Built-in fallbacks and load balancing
- **Model Diversity**: Access to cutting-edge models from multiple providers
- **OpenAI Compatible**: Drop-in replacement for OpenAI SDK

## Research Findings

### OpenRouter API Details
- **Base URL**: `https://openrouter.ai/api/v1`
- **Authentication**: Bearer token (OPENROUTER_API_KEY)
- **Models API**: `GET /api/v1/models` for dynamic model list
- **Streaming**: Fully supported with SSE (Server-Sent Events)
- **OpenAI Compatible**: Can use OpenAI SDK with different base URL

### Example Streaming Implementation (from OpenRouter examples):
```typescript
const stream = await openai.chat.completions.create({
  model: "openai/gpt-4",
  messages: [{ role: "user", content: "Hello" }],
  stream: true,
});
for await (const part of stream) {
  process.stdout.write(part.choices[0]?.delta?.content || "");
}
```

## Migration Plan

### ✅ Phase 1: Configuration Migration - COMPLETED
**Goal**: Replace Gemini configuration with OpenRouter configuration

#### ✅ 1.1 Update Configuration Types (`sagitta-code`) - COMPLETED
- **File**: `crates/sagitta-code/src/config/types.rs`
- **Status**: ✅ COMPLETED
- **Changes**:
  ```rust
  // ✅ Implemented OpenRouterConfig with all required fields
  pub struct OpenRouterConfig {
      /// OpenRouter API key
      pub api_key: Option<String>,
      
      /// Selected model (e.g., "openai/gpt-4", "anthropic/claude-3-5-sonnet")
      pub model: String,
      
      /// Provider preferences (optional routing configuration)
      pub provider_preferences: Option<ProviderPreferences>,
      
      /// Maximum message history size
      #[serde(default = "default_max_history_size")]
      pub max_history_size: usize,
      
      /// Maximum reasoning steps to prevent infinite loops
      #[serde(default = "default_max_reasoning_steps")]
      pub max_reasoning_steps: u32,
      
      /// Request timeout in seconds
      #[serde(default = "default_request_timeout")]
      pub request_timeout: u64,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct ProviderPreferences {
      /// Preferred providers in order
      pub order: Option<Vec<String>>,
      /// Whether to allow fallbacks
      pub allow_fallbacks: Option<bool>,
      /// Sort by price, throughput, or latency
      pub sort: Option<String>,
      /// Data collection policy
      pub data_collection: Option<String>,
  }
  ```

#### ✅ 1.2 Update Main Config Structure - COMPLETED
- **Status**: ✅ COMPLETED
- ✅ Replaced `gemini: GeminiConfig` with `openrouter: OpenRouterConfig`
- ✅ Updated default model from "gemini-2.5-flash-preview-05-20" to "openai/gpt-4"
- ✅ Removed all Gemini-related configuration

#### ✅ 1.3 Update Configuration Loading - COMPLETED
- **File**: `crates/sagitta-code/src/config/loader.rs`
- **Status**: ✅ COMPLETED
- ✅ Removed Gemini configuration loading and validation
- ✅ Added OpenRouter API key validation with environment variable support (`OPENROUTER_API_KEY`)
- ✅ Fixed `load_all_configs()` return type
- ✅ Added missing `save_config_to_path` function
- ✅ Updated all test cases to use TOML format and OpenRouter config

#### ✅ 1.4 Update Module Exports - COMPLETED
- **File**: `crates/sagitta-code/src/config/mod.rs`
- **Status**: ✅ COMPLETED
- ✅ Export new OpenRouter types (`OpenRouterConfig`, `ProviderPreferences`)
- ✅ Removed Gemini exports completely
- ✅ Added `save_config_to_path` to exports

#### ✅ 1.5 Update All Code References - COMPLETED
- **Status**: ✅ COMPLETED
- ✅ Updated `crates/sagitta-code/src/main.rs` to use `OpenRouterClient`
- ✅ Updated `crates/sagitta-code/src/agent/core.rs` imports and system prompt
- ✅ Updated `crates/sagitta-code/src/reasoning/config.rs` to use `openrouter` config
- ✅ Updated GUI initialization file to use `OpenRouterClient`
- ✅ Updated settings panel to use OpenRouter configuration
- ✅ Updated `crates/sagitta-code/src/bin/chat_cli.rs` to use OpenRouter
- ✅ Updated all test files to use OpenRouter instead of Gemini
- ✅ Fixed all compilation errors and test failures

### 🚧 Phase 2: LLM Client Implementation (`sagitta-code`) - IN PROGRESS
**Goal**: Replace Gemini client with OpenRouter client

#### ✅ 2.1 Delete Gemini Module and Create OpenRouter Module - COMPLETED
- **Status**: ✅ COMPLETED
- ✅ **Deleted**: `crates/sagitta-code/src/llm/gemini/` directory completely
- ✅ **Created**: 
```
crates/sagitta-code/src/llm/openrouter/
├── mod.rs          ✅ COMPLETED
├── client.rs       ✅ BASIC STRUCTURE (needs API implementation)
├── api.rs          ✅ BASIC STRUCTURE (needs response types)
├── streaming.rs    ✅ PLACEHOLDER (needs implementation)
├── models.rs       ✅ PLACEHOLDER (needs implementation)  
└── error.rs        ✅ COMPLETED
```

#### 🚧 2.2 Implement OpenRouter Client (`client.rs`) - PARTIAL
- **Status**: 🚧 BASIC STRUCTURE CREATED - NEEDS API IMPLEMENTATION
- ✅ Basic `OpenRouterClient` struct with HTTP client and configuration
- ✅ Proper API key handling from config or environment
- ✅ Required HTTP headers for OpenRouter API
- ✅ All required `LlmClient` trait methods as placeholders
- ✅ `get_models()` method for dynamic model discovery
- ❌ **TODO**: Implement actual API calls (generate, generate_stream, etc.)
- ❌ **TODO**: OpenAI SDK compatibility layer
- ❌ **TODO**: Error handling and retries
- ❌ **TODO**: Rate limiting and circuit breaker
- ❌ **TODO**: Token usage tracking

#### ❌ 2.3 Implement Streaming (`streaming.rs`) - PLACEHOLDER
- **Status**: ❌ PLACEHOLDER ONLY
- ❌ **TODO**: Server-Sent Events (SSE) parsing
- ❌ **TODO**: Chunk aggregation and buffering
- ❌ **TODO**: Error recovery and reconnection
- ❌ **TODO**: Integration with reasoning-engine streaming
- ❌ **TODO**: Backpressure handling

#### ❌ 2.4 Implement Model Discovery (`models.rs`) - PLACEHOLDER
- **Status**: ❌ PLACEHOLDER ONLY
- ❌ **TODO**: Dynamic model fetching from OpenRouter API
- ❌ **TODO**: Model filtering and categorization
- ❌ **TODO**: Caching mechanism for model list
- ❌ **TODO**: Provider information extraction

#### ✅ 2.5 Update LLM Module - COMPLETED
- **File**: `crates/sagitta-code/src/llm/mod.rs`
- **Status**: ✅ COMPLETED
- ✅ Replace `pub mod gemini` with `pub mod openrouter`
- ✅ Update re-exports

### 🎯 Phase 3: GUI Integration (`sagitta-code`) - READY
**Goal**: Update settings UI for OpenRouter configuration

#### ✅ 3.1 Update Settings Panel - MOSTLY COMPLETED
- **Status**: ✅ BASIC UI COMPLETED - ADVANCED FEATURES PENDING
- ✅ **Components Completed**:
  - ✅ Replaced Gemini API key field with OpenRouter API key field
  - ✅ Replaced model field with OpenRouter model text field
  - ✅ Added max_reasoning_steps configuration
  - ✅ Basic OpenRouter configuration persistence
- ❌ **Components TODO**:
  - ❌ Advanced model dropdown with search/filter capability
  - ❌ Provider preferences section
  - ❌ Model refresh button
  - ❌ Model information display (pricing, context length, etc.)

#### ❌ 3.2 Implement Dynamic Model Selection - PENDING
- **Status**: ❌ NOT STARTED
- ❌ **TODO**: Searchable dropdown with hundreds of models
- ❌ **TODO**: Filtering by provider, capability, price
- ❌ **TODO**: Real-time model information
- ❌ **TODO**: Favorites/recently used models
- ❌ **TODO**: Model comparison view

#### ✅ 3.3 Update Settings Persistence - COMPLETED
- **Status**: ✅ COMPLETED
- ✅ Replace all Gemini settings references
- ✅ Save OpenRouter preferences to config.toml
- ✅ Handle API key storage securely

### ⏳ Phase 4: Reasoning Engine Integration - PENDING
**Goal**: Update reasoning-engine to work with OpenRouter

#### ❌ 4.1 Update LLM Client Adapter - PENDING
- **File**: `crates/reasoning-engine/src/lib.rs` (or create new adapter)
- **Status**: ❌ NOT STARTED
- ❌ **TODO**: Replace Gemini client references with OpenRouter client
- ❌ **TODO**: Create `OpenRouterLlmClientAdapter` implementing `LlmClient` trait
- ❌ **TODO**: Handle OpenRouter-specific response formats
- ❌ **TODO**: Integrate with streaming engine

#### ❌ 4.2 Update Streaming Integration - PENDING
- **File**: `crates/reasoning-engine/src/streaming.rs`
- **Status**: ❌ NOT STARTED
- ❌ **TODO**: Ensure compatibility with OpenRouter SSE format
- ❌ **TODO**: Handle OpenRouter-specific chunk types
- ❌ **TODO**: Maintain existing streaming state machine

#### ❌ 4.3 Update Error Handling - PENDING
- **Status**: ❌ NOT STARTED
- ❌ **TODO**: Map OpenRouter errors to `ReasoningError`
- ❌ **TODO**: Handle rate limiting and provider failures
- ❌ **TODO**: Implement retry logic for different error types

### ⏳ Phase 5: Testing and Validation - PENDING
**Goal**: Ensure robust migration with comprehensive testing

#### ✅ 5.1 Unit Tests - PARTIALLY COMPLETED
- **Status**: ✅ CONFIGURATION TESTS COMPLETED - API TESTS PENDING
- ✅ **Completed**: Configuration loading tests updated to OpenRouter
- ✅ **Completed**: Settings panel tests updated to OpenRouter
- ✅ **Completed**: Core tests updated to use OpenRouter client structure
- ❌ **TODO**: OpenRouter client functionality tests
- ❌ **TODO**: Streaming chunk processing tests  
- ❌ **TODO**: Error handling scenario tests

#### ❌ 5.2 Integration Tests - PENDING
- **Status**: ❌ NOT STARTED
- ❌ **TODO**: End-to-end conversation flows
- ❌ **TODO**: Model switching during conversations
- ❌ **TODO**: Provider fallback scenarios
- ❌ **TODO**: Rate limiting behavior

#### ❌ 5.3 Performance Testing - PENDING
- **Status**: ❌ NOT STARTED
- ❌ **TODO**: Streaming performance validation
- ❌ **TODO**: Memory usage validation
- ❌ **TODO**: Concurrent request handling
- ❌ **TODO**: Model discovery caching

### ⏳ Phase 6: Documentation and Cleanup - PENDING
**Goal**: Complete migration with proper documentation

#### ❌ 6.1 Update Documentation - PENDING
- **Status**: ❌ NOT STARTED
- ❌ **TODO**: README files for both crates
- ❌ **TODO**: Configuration examples
- ❌ **TODO**: Setup guide for users
- ❌ **TODO**: Troubleshooting guide

#### 🎯 6.2 Final Cleanup - MOSTLY COMPLETED
- **Status**: ✅ MOSTLY COMPLETED - MINOR REFERENCES REMAIN
- ✅ **Completed**: Removed main Gemini dependencies and modules
- ✅ **Completed**: Updated all import statements in core files
- ✅ **Completed**: Updated test files
- 🚧 **Remaining**: Some comment references and test names still mention Gemini
- 🚧 **Remaining**: Some documentation strings and error messages

#### ✅ 6.3 Update Dependencies - COMPLETED
- **Status**: ✅ COMPLETED
- ✅ **Completed**: Confirmed existing reqwest dependency has required features for OpenRouter
- ✅ **Completed**: Updated Cargo.toml comments from "Gemini API" to "OpenRouter API"
- ✅ **Completed**: All compilation successful with OpenRouter placeholders

## 🎯 IMMEDIATE NEXT STEPS

### Priority 1: Complete Phase 2 (LLM Client Implementation)
The foundation is solid, but we need to implement the actual API functionality:

1. **Implement OpenRouter API calls** in `client.rs`:
   - `generate()` method with proper OpenAI-compatible request/response
   - `generate_stream()` method with SSE parsing
   - Error handling and HTTP client configuration

2. **Implement streaming** in `streaming.rs`:
   - SSE (Server-Sent Events) parser for OpenRouter responses
   - Integration with existing reasoning-engine streaming

3. **Implement model discovery** in `models.rs`:
   - Fetch available models from `/api/v1/models`
   - Model caching and filtering capabilities

### Priority 2: Enhance GUI (Phase 3.2)
With basic UI working, enhance the user experience:
- Dynamic model dropdown with real OpenRouter models
- Search and filtering capabilities
- Provider preferences configuration

### Priority 3: Reasoning Engine Integration (Phase 4)
Ensure the reasoning engine works seamlessly with OpenRouter:
- Update adapters to work with OpenRouter client
- Test multi-step reasoning with OpenRouter models

## 🚀 WHAT'S WORKING NOW

✅ **Full Compilation**: All code compiles successfully with OpenRouter placeholders
✅ **Configuration System**: Complete OpenRouter configuration with TOML persistence
✅ **GUI Integration**: Basic settings panel with OpenRouter fields
✅ **Module Structure**: Clean OpenRouter module structure replacing Gemini
✅ **Test Framework**: All tests pass with OpenRouter configuration

## ⚠️ WHAT'S NOT WORKING YET

❌ **API Functionality**: OpenRouter client methods return placeholder errors
❌ **Model Discovery**: No actual model fetching from OpenRouter API
❌ **Streaming**: No actual SSE parsing implementation
❌ **Dynamic UI**: Model dropdown still uses text input instead of searchable list

## Implementation Details

### Key Dependencies to Add
```toml
# For OpenRouter client - ALREADY AVAILABLE
reqwest = { version = "0.11", features = ["json", "stream"] } ✅ CONFIRMED
tokio-stream = "0.1"  # May be needed for advanced streaming
futures-util = "0.3" ✅ CONFIRMED
```

### Key Dependencies to Remove
```toml
# Remove Gemini-related dependencies - RESEARCH NEEDED
google-generativeai = "0.2.0"  # or whatever version was used
```

### Model Selection UI Component
```rust
// Pseudo-code for model selection UI - TODO: IMPLEMENT
struct ModelSelector {
    available_models: Vec<OpenRouterModel>,
    filtered_models: Vec<OpenRouterModel>,
    search_query: String,
    selected_model: Option<String>,
    filter_provider: Option<String>,
    filter_capability: Option<String>,
}

impl ModelSelector {
    fn update_filter(&mut self) {
        self.filtered_models = self.available_models
            .iter()
            .filter(|model| {
                model.name.to_lowercase().contains(&self.search_query.to_lowercase())
                && self.filter_provider.as_ref()
                    .map_or(true, |provider| model.provider == *provider)
            })
            .take(50) // Limit results for performance
            .cloned()
            .collect();
    }
}
```

## Risk Mitigation

### Model Availability
- Cache popular models locally
- Fallback to default models if discovery fails
- Provider redundancy for critical models

### Performance Considerations
- Lazy loading of model list
- Efficient streaming chunk processing
- Memory-efficient model information caching

### API Reliability
- Robust error handling for API failures
- Retry logic with exponential backoff
- Circuit breaker pattern for API protection

## Success Criteria

1. **Functional**: All existing functionality works with OpenRouter
2. **Performance**: Streaming performance matches or exceeds Gemini
3. **Usability**: Easy model selection and configuration
4. **Reliability**: Robust error handling and fallback mechanisms
5. **Extensibility**: Easy to add new models and providers

## Timeline Estimate

- ✅ **Phase 1**: 1-2 days (Configuration) - **COMPLETED**
- 🚧 **Phase 2**: 4-5 days (Client Implementation) - **~1 day completed, 3-4 days remaining**
- 🎯 **Phase 3**: 2-3 days (GUI Integration) - **~1 day completed, 1-2 days remaining**
- ⏳ **Phase 4**: 2-3 days (Reasoning Engine) - **Not started**
- ⏳ **Phase 5**: 2-3 days (Testing) - **Basic tests completed, integration tests remain**
- ⏳ **Phase 6**: 1 day (Documentation/Cleanup) - **Mostly completed**

**Revised Total**: 9-14 days remaining of original 12-17 day estimate

## Next Steps

1. ✅ ~~Start with Phase 1 (Configuration Migration)~~ - **COMPLETED**
2. 🚧 **CURRENT**: Complete Phase 2 (LLM Client Implementation) - **Priority 1**
3. 🎯 **NEXT**: Enhance Phase 3 (Dynamic Model Selection) - **Priority 2**
4. ⏳ **THEN**: Implement Phase 4 (Reasoning Engine Integration) - **Priority 3**
5. ⏳ **FINALLY**: Complete Phase 5 (Testing) and Phase 6 (Documentation) 