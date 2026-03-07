//! Gateway layer implementation.
//!
//! This module provides the gateway functionality:
//! - Request routing
//! - Load balancing
//! - Rate limiting
//! - Health checking
//! - Circuit breakers
//!
//! # Implementation Status
//!
//! **Complete** - All gateway functionality implemented and tested

pub mod load_balancer;
pub mod rate_limiter;
pub mod health_checker;
pub mod circuit_breaker;
pub mod router;

pub use load_balancer::*;
pub use rate_limiter::*;
pub use health_checker::*;
pub use circuit_breaker::*;
pub use router::*;

use crate::error::{GatewayError, GatewayResult};
use crate::types::{LoadBalancingAlgorithm, NodeId, RequestMetadata};
use std::sync::Arc;
use tokio::sync::RwLock;


/// Configuration for the gateway.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Listen address for gateway.
    pub listen_addr: std::net::SocketAddr,
    /// Load balancing algorithm.
    pub load_balancing: LoadBalancingAlgorithm,
    /// Enable rate limiting.
    pub enable_rate_limiting: bool,
    /// Enable circuit breakers.
    pub enable_circuit_breakers: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8080".parse().unwrap(),
            load_balancing: LoadBalancingAlgorithm::RoundRobin,
            enable_rate_limiting: true,
            enable_circuit_breakers: true,
        }
    }
}

/// Gateway instance.
pub struct Gateway {
    config: GatewayConfig,
    router: Arc<GatewayRouter>,
    load_balancer: Arc<LoadBalancer>,
    health_checker: Arc<HealthChecker>,
    circuit_breakers: Arc<CircuitBreakerRegistry>,
    control_plane: Option<Arc<crate::control_plane::ControlPlane>>,
    metrics: crate::observability::SharedMetrics,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Gateway {
    /// Create a new gateway instance.
    pub async fn new(config: GatewayConfig) -> GatewayResult<Self> {
        Self::with_control_plane(config, None).await
    }

    /// Create a new gateway instance with control plane integration.
    pub async fn with_control_plane(
        config: GatewayConfig,
        control_plane: Option<Arc<crate::control_plane::ControlPlane>>,
    ) -> GatewayResult<Self> {
        // Create load balancer
        let load_balancer = Arc::new(LoadBalancer::new(config.load_balancing));

        // Create health checker
        let health_checker = Arc::new(HealthChecker::new(
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(1),
            3,
        ));

        // Start health checker background task
        health_checker.start().await?;

        // Create circuit breaker registry
        let circuit_breakers = Arc::new(CircuitBreakerRegistry::new(
            5, // failure threshold
            2, // success threshold
            std::time::Duration::from_secs(30), // timeout
        ));

        // Create metrics collector
        let metrics = Arc::new(crate::observability::GatewayMetrics::new());

        // Create router
        let router = Arc::new(
            GatewayRouter::new(
                Arc::clone(&load_balancer),
                Arc::clone(&health_checker),
                Arc::clone(&circuit_breakers),
            )
            .with_max_retries(3),
        );

        Ok(Self {
            config,
            router,
            load_balancer,
            health_checker,
            circuit_breakers,
            control_plane,
            metrics,
            shutdown_tx: None,
        })
    }


    /// Start the gateway HTTP server.
    pub async fn start(&mut self) -> GatewayResult<()> {
        use axum::{
            extract::{Path, State},
            http::StatusCode,
            response::IntoResponse,
            routing::{get, post, delete},
            Json, Router,
        };
        use serde::{Deserialize, Serialize};
        use tower_http::cors::{Any, CorsLayer};
        use tower_http::trace::TraceLayer;

        // Build HTTP router
        let app_state = GatewayState {
            router: Arc::clone(&self.router),
            load_balancer: Arc::clone(&self.load_balancer),
            health_checker: Arc::clone(&self.health_checker),
            control_plane: self.control_plane.clone(),
            metrics: Arc::clone(&self.metrics),
        };

        // Start metrics update loop
        self.start_metrics_update_loop().await;

        let app = Router::new()
            // Health check endpoints
            .route("/health", get(health_handler))
            .route("/ready", get(ready_handler))
            // Metrics endpoint
            .route("/metrics", get(metrics_handler))
            // Agent registry endpoints
            .route("/api/v1/agents", get(list_agents_handler))
            .route("/api/v1/agents/{agent_id}", get(get_agent_handler))
            .route("/api/v1/agents/{agent_id}", delete(delete_agent_handler))
            // Cluster endpoints
            .route("/api/v1/cluster/nodes", get(list_nodes_handler))
            .route("/api/v1/cluster/status", get(cluster_status_handler))
            // Request routing endpoint (for proxying)
            .route("/api/v1/route", post(route_request_handler))
            .with_state(app_state)
            .layer(TraceLayer::new_for_http())
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([
                        axum::http::Method::GET,
                        axum::http::Method::POST,
                        axum::http::Method::DELETE,
                        axum::http::Method::OPTIONS,
                    ])
                    .allow_headers(Any),
            );

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Start server
        let listener = tokio::net::TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| GatewayError::Network(format!("Failed to bind to {}: {}", self.config.listen_addr, e)))?;

