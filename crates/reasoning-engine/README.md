# Reasoning Engine Crate (`reasoning-engine`)

## Overview

The `reasoning-engine` crate provides a sophisticated and extensible framework for powering AI agents that can perform complex, multi-step tasks. It enables an agent to understand user requests, interact with Large Language Models (LLMs), execute tools, manage state, and handle streaming output, all orchestrated through a central processing loop.

A key feature is its iterative nature, allowing for sequences of LLM interactions and tool executions to fulfill complex user goals. The engine is designed to be modular, relying on traits for external dependencies like LLM clients, tool execution, and semantic intent analysis, making it adaptable to different agent implementations.

## Core Components and Architecture

### 1. `ReasoningEngine<LC, IA>`
This is the central orchestrator, found in `src/lib.rs`. It drives the main processing loop.
- **Generics**: 
    - `LC: LlmClient + 'static`: A client for interacting with an LLM.
    - `IA: IntentAnalyzer + 'static`: A component for analyzing the semantic intent of LLM text responses.
- **Responsibilities**:
    - Managing the overall reasoning session.
    - Maintaining conversation history for the LLM.
    - Iteratively calling the LLM.
    - Handling LLM responses, including streaming text and parsing tool call requests.
    - Coordinating tool execution via the `ToolOrchestrator`.
    - Using the `IntentAnalyzer` to understand LLM responses that don't explicitly call tools, deciding whether to nudge for action or conclude an interaction.
    - Managing and updating `ReasoningState`.
    - Emitting events via an `EventEmitter`.
    - Handling streaming output via a `StreamHandler`.

### 2. Traits (`src/traits.rs`)
The engine relies on several traits that must be implemented by the consuming agent (e.g., `sagitta-code`):
- **`LlmClient`**: Defines the interface for making calls to an LLM (e.g., `generate_stream`). It uses `LlmMessage` and `LlmStreamChunk` (which supports text and structured `ToolCall` data) for communication.
- **`ToolExecutor`**: Defines how tools are executed (`execute_tool`) and how their definitions (`ToolDefinition`) are retrieved.
- **`StreamHandler`**: Defines how incoming stream chunks (e.g., from the LLM) are processed and passed to the end-user or UI.
- **`EventEmitter`**: Allows the engine to emit `ReasoningEvent`s about its progress and state changes.
- **`IntentAnalyzer`**: A crucial trait for advanced conversational flow. Implementations analyze LLM text responses to determine the `DetectedIntent` (e.g., `ProvidesFinalAnswer`, `ProvidesPlanWithoutExplicitAction`, `AsksClarifyingQuestion`). This informs the `ReasoningEngine`'s decision-making when an LLM doesn't explicitly request a tool.
- **`StatePersistence`**: For saving and loading `ReasoningState` (optional, for long-term memory or session recovery).
- **`MetricsCollector`**: For collecting operational metrics (optional).

### 3. Main Processing Loop (`ReasoningEngine::process`)
This method is the heart of the engine:
1.  **Initialization**: Takes the full conversation history (`Vec<LlmMessage>`) as input. Initializes `ReasoningState` and internal LLM conversation history.
2.  **Initial Tool Call (Optional but common)**: Typically, an initial tool like `analyze_input` is called via the `ToolOrchestrator` to preprocess the user's latest request. Results are added to the LLM history.
3.  **Iterative Loop (LLM & Tools)**: The engine then enters a loop, capped by `config.max_iterations`:
    a.  **LLM Call**: The current `llm_conversation_history` is sent to the `LlmClient`.
    b.  **Response Handling**: The LLM's response stream is processed:
        i.  Textual parts (`LlmStreamChunk::Text`) are sent to the `StreamHandler`.
        ii. Structured tool call requests (`LlmStreamChunk::ToolCall`) are collected.
    c.  **History Update**: The LLM's textual response is added to `llm_conversation_history`.
    d.  **Decision Point (Intent Analysis / Tool Calls)**:
        i.  **If Tool Calls Requested**: The `ToolOrchestrator` is invoked to execute the requested tools. Results are added to `llm_conversation_history` (currently as "user" messages with textual results), and the loop continues.
        ii. **If No Tool Calls (Text Only Response)**: The `IntentAnalyzer` is called with the LLM's text.
            - If intent is `ProvidesFinalAnswer`, `AsksClarifyingQuestion`, `GeneralConversation`, etc., the loop terminates.
            - If intent is `ProvidesPlanWithoutExplicitAction` (and iterations allow), a "nudge" message is added to history, prompting the LLM to proceed with a tool call, and the loop continues.
            - If intent is `Ambiguous` or analysis fails, the loop typically terminates.
    e.  **Loop Termination**: The loop also breaks on errors, max iterations, or successful completion.
