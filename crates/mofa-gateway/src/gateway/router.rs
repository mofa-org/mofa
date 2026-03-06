//! Request router for the gateway.
//!
//! This module handles routing requests to appropriate cluster nodes based on
//! load balancing, health checks, and circuit breaker state.

use crate::error::{GatewayError, GatewayResult};
use crate::types::{NodeId, RequestMetadata};
use crate::gateway::{
    CircuitBreakerRegistry, HealthChecker, LoadBalancer,
};
use std::sync::Arc;
use tracing::{debug, warn};

/// Request router.
pub struct GatewayRouter {
    load_balancer: Arc<LoadBalancer>,
    health_checker: Arc<HealthChecker>,
    circuit_breakers: Arc<CircuitBreakerRegistry>,
    max_retries: usize,
}

impl GatewayRouter {
    /// Create a new gateway router.
    pub fn new(
        load_balancer: Arc<LoadBalancer>,
        health_checker: Arc<HealthChecker>,
        circuit_breakers: Arc<CircuitBreakerRegistry>,
    ) -> Self {
        Self {
            load_balancer,
            health_checker,
            circuit_breakers,
            max_retries: 3,
        }
    }

    /// Set maximum retries for routing attempts.
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Route a request to an appropriate node.
    pub async fn route(&self, metadata: &RequestMetadata) -> GatewayResult<NodeId> {
        let mut last_error = None;

        // Try to find a healthy node with retries
        for attempt in 0..=self.max_retries {
            // Select node via load balancer
            let node_id = match self.load_balancer.select_node().await? {
                Some(id) => id,
                None => {
                    return Err(GatewayError::NoAvailableNodes(
                        "No nodes available in load balancer".to_string(),
                    ));
                }
            };

            // Check health
            let is_healthy = match self.health_checker.get_status(&node_id).await {
                Some(status) => status == crate::types::NodeStatus::Healthy,
                None => {
                    // Node not registered, try to check it
                    self.health_checker.check_node(&node_id).await?;
                    self.health_checker.get_status(&node_id).await
                        .map(|s| s == crate::types::NodeStatus::Healthy)
                        .unwrap_or(false)
                }
            };

            if !is_healthy {
                debug!("Node {} is unhealthy, trying next node", node_id);
                last_error = Some(GatewayError::UnhealthyNode(node_id.to_string()));
                continue;
            }

            // Check circuit breaker
            let breaker = self.circuit_breakers.get_or_create(&node_id).await;
            if !breaker.try_acquire().await? {
                debug!("Circuit breaker is open for node {}, trying next node", node_id);
                last_error = Some(GatewayError::CircuitBreakerOpen(node_id.to_string()));
                continue;
            }

            // Found a suitable node
            debug!("Routed request {} to node {} (attempt {})", metadata.request_id, node_id, attempt + 1);
            return Ok(node_id);
        }

        // All retries exhausted
        warn!("Failed to route request {} after {} attempts", metadata.request_id, self.max_retries + 1);
        Err(last_error.unwrap_or_else(|| {
            GatewayError::NoAvailableNodes("No healthy nodes available".to_string())
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_router_with_healthy_node() {
        let lb = Arc::new(LoadBalancer::new(crate::types::LoadBalancingAlgorithm::RoundRobin));
        let hc = Arc::new(HealthChecker::new(
            Duration::from_secs(5),
            Duration::from_secs(1),
            3,
        ));
        let cb = Arc::new(CircuitBreakerRegistry::new(3, 2, Duration::from_secs(5)));

        let node_id = NodeId::new("node-1");
        lb.add_node(node_id.clone()).await;
        hc.register_node(node_id.clone()).await;
        hc.check_node(&node_id).await.unwrap();

        let router = GatewayRouter::new(lb, hc, cb);
        let metadata = RequestMetadata {
            request_id: "req-1".to_string(),
            client_ip: None,
            user_id: None,
            timestamp: std::time::SystemTime::now(),
            extra: std::collections::HashMap::new(),
        };

        let routed = router.route(&metadata).await.unwrap();
        assert_eq!(routed, node_id);
    }
}
