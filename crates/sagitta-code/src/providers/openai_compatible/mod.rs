pub mod provider;
pub mod client;
pub mod translator;
pub mod types;
pub mod sse_parser;
pub mod stream_processor;
pub mod continuation;

#[cfg(test)]
mod tests;

pub use provider::OpenAICompatibleProvider;
pub use client::OpenAICompatibleClient;
pub use types::*;