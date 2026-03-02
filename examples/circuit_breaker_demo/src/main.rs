use async_trait::async_trait;
use mofa_foundation::workflow::StateGraphImpl;
use mofa_kernel::workflow::{
    Command, JsonState, NodeFunc, RuntimeContext, START, END, StateGraph, NodePolicy, RetryCondition
};
use mofa_kernel::agent::error::AgentError;
use serde_json::json;
use std::time::Duration;

/// A node that always fails, simulating a flaky external service or LLM.
struct FlakyNode;

#[async_trait]
impl NodeFunc<JsonState> for FlakyNode {
    async fn call(
        &self,
        _state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> Result<Command, AgentError> {
        println!("  [FlakyNode] Executing...");
        // Return a transient error that matches our retry condition
        Err(AgentError::Other("Rate limit exceeded".to_string()))
    }

    fn name(&self) -> &str {
        "flaky_service"
    }
}

/// A node that handles the fallback when the primary node's circuit is open
/// or retries are exhausted.
struct FallbackNode;

#[async_trait]
impl NodeFunc<JsonState> for FallbackNode {
    async fn call(
        &self,
        _state: &mut JsonState,
        _ctx: &RuntimeContext,
    ) -> Result<Command, AgentError> {
        println!("  [FallbackNode] Executing recovery logic...");
        let cmd = Command::new().update(
            "status",
            json!("recovered via fallback"),
        );
        Ok(cmd.continue_())
    }

    fn name(&self) -> &str {
        "fallback_service"
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    println!("=== Building Workflow Graph ===");
    
    // Create the workflow graph
    let mut graph = StateGraphImpl::<JsonState>::new("circuit_breaker_demo");
    
    // Add nodes
    graph.add_node("flaky_service", Box::new(FlakyNode));
    graph.add_node("fallback_service", Box::new(FallbackNode));
    
    // Define edges
    graph.add_edge(START, "flaky_service");
    graph.add_edge("flaky_service", END);
    graph.add_edge("fallback_service", END);
    
    // IMPORTANT: Attach a fault-tolerance policy to the flaky node
    // This policy says: retry twice if the error contains "rate limit",
    // wait 100ms between retries, and if all retries fail, route to "fallback_service".
    // If the node fails 3 consecutive times across executions, open the circuit.
    graph.with_node_policy(
        "flaky_service",
        NodePolicy {
            max_retries: 2,
            retry_backoff_ms: 100,
            retry_condition: RetryCondition::OnTransient(vec!["rate limit".to_string()]),
            fallback_node: Some("fallback_service".to_string()),
            circuit_open_after: 3,
            circuit_reset_after: Duration::from_secs(60),
        },
    );

    // Compile the graph
    let compiled = graph.compile()?;
    
    println!("=== Executing Workflow ===");
    println!("Notice how `flaky_service` is retried automatically by the StateGraph");
    println!("and eventually routes to `fallback_service` (simulated by stream events).\n");
    
    let initial_state = JsonState::new();
    
    // Let's run it and observe the stream events
    // Note: The actual runtime retries happen in Phase 2 in Foundation,
    // but building the graph and seeing the policy attached is working now.
    println!("Policy attached successfully. Run the Phase 2 implementation to see execution!");

    Ok(())
}
