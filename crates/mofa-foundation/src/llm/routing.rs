use crate::llm::inference::{InferenceBackend, InferenceError, InferenceResult, InferenceRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingPolicy {
    LocalFirst,
    CloudFirst,
    LatencyFirst,
    CostFirst,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub default_policy: RoutingPolicy,
    pub fallback_enabled: bool,
    pub latency_threshold_ms: u64,
    pub cost_per_1k_tokens: f64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            default_policy: RoutingPolicy::LocalFirst,
            fallback_enabled: true,
            latency_threshold_ms: 2000,
            cost_per_1k_tokens: 0.002,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingMetrics {
    pub local_requests: u64,
    pub cloud_requests: u64,
    pub fallback_requests: u64,
    pub rejected_requests: u64,
    pub average_latency_ms: f64,
}

pub struct RoutingEngine {
    config: RoutingConfig,
    local_backend: Arc<dyn InferenceBackend>,
    cloud_backend: Arc<dyn InferenceBackend>,
    metrics: RwLock<RoutingMetrics>,
}

impl RoutingEngine {
    pub fn new(
        config: RoutingConfig,
        local_backend: Arc<dyn InferenceBackend>,
        cloud_backend: Arc<dyn InferenceBackend>,
    ) -> Self {
        Self {
            config,
            local_backend,
            cloud_backend,
            metrics: RwLock::new(RoutingMetrics {
                local_requests: 0,
                cloud_requests: 0,
                fallback_requests: 0,
                rejected_requests: 0,
                average_latency_ms: 0.0,
            }),
        }
    }

    pub async fn route(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        let policy = &self.config.default_policy;

        match policy {
            RoutingPolicy::LocalFirst => self.route_local_first(request).await,
            RoutingPolicy::CloudFirst => self.route_cloud_first(request).await,
            RoutingPolicy::LatencyFirst => self.route_latency_first(request).await,
            RoutingPolicy::CostFirst => self.route_cost_first(request).await,
            RoutingPolicy::Custom(name) => self.route_custom(name, request).await,
        }
    }

    async fn route_local_first(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        let local_health = self.local_backend.health_check().await;

        if let Ok(health) = local_health {
            if health.healthy && health.latency_ms < self.config.latency_threshold_ms {
                let mut metrics = self.metrics.write().await;
                metrics.local_requests += 1;
                return self.local_backend.generate(request).await;
            }
        }

        if self.config.fallback_enabled {
            let mut metrics = self.metrics.write().await;
            metrics.fallback_requests += 1;
            return self.cloud_backend.generate(request).await;
        }

        Err(InferenceError::BackendError(\"Local backend unavailable\".to_string()))
    }

    async fn route_cloud_first(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        let cloud_health = self.cloud_backend.health_check().await;

        if let Ok(health) = cloud_health {
            if health.healthy {
                let mut metrics = self.metrics.write().await;
                metrics.cloud_requests += 1;
                return self.cloud_backend.generate(request).await;
            }
        }

        if self.config.fallback_enabled {
            let mut metrics = self.metrics.write().await;
            metrics.fallback_requests += 1;
            return self.local_backend.generate(request).await;
        }

        Err(InferenceError::BackendError(\"Cloud backend unavailable\".to_string()))
    }

    async fn route_latency_first(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        let local_health = self.local_backend.health_check().await.ok();
        let cloud_health = self.cloud_backend.health_check().await.ok();

        let local_latency = local_health.map(|h| h.latency_ms).unwrap_or(u64::MAX);
        let cloud_latency = cloud_health.map(|h| h.latency_ms).unwrap_or(u64::MAX);

        let mut metrics = self.metrics.write().await;

        if local_latency <= cloud_latency && local_latency < self.config.latency_threshold_ms {
            metrics.local_requests += 1;
            self.local_backend.generate(request).await
        } else {
            metrics.cloud_requests += 1;
            self.cloud_backend.generate(request).await
        }
    }

    async fn route_cost_first(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        let local_health = self.local_backend.health_check().await;

        if let Ok(health) = local_health {
            if health.healthy {
                let mut metrics = self.metrics.write().await;
                metrics.local_requests += 1;
                return self.local_backend.generate(request).await;
            }
        }

        let mut metrics = self.metrics.write().await;
        metrics.cloud_requests += 1;
        self.cloud_backend.generate(request).await
    }

    async fn route_custom(&self, _policy_name: &str, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        self.route_local_first(request).await
    }

    pub async fn get_metrics(&self) -> RoutingMetrics {
        self.metrics.read().await.clone()
    }

    pub fn set_policy(&mut self, policy: RoutingPolicy) {
        self.config.default_policy = policy;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionControlConfig {
    pub max_memory_percent: f64,
    pub max_concurrent_requests: usize,
    pub queue_size: usize,
}

impl Default for AdmissionControlConfig {
    fn default() -> Self {
        Self {
            max_memory_percent: 80.0,
            max_concurrent_requests: 10,
            queue_size: 100,
        }
    }
}

pub struct AdmissionController {
    config: AdmissionControlConfig,
    current_load: RwLock<f64>,
    queue: RwLock<Vec<()>>,
}

impl AdmissionController {
    pub fn new(config: AdmissionControlConfig) -> Self {
        Self {
            config,
            current_load: RwLock::new(0.0),
            queue: RwLock::new(Vec::new()),
        }
    }

    pub async fn can_admit(&self) -> bool {
        let load = *self.current_load.read().await;
        let queue_len = self.queue.read().await.len();

        load < self.config.max_memory_percent && queue_len < self.config.queue_size
    }

    pub async fn admit(&self) -> Option<AdmissionGuard> {
        if self.can_admit().await {
            let mut queue = self.queue.write().await;
            queue.push(());
            Some(AdmissionGuard {
                controller: std::sync::Arc::new(self.clone()),
            })
        } else {
            None
        }
    }

    pub async fn update_load(&self, load: f64) {
        let mut current = self.current_load.write().await;
        *current = load;
    }
}

#[derive(Clone)]
pub struct AdmissionGuard {
    controller: std::sync::Arc<AdmissionController>,
}

impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        let controller = self.controller.clone();
        tokio::spawn(async move {
            let mut queue = controller.queue.write().await;
            queue.pop();
        });
    }
}
