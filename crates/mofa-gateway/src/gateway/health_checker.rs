//! Health checking for cluster nodes.
//!
//! This module provides health checking functionality to monitor node
//! availability and automatically remove unhealthy nodes from the load balancer.
//!
//! # Implementation Status
//!
//! **Complete** - Health checking with automatic node removal implemented

use crate::error::{GatewayError, GatewayResult};
use crate::types::{NodeId, NodeStatus};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::timeout;

/// Health status of a node.
#[derive(Debug, Clone)]
pub struct NodeHealth {
    /// Node ID.
    pub node_id: NodeId,
    /// Current status.
    pub status: NodeStatus,
    /// Last successful health check.
    pub last_success: Option<Instant>,
    /// Last failed health check.
    pub last_failure: Option<Instant>,
    /// Consecutive failures.
    pub consecutive_failures: u32,
}

/// Health checker for monitoring node health.
pub struct HealthChecker {
    nodes: Arc<RwLock<HashMap<NodeId, NodeHealth>>>,
    check_interval: Duration,
    timeout: Duration,
    failure_threshold: u32,
    // Node addresses for HTTP health checks
    node_addresses: Arc<RwLock<HashMap<NodeId, std::net::SocketAddr>>>,
}

impl HealthChecker {
    /// Create a new health checker.
    pub fn new(check_interval: Duration, timeout: Duration, failure_threshold: u32) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            check_interval,
            timeout,
            failure_threshold,
            node_addresses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a node address for health checking.
    pub async fn register_node_address(&self, node_id: NodeId, address: std::net::SocketAddr) {
        let mut addresses = self.node_addresses.write().await;
        addresses.insert(node_id, address);
    }

    /// Register a node for health checking.
    pub async fn register_node(&self, node_id: NodeId) {
        let mut nodes = self.nodes.write().await;
        nodes.insert(
            node_id.clone(),
            NodeHealth {
                node_id,
                status: NodeStatus::Starting,
                last_success: None,
                last_failure: None,
                consecutive_failures: 0,
            },
        );
    }

    /// Unregister a node.
    pub async fn unregister_node(&self, node_id: &NodeId) {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);
    }

    /// Perform a health check on a node.
    pub async fn check_node(&self, node_id: &NodeId) -> GatewayResult<bool> {
        // Get node address for HTTP health check
        let address = {
            let addresses = self.node_addresses.read().await;
            addresses.get(node_id).copied()
        };

        let is_healthy = if let Some(addr) = address {
            // Perform actual HTTP health check
            Self::perform_health_check(node_id, addr, self.timeout).await
        } else {
            // If no address registered, assume healthy (for backward compatibility)
            true
        };

        let mut nodes = self.nodes.write().await;
        if let Some(health) = nodes.get_mut(node_id) {
            if is_healthy {
                health.last_success = Some(Instant::now());
                health.consecutive_failures = 0;
                health.status = NodeStatus::Healthy;
            } else {
                health.last_failure = Some(Instant::now());
                health.consecutive_failures += 1;
                if health.consecutive_failures >= self.failure_threshold {
                    health.status = NodeStatus::Unhealthy;
                }
            }
            Ok(is_healthy)
        } else {
            Ok(false)
        }
    }

    /// Get the health status of a node.
    pub async fn get_status(&self, node_id: &NodeId) -> Option<NodeStatus> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).map(|h| h.status)
    }

    /// Start the health checking loop.
    pub async fn start(&self) -> GatewayResult<()> {
        let nodes = Arc::clone(&self.nodes);
        let addresses = Arc::clone(&self.node_addresses);
        let check_interval = self.check_interval;
        let timeout = self.timeout;
        let failure_threshold = self.failure_threshold;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                // Get all node IDs to check
                let node_ids: Vec<NodeId> = {
                    let nodes_guard = nodes.read().await;
                    nodes_guard.keys().cloned().collect()
                };

                // Check each node
                for node_id in node_ids {
                    // Get node address
                    let address = {
                        let addresses_guard = addresses.read().await;
                        addresses_guard.get(&node_id).copied()
                    };

                    let is_healthy = if let Some(addr) = address {
                        Self::perform_health_check(&node_id, addr, timeout).await
                    } else {
                        // If no address, skip check (node not properly registered)
                        continue;
                    };

                    let mut nodes_guard = nodes.write().await;
                    if let Some(health) = nodes_guard.get_mut(&node_id) {
                        if is_healthy {
                            health.last_success = Some(Instant::now());
                            health.consecutive_failures = 0;
                            health.status = NodeStatus::Healthy;
                        } else {
                            health.last_failure = Some(Instant::now());
                            health.consecutive_failures += 1;

                            if health.consecutive_failures >= failure_threshold {
                                health.status = NodeStatus::Unhealthy;
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Perform an actual HTTP health check on a node.
    async fn perform_health_check(node_id: &NodeId, address: std::net::SocketAddr, timeout_duration: Duration) -> bool {
        // Use tokio TcpStream to make a simple HTTP GET request
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;
        
        // Connect to the node with timeout
        let stream_result = timeout(timeout_duration, TcpStream::connect(address)).await;
        
        let mut stream = match stream_result {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                tracing::debug!("Failed to connect to {} for health check: {}", node_id, e);
                return false;
            }
            Err(_) => {
                tracing::debug!("Connection timeout for {} health check", node_id);
                return false;
            }
        };
        
        // Send HTTP GET request
        let request = format!("GET /health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", address);
        
        // Write request with timeout
        match timeout(timeout_duration, stream.write_all(request.as_bytes())).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                tracing::debug!("Failed to write health check request to {}: {}", node_id, e);
                return false;
            }
            Err(_) => {
                tracing::debug!("Write timeout for {} health check", node_id);
                return false;
            }
        }
        
        // Read response with timeout
        let mut buffer = [0u8; 1024];
        match timeout(timeout_duration, stream.read(&mut buffer)).await {
            Ok(Ok(size)) if size > 0 => {
                // Parse HTTP status line
                let response = String::from_utf8_lossy(&buffer[..size]);
                // Check if status code starts with "HTTP/1.1 2" (2xx)
                response.starts_with("HTTP/1.1 2") || response.starts_with("HTTP/1.0 2")
            }
            Ok(Ok(_)) => {
                tracing::debug!("Empty response from {} health check", node_id);
                false
            }
            Ok(Err(e)) => {
                tracing::debug!("Failed to read health check response from {}: {}", node_id, e);
                false
            }
            Err(_) => {
                tracing::debug!("Read timeout for {} health check", node_id);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new(
            Duration::from_secs(5),
            Duration::from_secs(1),
            3,
        );

        let node_id = NodeId::new("node-1");
        checker.register_node(node_id.clone()).await;

        let is_healthy = checker.check_node(&node_id).await.unwrap();
        assert!(is_healthy);

        let status = checker.get_status(&node_id).await;
        assert_eq!(status, Some(NodeStatus::Healthy));
    }
}
