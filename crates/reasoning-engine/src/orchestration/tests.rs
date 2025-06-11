use super::*;
use crate::traits::{ToolDefinition};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Mock tool executor for testing
struct MockToolExecutor {
    call_count: AtomicUsize,
    should_fail: bool,
    execution_delay: Duration,
}

impl MockToolExecutor {
    fn new(should_fail: bool, execution_delay: Duration) -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            should_fail,
            execution_delay,
        }
    }
    
    fn get_call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl crate::traits::ToolExecutor for MockToolExecutor {
    async fn execute_tool(&self, name: &str, args: serde_json::Value) -> crate::error::Result<crate::traits::ToolResult> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        
        // Simulate execution delay
        tokio::time::sleep(self.execution_delay).await;
        
        if self.should_fail {
            Err(crate::error::ReasoningError::tool_execution(name, "Mock tool failure"))
        } else {
            Ok(crate::traits::ToolResult::success(
                serde_json::json!({"tool": name, "args": args}),
                self.execution_delay.as_millis() as u64
            ))
        }
    }
    
    async fn get_available_tools(&self) -> crate::error::Result<Vec<ToolDefinition>> {
        Ok(vec![
            ToolDefinition {
                name: "test_tool_1".to_string(),
                description: "Test tool 1".to_string(),
                parameters: serde_json::json!({}),
                is_required: false,
                category: None,
                estimated_duration_ms: Some(100),
            },
            ToolDefinition {
                name: "test_tool_2".to_string(),
                description: "Test tool 2".to_string(),
                parameters: serde_json::json!({}),
                is_required: false,
                category: None,
                estimated_duration_ms: Some(200),
            },
        ])
    }
}

// Mock event emitter for testing
struct MockEventEmitter {
    events: Arc<tokio::sync::Mutex<Vec<crate::traits::ReasoningEvent>>>,
}

impl MockEventEmitter {
    fn new() -> Self {
        Self {
            events: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
    
    async fn get_events(&self) -> Vec<crate::traits::ReasoningEvent> {
        self.events.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl crate::traits::EventEmitter for MockEventEmitter {
    async fn emit_event(&self, event: crate::traits::ReasoningEvent) -> crate::error::Result<()> {
        self.events.lock().await.push(event);
        Ok(())
    }
}

#[tokio::test]
async fn test_orchestrator_creation() {
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(5); // Shorter for tests
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    let metrics = orchestrator.get_metrics().await;
    assert_eq!(metrics.total_orchestrations, 0);
}

#[tokio::test]
async fn test_single_tool_orchestration() {
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(10); // Shorter for tests
    config.default_tool_timeout = Duration::from_secs(2);
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(50)));
    let event_emitter = Arc::new(MockEventEmitter::new());
    
    let request = ToolExecutionRequest::new(
        "test_tool_1".to_string(),
        serde_json::json!({"param": "value"})
    );
    
    let result = orchestrator.orchestrate_tools(
        vec![request],
        tool_executor.clone(),
        event_emitter.clone(),
    ).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.successful_tools, 1);
    assert_eq!(result.failed_tools, 0);
    assert_eq!(tool_executor.get_call_count(), 1);
    
    // Check events were emitted
    let events = event_emitter.get_events().await;
    assert!(!events.is_empty());
}

#[tokio::test]
async fn test_parallel_tool_orchestration() {
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(15); // Shorter for tests
    config.default_tool_timeout = Duration::from_secs(3);
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(100)));
    let event_emitter = Arc::new(MockEventEmitter::new());
    
    let requests = vec![
        ToolExecutionRequest::new("tool_1".to_string(), serde_json::json!({})),
        ToolExecutionRequest::new("tool_2".to_string(), serde_json::json!({})),
        ToolExecutionRequest::new("tool_3".to_string(), serde_json::json!({})),
    ];
    
    let start_time = Instant::now();
    let result = orchestrator.orchestrate_tools(
        requests,
        tool_executor.clone(),
        event_emitter,
    ).await.unwrap();
    let execution_time = start_time.elapsed();
    
    assert!(result.success);
    assert_eq!(result.successful_tools, 3);
    assert_eq!(tool_executor.get_call_count(), 3);
    
    // Should execute in parallel, so total time should be close to individual tool time
    assert!(execution_time < Duration::from_millis(250)); // Some buffer for overhead
}

