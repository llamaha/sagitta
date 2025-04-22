use crate::llm::message::{AnthropicMessage, Role};
use crate::tools; // Assuming tools will have shared state eventually
use serde_json::Value; // Using Value for flexible context
use std::collections::HashMap;

/// Holds the current state shared across actions in a chain.
#[derive(Debug, Clone, Default)]
pub struct ChainState {
    /// Flexible context map for passing data between actions.
    /// Keys are strings, values can be any JSON-serializable type.
    pub context: HashMap<String, Value>,

    /// History of LLM interactions within this chain.
    pub history: Vec<AnthropicMessage>,

    /// The original user prompt or query that initiated the chain.
    pub initial_query: Option<String>,

    /// Current working directory relevant to the chain's task.
    pub current_directory: Option<String>,

    /// Name of the currently active repository context, if any.
    pub active_repository: Option<String>,
    // TODO: Add more fields as needed, e.g.,
    // - list of open files
    // - current repository context
    // - bug list
    // - technical debt items
}

impl ChainState {
    /// Creates a new, empty chain state.
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds a message to the history.
    pub fn add_history(&mut self, role: Role, content: String) {
        self.history.push(AnthropicMessage {
            role,
            content: vec![crate::llm::message::AnthropicContent::Text { text: content }],
        });
    }

    /// Sets a value in the context map.
    pub fn set_context<T: serde::Serialize>(&mut self, key: String, value: T) -> Result<(), serde_json::Error> {
        let json_value = serde_json::to_value(value)?;
        self.context.insert(key, json_value);
        Ok(())
    }

    /// Gets a value from the context map, attempting to deserialize it.
    pub fn get_context<T>(&self, key: &str) -> Option<Result<T, serde_json::Error>>
    where 
        T: serde::de::DeserializeOwned
    {
        self.context.get(key).map(|v| serde_json::from_value(v.clone()))
    }
} 