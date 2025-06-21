//! Memory pool implementation for tensor reuse

use crate::config::MemoryPoolConfig;
use std::sync::Arc;

/// Statistics for memory pool usage
#[derive(Debug, Clone, Default)]
pub struct MemoryPoolStats {
    pub total_allocations: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub current_pool_size: usize,
    pub total_memory_bytes: usize,
}

/// Tensor memory pool for efficient tensor reuse
#[derive(Debug)]
pub struct TensorMemoryPool {
    config: MemoryPoolConfig,
    stats: MemoryPoolStats,
}

impl TensorMemoryPool {
    /// Create a new memory pool with the given configuration
    pub fn new(config: MemoryPoolConfig) -> Self {
        Self {
            config,
            stats: MemoryPoolStats::default(),
        }
    }

    /// Get current pool statistics
    pub fn get_stats(&self) -> MemoryPoolStats {
        self.stats.clone()
    }

    /// Clear the memory pool
    pub fn clear(&mut self) {
        // Reset pool state
        self.stats.current_pool_size = 0;
        self.stats.total_memory_bytes = 0;
    }
}