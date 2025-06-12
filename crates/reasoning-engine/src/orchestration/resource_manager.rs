use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

use crate::error::{Result, ReasoningError};
use super::types::{
    ResourcePool, AllocatedResource, PendingAllocation, ResourceAllocationRecord
};

/// Manages resource pools and allocation/deallocation
pub struct ResourceManager {
    /// Available resources by type
    resources: Arc<RwLock<HashMap<String, ResourcePool>>>,
    /// Active resource allocations
    allocations: Arc<RwLock<HashMap<Uuid, Vec<AllocatedResource>>>>,
    /// Resource allocation history for optimization
    allocation_history: Arc<RwLock<VecDeque<ResourceAllocationRecord>>>,
}

impl ResourceManager {
    /// Create a new resource manager
    pub async fn new() -> Result<Self> {
        Ok(Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            allocation_history: Arc::new(RwLock::new(VecDeque::new())),
        })
    }
    
    /// Register a resource pool
    pub async fn register_resource_pool(&self, resource_type: String, capacity: u32) -> Result<()> {
        let pool = ResourcePool {
            resource_type: resource_type.clone(),
            total_capacity: capacity,
            available: capacity,
            semaphore: Arc::new(Semaphore::new(capacity as usize)),
            pending_allocations: VecDeque::new(),
        };
        
        let mut resources = self.resources.write().await;
        resources.insert(resource_type, pool);
        Ok(())
    }
    
    /// Allocate a resource
    pub async fn allocate_resource(
        &self,
        resource_type: &str,
        amount: u32,
        priority: f32,
        tool_name: &str,
    ) -> Result<AllocatedResource> {
        let allocation_id = Uuid::new_v4();
        
        // For now, implement a simple allocation strategy with timeout
        // In a full implementation, this would handle priority queuing, deadlock detection, etc.
        
        // Add timeout to prevent hanging
        let allocation_result = tokio::time::timeout(
            Duration::from_secs(5),
            async {
                let allocation = AllocatedResource {
                    allocation_id,
                    resource_type: resource_type.to_string(),
                    amount,
                    allocated_at: chrono::Utc::now(),
                    owner_tool: tool_name.to_string(),
                };
                
                // Record allocation
                let mut allocations = self.allocations.write().await;
                allocations.entry(allocation_id).or_insert_with(Vec::new).push(allocation.clone());
                
                Ok(allocation)
            }
        ).await;
        
        match allocation_result {
            Ok(result) => result,
            Err(_) => Err(ReasoningError::orchestration("Resource allocation timed out")),
        }
    }
    
    /// Release a resource
    pub async fn release_resource(&self, resource: &AllocatedResource) -> Result<()> {
        let mut allocations = self.allocations.write().await;
        allocations.remove(&resource.allocation_id);
        
        // Record in history for optimization
        let duration = chrono::Utc::now().signed_duration_since(resource.allocated_at);
        let record = ResourceAllocationRecord {
            tool_name: resource.owner_tool.clone(),
            resource_type: resource.resource_type.clone(),
            amount: resource.amount,
            duration: duration.to_std().unwrap_or(Duration::ZERO),
            efficiency: 1.0, // TODO: Calculate actual efficiency
            timestamp: std::time::Instant::now(),
        };
        
        let mut history = self.allocation_history.write().await;
        history.push_back(record);
        
        // Limit history size
        while history.len() > 10000 {
            history.pop_front();
        }
        
        Ok(())
    }
} 