#[tokio::test]
async fn test_dependency_orchestration() {
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(15); // Shorter for tests
    config.default_tool_timeout = Duration::from_secs(2);
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(50)));
    let event_emitter = Arc::new(MockEventEmitter::new());
    
    let requests = vec![
        ToolExecutionRequest::new("tool_1".to_string(), serde_json::json!({})),
        ToolExecutionRequest::new("tool_2".to_string(), serde_json::json!({}))
            .with_dependency("tool_1".to_string()),
        ToolExecutionRequest::new("tool_3".to_string(), serde_json::json!({}))
            .with_dependency("tool_2".to_string()),
    ];
    
    let result = orchestrator.orchestrate_tools(
        requests,
        tool_executor.clone(),
        event_emitter,
    ).await.unwrap();
    
    assert!(result.success);
    assert_eq!(result.successful_tools, 3);
    assert_eq!(result.execution_plan.phases.len(), 3); // Should be 3 phases due to dependencies
}

#[tokio::test]
async fn test_tool_failure_handling() {
    // Create config with shorter timeouts for testing
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(30); // Increased for debugging
    config.default_tool_timeout = Duration::from_secs(5);
    config.max_retry_attempts = 1; // Fewer retries for faster tests
    
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    let tool_executor = Arc::new(MockToolExecutor::new(true, Duration::from_millis(10))); // Will fail
    let event_emitter = Arc::new(MockEventEmitter::new());
    
    let request = ToolExecutionRequest::new(
        "failing_tool".to_string(),
        serde_json::json!({})
    );
    
    let result = orchestrator.orchestrate_tools(
        vec![request],
        tool_executor,
        event_emitter,
    ).await.unwrap();
    
    assert!(!result.success);
    assert_eq!(result.successful_tools, 0);
    assert_eq!(result.failed_tools, 1);
    assert_eq!(result.skipped_tools, 0);
}

#[tokio::test]
async fn test_resource_management() {
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(15); // Shorter for tests
    config.default_tool_timeout = Duration::from_secs(2);
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    // Register a resource pool
    orchestrator.resource_manager.register_resource_pool(
        "test_resource".to_string(),
        2 // Only 2 units available
    ).await.unwrap();
    
    let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(100)));
    let event_emitter = Arc::new(MockEventEmitter::new());
    
    let requests = vec![
        ToolExecutionRequest::new("tool_1".to_string(), serde_json::json!({}))
            .with_resource("test_resource".to_string(), 1, false),
        ToolExecutionRequest::new("tool_2".to_string(), serde_json::json!({}))
            .with_resource("test_resource".to_string(), 1, false),
        ToolExecutionRequest::new("tool_3".to_string(), serde_json::json!({}))
            .with_resource("test_resource".to_string(), 1, false),
    ];
    
    let result = orchestrator.orchestrate_tools(
        requests,
        tool_executor,
        event_emitter,
    ).await.unwrap();
    
    // All tools should complete despite resource constraints
    assert!(result.success);
    assert_eq!(result.successful_tools, 3);
}

#[tokio::test]
async fn test_execution_request_builder() {
    let request = ToolExecutionRequest::new("test".to_string(), serde_json::json!({}))
        .with_dependency("dep1".to_string())
        .with_resource("cpu".to_string(), 2, false)
        .with_priority(0.8)
        .with_timeout(Duration::from_secs(10))
        .as_critical();
    
    assert_eq!(request.tool_name, "test");
    assert_eq!(request.dependencies, vec!["dep1"]);
    assert_eq!(request.required_resources.len(), 1);
    assert_eq!(request.priority, 0.8);
    assert_eq!(request.timeout, Some(Duration::from_secs(10)));
    assert!(request.is_critical);
}

