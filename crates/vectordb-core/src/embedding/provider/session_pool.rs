use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, AtomicBool, Ordering},
    time::Instant,
    sync::{Mutex, Arc},
    fmt::Debug,
    thread,
};
use crate::error::{Result, VectorDBError};
use super::{onnx::OnnxEmbeddingModel, EmbeddingProvider, EmbeddingModelType};

/// A pool of ONNX model sessions for efficient parallel processing
pub struct OnnxSessionPool {
    sessions: Vec<OnnxEmbeddingModel>,
    current_index: AtomicUsize,
    max_sessions: usize,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    last_used: Instant,
    connection_status: AtomicBool,
}

impl Clone for OnnxSessionPool {
    fn clone(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            current_index: AtomicUsize::new(self.current_index.load(Ordering::SeqCst)),
            max_sessions: self.max_sessions,
            model_path: self.model_path.clone(),
            tokenizer_path: self.tokenizer_path.clone(),
            last_used: Instant::now(), // Reset last_used for the clone
            connection_status: AtomicBool::new(self.connection_status.load(Ordering::SeqCst)),
        }
    }
}

impl Debug for OnnxSessionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSessionPool")
            .field("session_count", &self.sessions.len())
            .field("max_sessions", &self.max_sessions)
            .field("model_path", &self.model_path)
            .field("tokenizer_path", &self.tokenizer_path)
            .field("last_used", &self.last_used)
            .field("connection_status", &self.connection_status.load(Ordering::SeqCst))
            .finish()
    }
}

impl EmbeddingProvider for OnnxSessionPool {
    fn dimension(&self) -> usize {
        // Get dimension from the first session
        self.sessions.first()
            .map(|s| s.dimension())
            .unwrap_or(0)
    }

    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Onnx
    }

    fn embed_batch<'a>(&self, texts: &[&'a str]) -> Result<Vec<Vec<f32>>> {
        let session = self.get_session()?;
        session.embed_batch(texts)
    }
}

impl OnnxSessionPool {
    /// Creates a new session pool with the specified number of sessions
    pub fn new(
        model_path: PathBuf,
        tokenizer_path: PathBuf,
        max_sessions: usize
    ) -> Result<Self> {
        // Initialize pool with all sessions pre-allocated
        let mut sessions = Vec::with_capacity(max_sessions);
        for _ in 0..max_sessions {
            sessions.push(OnnxEmbeddingModel::new(&model_path, &tokenizer_path)?);
        }
        
        Ok(Self {
            sessions,
            current_index: AtomicUsize::new(0),
            max_sessions,
            model_path,
            tokenizer_path,
            last_used: Instant::now(),
            connection_status: AtomicBool::new(true),
        })
    }

    /// Gets the next available session in a round-robin fashion
    pub fn get_session(&self) -> Result<&OnnxEmbeddingModel> {
        let index = self.current_index.fetch_add(1, Ordering::SeqCst) % self.sessions.len();
        Ok(&self.sessions[index])
    }

    /// Ensures the connection is active and reconnects if necessary
    pub fn ensure_connection(&mut self) -> Result<()> {
        if !self.connection_status.load(Ordering::SeqCst) {
            self.reconnect()?;
        }
        Ok(())
    }

    /// Attempts to reconnect all sessions in the pool
    fn reconnect(&mut self) -> Result<()> {
        log::info!("Attempting to reconnect ONNX sessions...");
        let mut new_sessions = Vec::with_capacity(self.max_sessions);
        
        for _ in 0..self.max_sessions {
            match OnnxEmbeddingModel::new(&self.model_path, &self.tokenizer_path) {
                Ok(session) => new_sessions.push(session),
                Err(e) => {
                    log::error!("Failed to create new ONNX session during reconnect: {}", e);
                    return Err(VectorDBError::EmbeddingError(format!(
                        "Failed to reconnect ONNX sessions: {}", e
                    )));
                }
            }
        }

        self.sessions = new_sessions;
        self.connection_status.store(true, Ordering::SeqCst);
        self.last_used = Instant::now();
        log::info!("Successfully reconnected ONNX sessions");
        Ok(())
    }

