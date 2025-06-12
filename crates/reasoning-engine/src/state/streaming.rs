//! Streaming state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

/// Streaming state management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingState {
    /// Active streams
    pub active_streams: HashMap<Uuid, StreamInfo>,
    /// Pending chunks waiting for processing
    pub pending_chunks: VecDeque<StreamChunk>,
    /// Stream errors encountered
    pub stream_errors: Vec<StreamError>,
    /// Backpressure signals
    pub backpressure_signals: Vec<BackpressureSignal>,
}

/// Information about an active stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Stream identifier
    pub id: Uuid,
    /// Stream type
    pub stream_type: String,
    /// When stream started
    pub started_at: DateTime<Utc>,
    /// Current state
    pub state: String,
    /// Bytes processed
    pub bytes_processed: u64,
    /// Chunks processed
    pub chunks_processed: u64,
}

/// A chunk of streaming data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Chunk identifier
    pub id: Uuid,
    /// Chunk data
    pub data: Vec<u8>,
    /// Chunk type
    pub chunk_type: String,
    /// Whether this is the final chunk
    pub is_final: bool,
}

/// Stream error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    /// Error identifier
    pub id: Uuid,
    /// Stream that errored
    pub stream_id: Uuid,
    /// Error message
    pub message: String,
    /// When error occurred
    pub timestamp: DateTime<Utc>,
    /// Whether error is recoverable
    pub recoverable: bool,
}

/// Backpressure signal for stream management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackpressureSignal {
    /// Signal identifier
    pub id: Uuid,
    /// Stream experiencing backpressure
    pub stream_id: Uuid,
    /// Severity (0.0 to 1.0)
    pub severity: f32,
    /// When signal was generated
    pub timestamp: DateTime<Utc>,
}

impl StreamingState {
    pub fn new() -> Self {
        Self {
            active_streams: HashMap::new(),
            pending_chunks: VecDeque::new(),
            stream_errors: Vec::new(),
            backpressure_signals: Vec::new(),
        }
    }
    
    pub fn add_stream(&mut self, stream_info: StreamInfo) {
        self.active_streams.insert(stream_info.id, stream_info);
    }
    
    pub fn remove_stream(&mut self, stream_id: Uuid) {
        self.active_streams.remove(&stream_id);
    }
    
    pub fn add_pending_chunk(&mut self, chunk: StreamChunk) {
        self.pending_chunks.push_back(chunk);
    }
    
    pub fn next_pending_chunk(&mut self) -> Option<StreamChunk> {
        self.pending_chunks.pop_front()
    }
}

impl Default for StreamingState {
    fn default() -> Self {
        Self::new()
    }
} 