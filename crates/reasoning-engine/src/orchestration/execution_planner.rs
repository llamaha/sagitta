use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{Result, ReasoningError};
use crate::config::OrchestrationConfig;
use super::types::{
    ToolExecutionRequest, ExecutionPlan, ExecutionPhase, ResourceAllocationPlan,
    DependencyGraph, ToolPerformanceData, ResourceUsagePattern
};

/// Plans execution of tools with dependency resolution
pub struct ExecutionPlanner {
    config: OrchestrationConfig,
    dependency_analyzer: DependencyAnalyzer,
    resource_optimizer: ResourceOptimizer,
}

/// Analyzes dependencies between tools
pub struct DependencyAnalyzer {
    /// Cache of dependency graphs
    dependency_cache: Arc<RwLock<HashMap<String, DependencyGraph>>>,
}

/// Optimizes resource allocation for tool execution
pub struct ResourceOptimizer {
    /// Historical performance data
    performance_history: Arc<RwLock<HashMap<String, ToolPerformanceData>>>,
}

impl ExecutionPlanner {
    /// Create a new execution planner
    pub async fn new(config: OrchestrationConfig) -> Result<Self> {
        Ok(Self {
            config,
            dependency_analyzer: DependencyAnalyzer::new().await?,
            resource_optimizer: ResourceOptimizer::new().await?,
        })
    }
    
    /// Create an execution plan for the given requests
    pub async fn create_plan(&self, requests: &[ToolExecutionRequest]) -> Result<ExecutionPlan> {
        let plan_id = Uuid::new_v4();
        
        // Add timeout to prevent hanging in dependency analysis
        let plan_result = tokio::time::timeout(
            Duration::from_secs(10),
            async {
                // Analyze dependencies
                let dependency_graph = self.dependency_analyzer.analyze_dependencies(requests).await?;
                
                // Create execution phases based on topological sort
                let phases = self.create_execution_phases(&dependency_graph, requests).await?;
                
                // Estimate duration
                let estimated_duration = self.estimate_total_duration(&phases).await?;
                
                // Create resource allocation plan
                let resource_plan = self.resource_optimizer.create_allocation_plan(requests, &phases).await?;
                
                Ok(ExecutionPlan {
                    id: plan_id,
                    phases,
                    estimated_duration,
                    critical_path: dependency_graph.critical_path,
                    resource_plan,
                })
            }
        ).await;
        
        match plan_result {
            Ok(result) => result,
            Err(_) => Err(ReasoningError::orchestration("Execution plan creation timed out")),
        }
    }
    
    /// Create execution phases from dependency graph
    async fn create_execution_phases(
        &self,
        graph: &DependencyGraph,
        requests: &[ToolExecutionRequest],
    ) -> Result<Vec<ExecutionPhase>> {
        let mut phases = Vec::new();
        let mut remaining_tools: HashSet<String> = graph.nodes.clone();
        let mut phase_number = 0;
        
        while !remaining_tools.is_empty() {
            let mut current_phase_tools = Vec::new();
            
            // Find tools with no remaining dependencies
            for tool in &remaining_tools {
                let has_pending_deps = graph.edges.get(tool)
                    .map(|deps| deps.iter().any(|dep| remaining_tools.contains(dep)))
                    .unwrap_or(false);
                
                if !has_pending_deps {
                    current_phase_tools.push(tool.clone());
                }
            }
            
            if current_phase_tools.is_empty() {
                return Err(ReasoningError::orchestration("Circular dependency detected"));
            }
            
            // Remove tools from remaining set
            for tool in &current_phase_tools {
                remaining_tools.remove(tool);
            }
            
            // Calculate phase duration and resources
            let estimated_duration = self.estimate_phase_duration(&current_phase_tools, requests).await?;
            let required_resources = self.calculate_phase_resources(&current_phase_tools, requests).await?;
            
            phases.push(ExecutionPhase {
                phase_number,
                tools: current_phase_tools,
                estimated_duration,
                required_resources,
            });
            
            phase_number += 1;
        }
        
        Ok(phases)
    }
    
    /// Estimate duration for a phase
    async fn estimate_phase_duration(
        &self,
        tools: &[String],
        requests: &[ToolExecutionRequest],
    ) -> Result<Duration> {
        let mut max_duration = Duration::ZERO;
        
        for tool in tools {
            if let Some(request) = requests.iter().find(|r| r.tool_name == *tool) {
                let tool_duration = request.timeout.unwrap_or(self.config.default_tool_timeout);
                max_duration = max_duration.max(tool_duration);
            }
        }
        
        Ok(max_duration)
    }
    
    /// Calculate resource requirements for a phase
    async fn calculate_phase_resources(
        &self,
        tools: &[String],
        requests: &[ToolExecutionRequest],
    ) -> Result<HashMap<String, u32>> {
        let mut resource_totals = HashMap::new();
        
        for tool in tools {
            if let Some(request) = requests.iter().find(|r| r.tool_name == *tool) {
                for resource_req in &request.required_resources {
                    let current = resource_totals.get(&resource_req.resource_type).unwrap_or(&0);
                    resource_totals.insert(
                        resource_req.resource_type.clone(),
                        current + resource_req.amount,
                    );
                }
            }
        }
        
        Ok(resource_totals)
    }
    