    /// Returns the number of active sessions in the pool
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns the maximum number of sessions allowed in the pool
    pub fn max_sessions(&self) -> usize {
        self.max_sessions
    }
}

/// A thread-safe wrapper for OnnxSessionPool that provides thread-local pools
pub struct ThreadSafeSessionPool {
    thread_pools: Arc<Mutex<Vec<OnnxSessionPool>>>,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    sessions_per_thread: usize,
}

impl ThreadSafeSessionPool {
    pub fn new(
        model_path: PathBuf,
        tokenizer_path: PathBuf,
        sessions_per_thread: usize,
    ) -> Result<Self> {
        Ok(Self {
            thread_pools: Arc::new(Mutex::new(Vec::new())),
            model_path,
            tokenizer_path,
            sessions_per_thread,
        })
    }

    pub fn get_pool(&self) -> Result<OnnxSessionPool> {
        let thread_id = thread::current().id();
        let mut pools = self.thread_pools.lock().unwrap();
        
        // Try to find existing pool for this thread
        for pool in pools.iter() {
            if pool.session_count() > 0 {
                return Ok(pool.clone());
            }
        }

        // Create new pool if none exists
        let new_pool = OnnxSessionPool::new(
            self.model_path.clone(),
            self.tokenizer_path.clone(),
            self.sessions_per_thread,
        )?;
        let pool_clone = new_pool.clone();
        pools.push(new_pool);
        Ok(pool_clone)
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let pool = self.get_pool()?;
        pool.embed_batch(texts)
    }
}

impl Clone for ThreadSafeSessionPool {
    fn clone(&self) -> Self {
        Self {
            thread_pools: self.thread_pools.clone(),
            model_path: self.model_path.clone(),
            tokenizer_path: self.tokenizer_path.clone(),
            sessions_per_thread: self.sessions_per_thread,
        }
    }
}

impl Debug for ThreadSafeSessionPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadSafeSessionPool")
            .field("sessions_per_thread", &self.sessions_per_thread)
            .field("model_path", &self.model_path)
            .field("tokenizer_path", &self.tokenizer_path)
            .finish()
    }
}

impl EmbeddingProvider for ThreadSafeSessionPool {
    fn dimension(&self) -> usize {
        // Get dimension from the first available pool
        if let Ok(pool) = self.get_pool() {
            pool.dimension()
        } else {
            0
        }
    }

    fn model_type(&self) -> EmbeddingModelType {
        EmbeddingModelType::Onnx
    }

    fn embed_batch<'a>(&self, texts: &[&'a str]) -> Result<Vec<Vec<f32>>> {
        self.embed_batch(texts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_session_pool_creation() {
        let temp_dir = tempdir().unwrap();
        let model_path = temp_dir.path().join("model.onnx");
        let tokenizer_path = temp_dir.path().join("tokenizer.json");
        
        // Create dummy files for testing
        fs::write(&model_path, "dummy model").unwrap();
        fs::write(&tokenizer_path, "dummy tokenizer").unwrap();

        let pool = OnnxSessionPool::new(
            model_path,
            tokenizer_path,
            2
        );

        assert!(pool.is_ok());
        let pool = pool.unwrap();
        assert_eq!(pool.session_count(), 2); // Now creates all sessions upfront
        assert_eq!(pool.max_sessions(), 2);
    }

    #[test]
    fn test_session_pool_round_robin() {
        let temp_dir = tempdir().unwrap();
        let model_path = temp_dir.path().join("model.onnx");
        let tokenizer_path = temp_dir.path().join("tokenizer.json");
        
        // Create dummy files for testing
        fs::write(&model_path, "dummy model").unwrap();
        fs::write(&tokenizer_path, "dummy tokenizer").unwrap();

        let pool = OnnxSessionPool::new(
            model_path,
            tokenizer_path,
            2
        ).unwrap();

        // Test round-robin session selection
        let session1 = pool.get_session().unwrap();
        let session2 = pool.get_session().unwrap();
        let session3 = pool.get_session().unwrap();

        assert_eq!(session1 as *const _, session3 as *const _);
        assert_ne!(session1 as *const _, session2 as *const _);
    }
} 