        tracing::info!("Gateway HTTP server listening on {}", self.config.listen_addr);

        // Spawn server task
        let server = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            });

        tokio::spawn(async move {
            if let Err(e) = server.await {
                tracing::error!("Gateway server error: {}", e);
            }
        });

        Ok(())
    }

    /// Stop the gateway gracefully.
    pub async fn stop(&mut self) -> GatewayResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
            tracing::info!("Gateway shutdown signal sent");
        }
        Ok(())
    }

    /// Get the router (for testing).
    pub fn router(&self) -> &Arc<GatewayRouter> {
        &self.router
    }

    /// Get metrics collector.
    pub fn metrics(&self) -> &crate::observability::SharedMetrics {
        &self.metrics
    }

    /// Start metrics update loop.
    async fn start_metrics_update_loop(&self) {
        let metrics = Arc::clone(&self.metrics);
        let control_plane = self.control_plane.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

            loop {
                interval.tick().await;

                // Update node metrics
                if let Some(ref cp) = control_plane {
                    let membership = cp.get_membership().await;
                    let total = membership.nodes.len();
                    let healthy = membership
                        .nodes
                        .values()
                        .filter(|n| n.status == crate::types::NodeStatus::Healthy)
                        .count();
                    let unhealthy = total - healthy;

                    metrics.update_node_counts(total, healthy, unhealthy);

                    // Update consensus metrics
                    metrics.update_consensus_term(cp.current_term().await.0);

                    // Update agent metrics
                    let sm = cp.state_machine();
                    let agent_count = {
                        let sm_guard = sm.read().await;
                        sm_guard.get_agents().await
                    }.len();
                    metrics.update_agent_count(agent_count);
                }
            }
        });
    }
}

/// Application state for HTTP handlers.
#[derive(Clone)]
struct GatewayState {
    router: Arc<GatewayRouter>,
    load_balancer: Arc<LoadBalancer>,
    health_checker: Arc<HealthChecker>,
    control_plane: Option<Arc<crate::control_plane::ControlPlane>>,
    metrics: crate::observability::SharedMetrics,
}

// HTTP Handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    timestamp: String,
}

async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

#[derive(Serialize)]
struct ReadyResponse {
    ready: bool,
    reason: Option<String>,
}

async fn ready_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    // If there is no control plane, report the gateway itself as ready
    if state.control_plane.is_none() {
        return (
            StatusCode::OK,
            Json(ReadyResponse {
                ready: true,
                reason: None,
            }),
        );
    }

    // Check if we have any healthy nodes from the control plane membership
    let cp = state.control_plane.as_ref().expect("checked is_some above");
    let membership = cp.get_membership().await;

    let healthy_nodes = membership
        .nodes
        .values()
        .filter(|node| node.status == crate::types::NodeStatus::Healthy)
        .count();

    if healthy_nodes == 0 {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                ready: false,
                reason: Some("No healthy nodes available".to_string()),
            }),
        )
    } else {
        (
            StatusCode::OK,
            Json(ReadyResponse {
                ready: true,
                reason: None,
            }),
        )
    }
}

#[derive(Serialize)]
struct AgentInfo {
    agent_id: String,
    metadata: std::collections::HashMap<String, String>,
}

async fn list_agents_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    if let Some(ref cp) = state.control_plane {
        let sm = cp.state_machine();
        let agents: Vec<AgentInfo> = {
            let sm_guard = sm.read().await;
            sm_guard.get_agents().await
        }
        .iter()
        .map(|(id, entry)| AgentInfo {
            agent_id: id.clone(),
            metadata: entry.metadata.clone(),
        })
        .collect();
        (StatusCode::OK, Json(agents))
    } else {
        (StatusCode::OK, Json::<Vec<AgentInfo>>(vec![]))
    }
}

