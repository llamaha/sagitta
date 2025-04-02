use anyhow::{Result, Error};
use ort::{Environment, Session, SessionBuilder, GraphOptimizationLevel, ExecutionProvider};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use lazy_static::lazy_static;

// Global ONNX environment to ensure it's only created once
lazy_static! {
    static ref ONNX_ENV: Arc<Environment> = {
        Environment::builder()
            .with_name("vectordb-onnx")
            .with_log_level(ort::LoggingLevel::Warning)
            .build()
            .expect("Failed to initialize ONNX environment")
            .into_arc()
    };
}

/// Configuration for the ONNX session
#[derive(Debug)]
pub struct SessionConfig {
    /// Maximum number of sessions to keep in the pool
    pub max_pool_size: usize,
    /// Number of threads to use for inference
    pub num_threads: i16,
    /// Whether to use hardware acceleration if available
    pub use_cuda: bool,
    /// Optimization level for the model (as u8 for clonability)
    pub optimization_level: u8,
    /// Session timeout - sessions idle longer than this will be recreated
    pub session_timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_pool_size: num_cpus::get(),
            num_threads: num_cpus::get() as i16,
            use_cuda: true,
            optimization_level: 1, // Level1
            session_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl Clone for SessionConfig {
    fn clone(&self) -> Self {
        Self {
            max_pool_size: self.max_pool_size,
            num_threads: self.num_threads,
            use_cuda: self.use_cuda,
            optimization_level: self.optimization_level,
            session_timeout: self.session_timeout,
        }
    }
}

/// A pooled ONNX session with metadata
struct PooledSession {
    /// The actual ONNX session
    session: Session,
    /// When this session was last used
    last_used: Instant,
}

/// Manager for ONNX runtime sessions
pub struct SessionManager {
    /// The session pool
    pool: Mutex<VecDeque<PooledSession>>,
    /// Configuration for creating sessions
    config: SessionConfig,
    /// Path to the model
    model_path: PathBuf,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(model_path: &Path, config: SessionConfig) -> Result<Arc<Self>> {
        // Validate that the model exists
        if !model_path.exists() {
            return Err(Error::msg(format!(
                "Model file not found: {}",
                model_path.display()
            )));
        }

        // Create the session manager
        let manager = Arc::new(Self {
            pool: Mutex::new(VecDeque::with_capacity(config.max_pool_size)),
            config,
            model_path: model_path.to_path_buf(),
        });

        // Create a session for the pool
        let session = manager.create_session()?;
        
        // Add the session to the pool by cloning the Arc
        let manager_clone = Arc::clone(&manager);
        let mut pool = manager_clone.pool.lock().unwrap();
        pool.push_back(PooledSession {
            session,
            last_used: Instant::now(),
        });

        Ok(manager)
    }

    /// Get optimization level from config
    fn get_optimization_level(&self) -> GraphOptimizationLevel {
        match self.config.optimization_level {
            1 => GraphOptimizationLevel::Level1,
            2 => GraphOptimizationLevel::Level2,
            3 => GraphOptimizationLevel::Level3,
            _ => GraphOptimizationLevel::Level1, // Default to Level1 for unknown values
        }
    }

    /// Create a new ONNX session
    fn create_session(&self) -> Result<Session> {
        // Try with CUDA first if enabled
        if self.config.use_cuda {
            // Get the optimization level for CUDA
            let opt_level = self.get_optimization_level();
            let providers = vec![ExecutionProvider::CUDA(Default::default())];
            match SessionBuilder::new(&ONNX_ENV)
                .and_then(|b| b.with_optimization_level(opt_level))
                .and_then(|b| b.with_intra_threads(self.config.num_threads))
                .and_then(|b| b.with_execution_providers(providers))
                .and_then(|b| b.with_model_from_file(&self.model_path)) {
                Ok(session) => return Ok(session),
                Err(e) => eprintln!("Failed to create CUDA session, falling back to CPU: {}", e),
            }
        }

        // Fallback to CPU with a fresh optimization level
        let opt_level = self.get_optimization_level();
        let session = SessionBuilder::new(&ONNX_ENV)?
            .with_optimization_level(opt_level)?
            .with_intra_threads(self.config.num_threads)?
            .with_model_from_file(&self.model_path)?;

        Ok(session)
    }

