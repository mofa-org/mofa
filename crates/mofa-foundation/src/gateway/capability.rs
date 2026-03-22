use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use mofa_kernel::agent::types::error::GlobalResult;

use crate::swarm::{CapabilityExecutionPolicy, SwarmSubtask};

/// Request payload passed to a gateway-backed capability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityRequest {
    /// Primary task input, typically the subtask description.
    pub input: String,
    /// Structured arguments for the capability implementation.
    #[serde(default)]
    pub params: HashMap<String, Value>,
    /// Trace identifier propagated from the caller.
    pub trace_id: String,
}

/// Response payload returned from a gateway-backed capability.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityResponse {
    /// Primary textual output that becomes the task result.
    pub output: String,
    /// Structured metadata returned by the capability implementation.
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
    /// End-to-end capability invocation latency in milliseconds.
    pub latency_ms: u64,
}

/// Capability interface for external tools, devices, and APIs.
#[async_trait]
pub trait GatewayCapability: Send + Sync {
    /// Stable capability name used for registry lookup.
    fn name(&self) -> &str;

    /// Execute the capability with a structured request.
    async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse>;
}

/// Shared registry of gateway-backed capabilities.
#[derive(Default)]
pub struct GatewayCapabilityRegistry {
    capabilities: DashMap<String, Arc<dyn GatewayCapability>>,
}

impl GatewayCapabilityRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register or replace a capability under its stable name.
    pub fn register(&self, capability: Arc<dyn GatewayCapability>) {
        self.capabilities
            .insert(capability.name().to_string(), capability);
    }

    /// Remove a capability by name.
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn GatewayCapability>> {
        self.capabilities.remove(name).map(|(_, capability)| capability)
    }

    /// Look up a capability by exact name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn GatewayCapability>> {
        self.capabilities.get(name).map(|entry| Arc::clone(entry.value()))
    }

    /// Returns `true` when a capability exists for the given name.
    pub fn contains(&self, name: &str) -> bool {
        self.capabilities.contains_key(name)
    }

    /// Return all registered capability names in sorted order.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .capabilities
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        names.sort();
        names
    }

    /// Resolve the first registered capability required by a subtask.
    ///
    /// This scans all declared required capabilities in order rather than
    /// assuming the first tag is always available.
    pub fn resolve_for_task(&self, task: &SwarmSubtask) -> Option<Arc<dyn GatewayCapability>> {
        task.required_capabilities
            .iter()
            .find_map(|capability_name| self.get(capability_name))
    }

    /// Invoke the first registered capability required by a subtask.
    ///
    /// Returns `Ok(None)` when none of the task's required capabilities are
    /// registered in the gateway registry so callers can decide whether to
    /// fail or fall back to another execution path.
    pub async fn invoke_task(
        &self,
        task: &SwarmSubtask,
        trace_id: impl Into<String>,
    ) -> GlobalResult<Option<CapabilityResponse>> {
        if matches!(task.capability_policy, CapabilityExecutionPolicy::LocalOnly) {
            return Ok(None);
        }

        let Some(capability) = self.resolve_for_task(task) else {
            if matches!(
                task.capability_policy,
                CapabilityExecutionPolicy::RequireCapability
            ) && !task.required_capabilities.is_empty()
            {
                return Err(mofa_kernel::agent::types::error::GlobalError::Other(
                    format!(
                        "required capability not available for task '{}': {}",
                        task.id,
                        task.required_capabilities.join(", ")
                    ),
                ));
            }
            return Ok(None);
        };

        let request = CapabilityRequest {
            input: task.description.clone(),
            params: task.capability_params.clone(),
            trace_id: trace_id.into(),
        };

        capability.invoke(request).await.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::types::error::GlobalError;

    struct MockCapability {
        name: &'static str,
        output: &'static str,
    }

    #[async_trait]
    impl GatewayCapability for MockCapability {
        fn name(&self) -> &str {
            self.name
        }

        async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
            if input.input.is_empty() {
                return Err(GlobalError::Other("missing input".to_string()));
            }

            Ok(CapabilityResponse {
                output: format!("{}: {}", self.output, input.input),
                metadata: HashMap::from([(
                    "trace_id".to_string(),
                    Value::String(input.trace_id),
                )]),
                latency_ms: 12,
            })
        }
    }

    #[test]
    fn register_and_lookup_capability() {
        let registry = GatewayCapabilityRegistry::new();
        registry.register(Arc::new(MockCapability {
            name: "web_search",
            output: "ok",
        }));

        assert!(registry.contains("web_search"));
        assert_eq!(registry.names(), vec!["web_search".to_string()]);
        assert!(registry.get("web_search").is_some());
    }

    #[tokio::test]
    async fn invoke_task_uses_first_matching_registered_capability() {
        let registry = GatewayCapabilityRegistry::new();
        registry.register(Arc::new(MockCapability {
            name: "http_fetch",
            output: "fetched",
        }));

        let task = SwarmSubtask::new("task-1", "fetch status page").with_capabilities(vec![
            "missing".to_string(),
            "http_fetch".to_string(),
        ]);

        let response = registry
            .invoke_task(&task, "trace-123")
            .await
            .expect("capability invocation should succeed")
            .expect("matching capability should be found");

        assert_eq!(response.output, "fetched: fetch status page");
        assert_eq!(
            response.metadata.get("trace_id"),
            Some(&Value::String("trace-123".to_string()))
        );
    }

    #[tokio::test]
    async fn invoke_task_returns_none_when_no_capability_matches() {
        let registry = GatewayCapabilityRegistry::new();
        registry.register(Arc::new(MockCapability {
            name: "web_search",
            output: "ok",
        }));

        let task =
            SwarmSubtask::new("task-1", "draft report").with_capabilities(vec!["llm".to_string()]);

        let response = registry
            .invoke_task(&task, "trace-456")
            .await
            .expect("missing capability should not be an error");

        assert!(response.is_none());
    }

    #[tokio::test]
    async fn invoke_task_errors_when_policy_requires_capability() {
        let registry = GatewayCapabilityRegistry::new();
        let task = SwarmSubtask::new("task-1", "read sensor")
            .with_capabilities(vec!["read_sensor".to_string()])
            .with_capability_policy(CapabilityExecutionPolicy::RequireCapability);

        let error = registry
            .invoke_task(&task, "trace-required")
            .await
            .expect_err("missing required capability should fail");

        assert!(error.to_string().contains("required capability not available"));
    }
}