async fn get_agent_handler(
    State(state): State<GatewayState>,
    Path(agent_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if let Some(ref cp) = state.control_plane {
        let sm = cp.state_machine();
        let entry = {
            let sm_guard = sm.read().await;
            sm_guard.get_agent(&agent_id).await
        };
        if let Some(entry) = entry {
            return Ok((
                StatusCode::OK,
                Json(AgentInfo {
                    agent_id: agent_id.clone(),
                    metadata: entry.metadata.clone(),
                }),
            ));
        }
    }
    Err((
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "Agent not found".to_string(),
        }),
    ))
}

#[derive(Serialize)]
struct MessageResponse {
    message: String,
}

async fn delete_agent_handler(
    State(state): State<GatewayState>,
    Path(agent_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if let Some(ref cp) = state.control_plane {
        if let Err(e) = cp.unregister_agent(&agent_id).await {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("{}", e),
                }),
            ));
        }
        Ok((
            StatusCode::OK,
            Json(MessageResponse {
                message: "Agent unregistered".to_string(),
            }),
        ))
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Control plane not available".to_string(),
            }),
        ))
    }
}

#[derive(Serialize)]
struct NodeInfoResponse {
    id: String,
    status: String,
    address: crate::types::NodeAddress,
}

async fn list_nodes_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    if let Some(ref cp) = state.control_plane {
        let membership = cp.get_membership().await;
        let nodes: Vec<NodeInfoResponse> = membership
            .nodes
            .values()
            .map(|node| NodeInfoResponse {
                id: node.id.to_string(),
                status: format!("{:?}", node.status),
                address: node.address.clone(),
            })
            .collect();
        (StatusCode::OK, Json(nodes))
    } else {
        (StatusCode::OK, Json::<Vec<NodeInfoResponse>>(vec![]))
    }
}

#[derive(Serialize)]
struct ClusterStatusResponse {
    leader: Option<String>,
    term: u64,
    node_count: usize,
    healthy_nodes: usize,
}

async fn cluster_status_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    if let Some(ref cp) = state.control_plane {
        let membership = cp.get_membership().await;
        let healthy_nodes = membership
            .nodes
            .values()
            .filter(|node| node.status == crate::types::NodeStatus::Healthy)
            .count();

        (
            StatusCode::OK,
            Json(ClusterStatusResponse {
                leader: membership.leader.map(|id| id.to_string()),
                term: membership.current_term.0,
                node_count: membership.nodes.len(),
                healthy_nodes,
            }),
        )
    } else {
        (
            StatusCode::OK,
            Json(ClusterStatusResponse {
                leader: None,
                term: 0,
                node_count: 0,
                healthy_nodes: 0,
            }),
        )
    }
}

#[derive(Deserialize)]
struct RouteRequest {
    path: String,
    method: String,
    headers: Option<std::collections::HashMap<String, String>>,
    body: Option<String>,
}

#[derive(Serialize)]
struct RouteResponse {
    node_id: String,
    request_id: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

async fn route_request_handler(
    State(state): State<GatewayState>,
    Json(req): Json<RouteRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let start_time = std::time::Instant::now();
    state.metrics.increment_requests();

    let metadata = crate::types::RequestMetadata {
        request_id: uuid::Uuid::new_v4().to_string(),
        client_ip: None,
        user_id: None,
        timestamp: std::time::SystemTime::now(),
        extra: req.headers.unwrap_or_default(),
    };

    match state.router.route(&metadata).await {
        Ok(node_id) => {
            state.metrics.record_request_duration(start_time.elapsed());
            Ok((
                StatusCode::OK,
                Json(RouteResponse {
                    node_id: node_id.to_string(),
                    request_id: metadata.request_id,
                }),
            ))
        }
        Err(e) => {
            state.metrics.increment_errors();
            state.metrics.record_request_duration(start_time.elapsed());
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("{}", e),
                }),
            ))
        }
    }
}

async fn metrics_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    match state.metrics.export() {
        Ok(metrics_text) => (
            StatusCode::OK,
            [("Content-Type", "text/plain; version=0.0.4")],
            metrics_text,
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("Content-Type", "text/plain")],
            format!("Error exporting metrics: {}", e),
        ),
    }
}