    /// Get a session from the pool or create a new one
    pub fn get_session(&self) -> Result<Session> {
        // Try to get a session from the pool
        let mut pool = self.pool.lock().unwrap();
        
        // Check if we have any sessions in the pool
        if let Some(pooled) = pool.pop_front() {
            // Check if the session has timed out
            if pooled.last_used.elapsed() < self.config.session_timeout {
                return Ok(pooled.session);
            }
            // Session timed out, let it drop and create a new one
        }
        
        // No session available, create a new one
        self.create_session()
    }

    /// Return a session to the pool
    pub fn return_session(&self, session: Session) {
        let mut pool = self.pool.lock().unwrap();
        
        // Only add to the pool if we haven't reached the maximum size
        if pool.len() < self.config.max_pool_size {
            pool.push_back(PooledSession {
                session,
                last_used: Instant::now(),
            });
        }
        // Otherwise, let the session drop
    }

    /// Get a session guard that automatically returns the session to the pool
    pub fn get_session_guard(&self) -> Result<SessionGuard> {
        let session = self.get_session()?;
        Ok(SessionGuard {
            session: Some(session),
            manager: self,
        })
    }
}

/// A RAII guard for a session that automatically returns it to the pool when dropped
pub struct SessionGuard<'a> {
    /// The session, wrapped in an Option so we can take() it in drop
    session: Option<Session>,
    /// The session manager that created this session
    manager: &'a SessionManager,
}

impl<'a> SessionGuard<'a> {
    /// Get a reference to the underlying session
    pub fn session(&self) -> &Session {
        self.session.as_ref().unwrap()
    }
}

impl<'a> Drop for SessionGuard<'a> {
    fn drop(&mut self) {
        // Take the session out of the Option to avoid a clone
        if let Some(session) = self.session.take() {
            self.manager.return_session(session);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_session_creation() {
        // Skip if ONNX model isn't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        if !model_path.exists() {
            println!("Skipping test_session_creation because model file isn't available");
            return;
        }
        
        // Create a session manager with default config
        let config = SessionConfig::default();
        let manager = SessionManager::new(&model_path, config);
        assert!(manager.is_ok());
        
        // Get a session from the manager
        let manager = manager.unwrap();
        let session = manager.get_session();
        assert!(session.is_ok());
    }
    
    #[test]
    fn test_session_pooling() {
        // Skip if ONNX model isn't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        if !model_path.exists() {
            println!("Skipping test_session_pooling because model file isn't available");
            return;
        }
        
        // Create a session manager with a pool size of 2
        let mut config = SessionConfig::default();
        config.max_pool_size = 2;
        let manager = SessionManager::new(&model_path, config).unwrap();
        
        // Get 3 sessions
        let session1 = manager.get_session().unwrap();
        let session2 = manager.get_session().unwrap();
        let session3 = manager.get_session().unwrap();
        
        // Return them to the pool
        manager.return_session(session3);
        manager.return_session(session2);
        manager.return_session(session1);
        
        // The pool should now have 2 sessions (session2 and session1)
        // session3 should have been dropped
        assert_eq!(manager.pool.lock().unwrap().len(), 2);
    }
    
    #[test]
    fn test_session_guard() {
        // Skip if ONNX model isn't available
        let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
        if !model_path.exists() {
            println!("Skipping test_session_guard because model file isn't available");
            return;
        }
        
        // Create a session manager with default config
        let config = SessionConfig::default();
        let manager = SessionManager::new(&model_path, config).unwrap();
        
        // Get a session guard
        let guard = manager.get_session_guard();
        assert!(guard.is_ok());
        
        // Use the session through the guard
        {
            let guard = guard.unwrap();
            let _session = guard.session();
            // guard is dropped here and session is returned to the pool
        }
        
        // The pool should now have 1 session
        assert_eq!(manager.pool.lock().unwrap().len(), 1);
    }
} 