    /// Estimate total duration for all phases
    async fn estimate_total_duration(&self, phases: &[ExecutionPhase]) -> Result<Duration> {
        Ok(phases.iter().map(|p| p.estimated_duration).sum())
    }
}

impl DependencyAnalyzer {
    /// Create a new dependency analyzer
    pub async fn new() -> Result<Self> {
        Ok(Self {
            dependency_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Analyze dependencies for the given requests
    pub async fn analyze_dependencies(&self, requests: &[ToolExecutionRequest]) -> Result<DependencyGraph> {
        let mut nodes = HashSet::new();
        let mut edges = HashMap::new();
        
        // Build graph from requests
        for request in requests {
            nodes.insert(request.tool_name.clone());
            
            if !request.dependencies.is_empty() {
                edges.insert(request.tool_name.clone(), request.dependencies.iter().cloned().collect());
            }
        }
        
        // Perform topological sort
        let topological_order = self.topological_sort(&nodes, &edges)?;
        
        // Find critical path
        let critical_path = self.find_critical_path(&nodes, &edges);
        
        Ok(DependencyGraph {
            nodes,
            edges,
            topological_order,
            critical_path,
        })
    }
    
    /// Perform topological sort on the dependency graph using Kahn's algorithm
    fn topological_sort(&self, nodes: &HashSet<String>, edges: &HashMap<String, HashSet<String>>) -> Result<Vec<String>> {
        // Calculate in-degrees for each node
        let mut in_degree: HashMap<String, usize> = nodes.iter().map(|n| (n.clone(), 0)).collect();
        
        // Calculate actual in-degrees from the edges
        for (_node, dependencies) in edges {
            for dep in dependencies {
                // If node depends on dep, then dep has an outgoing edge to node
                // So we don't increment dep's in-degree, but we should increment node's in-degree
                // Wait, this is confusing. Let me think about this differently.
                // The edges map stores "node -> dependencies"
                // So if edges["B"] = {"A"}, it means B depends on A
                // In terms of directed graph, there should be an edge A -> B
                // So A's in-degree doesn't change, but B's in-degree increases
            }
        }
        
        // Calculate in-degrees correctly
        for (node, dependencies) in edges {
            // node depends on all items in dependencies
            // So there are edges from each dependency to node
            let current_in_degree = dependencies.len();
            in_degree.insert(node.clone(), current_in_degree);
        }
        
        // Find nodes with no incoming edges
        let mut queue: Vec<String> = in_degree.iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(node, _)| node.clone())
            .collect();
        
        let mut result = Vec::new();
        
        while let Some(node) = queue.pop() {
            result.push(node.clone());
            
            // For each node that depends on the current node, decrease its in-degree
            for (other_node, dependencies) in edges {
                if dependencies.contains(&node) {
                    let current_degree = *in_degree.get(other_node).unwrap_or(&0);
                    if current_degree > 0 {
                        in_degree.insert(other_node.clone(), current_degree - 1);
                        if current_degree - 1 == 0 {
                            queue.push(other_node.clone());
                        }
                    }
                }
            }
        }
        
        if result.len() != nodes.len() {
            return Err(ReasoningError::orchestration("Circular dependency detected"));
        }
        
        Ok(result)
    }
    
    /// Find the critical path through the dependency graph
    fn find_critical_path(&self, nodes: &HashSet<String>, edges: &HashMap<String, HashSet<String>>) -> Vec<String> {
        let mut longest_path = Vec::new();
        let mut visited = HashSet::new();
        
        for node in nodes {
            if !visited.contains(node) {
                let path = self.find_longest_path_from(node, edges, &mut visited);
                if path.len() > longest_path.len() {
                    longest_path = path;
                }
            }
        }
        
        longest_path
    }
    
    /// Find the longest path from a given node
    fn find_longest_path_from(
        &self,
        node: &str,
        edges: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<String>,
    ) -> Vec<String> {
        if visited.contains(node) {
            return vec![];
        }
        
        visited.insert(node.to_string());
        
        let mut longest_subpath = Vec::new();
        
        if let Some(deps) = edges.get(node) {
            for dep in deps {
                let subpath = self.find_longest_path_from(dep, edges, visited);
                if subpath.len() > longest_subpath.len() {
                    longest_subpath = subpath;
                }
            }
        }
        
        visited.remove(node);
        
        let mut result = vec![node.to_string()];
        result.extend(longest_subpath);
        result
    }
}

impl ResourceOptimizer {
    /// Create a new resource optimizer
    pub async fn new() -> Result<Self> {
        Ok(Self {
            performance_history: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Create a resource allocation plan
    pub async fn create_allocation_plan(
        &self,
        _requests: &[ToolExecutionRequest],
        _phases: &[ExecutionPhase],
    ) -> Result<ResourceAllocationPlan> {
        // For now, return a simple plan
        // In a full implementation, this would analyze resource conflicts,
        // optimize allocation timing, etc.
        Ok(ResourceAllocationPlan {
            peak_usage: HashMap::new(),
            allocation_timeline: Vec::new(),
            conflicts: Vec::new(),
        })
    }
} 