4.  **Final State**: Returns the final `ReasoningState`.

### 4. State Management (`src/state.rs`)
- **`ReasoningState`**: Holds all information about the current reasoning session, including context, history of steps, goals, confidence, etc.
- **`ReasoningStep`**: Represents a single discrete step in the reasoning process (e.g., an LLM call, a tool execution).
- Helper methods exist on these structs for managing and updating state (e.g., `add_step`, `set_completed`).

### 5. Tool Orchestration (`src/orchestration.rs`)
- **`ToolOrchestrator`**: Manages the execution of one or more tools, including handling dependencies (future), resource management (future), and retries.
- Takes `ToolExecutionRequest`s and uses a `ToolExecutor` implementation.

### 6. Configuration (`src/config.rs`)
- **`ReasoningConfig`**: A comprehensive struct allowing fine-tuning of various engine parameters (timeouts, iteration limits, thresholds for different components like streaming, graph, decision, orchestration, etc.).

### 7. Error Handling (`src/error.rs`)
- **`ReasoningError`**: A detailed enum defining various errors that can occur within the engine, using `thiserror` for easy error propagation and display.

## Integration with an Agent (e.g., Sagitta Code)

The `reasoning-engine` is designed to be a library. A concrete AI agent application (like `sagitta-code`) uses it by:
1.  **Providing Implementations for Traits**: The agent implements `LlmClient` (e.g., `ReasoningLlmClientAdapter` wrapping a `GeminiClient`), `ToolExecutor` (e.g., `AgentToolExecutor` using its `ToolRegistry`), `StreamHandler`, `EventEmitter`, and critically, the `IntentAnalyzer` (e.g., `SagittaCodeIntentAnalyzer` using `sagitta-search`'s embedding models).
2.  **Configuration**: The agent loads a `ReasoningConfig` and initializes the `ReasoningEngine` with it, along with the trait implementations.
3.  **Driving the Engine**: When the agent receives a user message, it prepares the full conversation history (as `Vec<LlmMessage>`) and calls `ReasoningEngine::process()`.
4.  **Handling Output**: The agent processes events from the `EventEmitter` and stream chunks from the `StreamHandler` to update its UI or interact with the user.

## How Semantic Intent Analysis Works (High-Level)

1.  The `ReasoningEngine` is initialized with an `IntentAnalyzer` implementation (e.g., `SagittaCodeIntentAnalyzer` from `sagitta-code`).
2.  `SagittaCodeIntentAnalyzer` (during its own initialization) loads an embedding model (e.g., an ONNX model via `sagitta_search`'s `ThreadSafeOnnxProvider`).
3.  It also pre-embeds a set of "prototype phrases" corresponding to different `DetectedIntent` enums (e.g., "The task is complete" -> `ProvidesFinalAnswer`).
4.  During `ReasoningEngine::process`, if the LLM returns text but no explicit tool calls, this text is passed to `SagittaCodeIntentAnalyzer::analyze_intent()`.
5.  `SagittaCodeIntentAnalyzer` embeds the LLM's text using the ONNX model.
6.  It then calculates the semantic similarity (e.g., cosine similarity) between the LLM text embedding and all pre-computed prototype embeddings.
7.  The `DetectedIntent` of the prototype with the highest similarity (above a certain threshold) is returned to the `ReasoningEngine`.
8.  The `ReasoningEngine` uses this `DetectedIntent` to make a more informed decision (e.g., nudge for action if `ProvidesPlanWithoutExplicitAction`, terminate if `ProvidesFinalAnswer` or `AsksClarifyingQuestion`).

This allows the engine to move beyond simple keyword spotting for its nudge/termination logic, leading to more natural and robust conversational interactions.

## Future Directions
- More sophisticated plan parsing and execution based on LLM output.
- Enhanced dependency management in `ToolOrchestrator`.
- Integration with graph-based execution for more complex reasoning flows (`graph.rs`).
- Advanced backtracking and reflection capabilities (`backtracking.rs`, `reflection.rs`).

This README provides a snapshot of the current architecture. Refer to the source code and inline documentation for more detailed information on specific modules and functions. 