#[tokio::test]
async fn test_dependency_analyzer() {
    let analyzer = DependencyAnalyzer::new().await.unwrap();
    
    let requests = vec![
        ToolExecutionRequest::new("A".to_string(), serde_json::json!({})),
        ToolExecutionRequest::new("B".to_string(), serde_json::json!({}))
            .with_dependency("A".to_string()),
        ToolExecutionRequest::new("C".to_string(), serde_json::json!({}))
            .with_dependency("A".to_string()),
        ToolExecutionRequest::new("D".to_string(), serde_json::json!({}))
            .with_dependency("B".to_string())
            .with_dependency("C".to_string()),
    ];
    
    let graph = analyzer.analyze_dependencies(&requests).await.unwrap();
    
    assert_eq!(graph.nodes.len(), 4);
    assert!(graph.nodes.contains("A"));
    assert!(graph.nodes.contains("B"));
    assert!(graph.nodes.contains("C"));
    assert!(graph.nodes.contains("D"));
    
    // Check topological order is valid
    let topo_order = &graph.topological_order;
    let a_pos = topo_order.iter().position(|x| x == "A").unwrap();
    let b_pos = topo_order.iter().position(|x| x == "B").unwrap();
    let c_pos = topo_order.iter().position(|x| x == "C").unwrap();
    let d_pos = topo_order.iter().position(|x| x == "D").unwrap();
    
    assert!(a_pos < b_pos); // A comes before B
    assert!(a_pos < c_pos); // A comes before C
    assert!(b_pos < d_pos); // B comes before D
    assert!(c_pos < d_pos); // C comes before D
}

#[tokio::test]
async fn test_circular_dependency_detection() {
    let analyzer = DependencyAnalyzer::new().await.unwrap();
    
    let requests = vec![
        ToolExecutionRequest::new("A".to_string(), serde_json::json!({}))
            .with_dependency("B".to_string()),
        ToolExecutionRequest::new("B".to_string(), serde_json::json!({}))
            .with_dependency("A".to_string()),
    ];
    
    let result = analyzer.analyze_dependencies(&requests).await;
    assert!(result.is_err()); // Should detect circular dependency
}

#[tokio::test]
async fn test_metrics_tracking() {
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(10);
    config.default_tool_timeout = Duration::from_secs(2);
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    let tool_executor = Arc::new(MockToolExecutor::new(false, Duration::from_millis(50)));
    let event_emitter = Arc::new(MockEventEmitter::new());
    
    let request = ToolExecutionRequest::new(
        "test_tool".to_string(),
        serde_json::json!({})
    );
    
    // Execute orchestration
    let _result = orchestrator.orchestrate_tools(
        vec![request],
        tool_executor,
        event_emitter,
    ).await.unwrap();
    
    // Check metrics were updated
    let metrics = orchestrator.get_metrics().await;
    assert_eq!(metrics.total_orchestrations, 1);
    assert_eq!(metrics.successful_orchestrations, 1);
    assert!(metrics.avg_execution_time_ms > 0);
}

#[tokio::test]
async fn test_simple_tool_failure() {
    let mut config = crate::config::OrchestrationConfig::default();
    config.global_timeout = Duration::from_secs(10);
    config.default_tool_timeout = Duration::from_secs(2);
    config.max_retry_attempts = 1; // Reduce retries for faster test
    
    let orchestrator = ToolOrchestrator::new(config).await.unwrap();
    
    let tool_executor = Arc::new(MockToolExecutor::new(true, Duration::from_millis(10))); // Will fail
    let event_emitter = Arc::new(MockEventEmitter::new());
    
    let request = ToolExecutionRequest::new(
        "failing_tool".to_string(),
        serde_json::json!({})
    );
    
    let result = orchestrator.orchestrate_tools(
        vec![request],
        tool_executor,
        event_emitter,
    ).await.unwrap();
    
    assert!(!result.success);
    assert_eq!(result.failed_tools, 1);
    
    // Check that the failed tool has recovery suggestions
    let failed_result = result.tool_results.values().next().unwrap();
    assert_eq!(failed_result.status, ExecutionStatus::Failed);
    assert!(failed_result.recovery_suggestions.is_some());
} 