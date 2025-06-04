//! ONNX-based embedding provider implementation.

pub mod model;
pub mod session_pool;
pub mod memory_pool;
pub mod io_binding;

// Re-export main types
pub use model::OnnxEmbeddingModel;
pub use session_pool::OnnxSessionPool; 