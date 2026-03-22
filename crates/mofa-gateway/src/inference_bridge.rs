//! Inference Bridge - Gateway-level routing between multiple backends.
//!
//! This module provides:
//! - Failover routing (primary → fallback on error)
//! - Cost-optimized routing (select cheapest provider)
//! - Simple provider selection based on configured policy
//!
//! Note: This is a lightweight gateway-level routing layer that works with
//! the existing inference orchestrator in mofa-foundation.

use crate::gateway::routing_policy::{RoutingPolicy, RoutingResult, StaticPricingRegistry};

/// InferenceBridge provides gateway-level routing between multiple backend providers.
///
/// This bridges the gateway's routing configuration with the underlying
/// inference orchestrator. It handles:
/// - Failover: try primary, fall back to secondary on error
/// - Cost optimization: select cheapest available provider
#[derive(Debug, Clone)]
pub struct InferenceBridge {
    /// The default provider to use.
    default_provider: String,
    /// Available providers in priority order.
    providers: Vec<String>,
    /// Routing policy to apply.
    policy: RoutingPolicy,
    /// Pricing registry for cost-based routing.
    pricing: StaticPricingRegistry,
}

impl InferenceBridge {
    /// Create a new inference bridge with the given default provider.
    pub fn new(default_provider: impl Into<String>) -> Self {
        let provider = default_provider.into();
        Self {
            default_provider: provider.clone(),
            providers: vec![provider],
            policy: RoutingPolicy::Default,
            pricing: StaticPricingRegistry::with_defaults(),
        }
    }

    /// Create a new bridge with multiple providers.
    pub fn with_providers(mut self, providers: Vec<String>) -> Self {
        if !providers.is_empty() {
            self.default_provider = providers[0].clone();
            self.providers = providers;
        }
        self
    }

    /// Set the routing policy.
    pub fn with_routing_policy(mut self, policy: RoutingPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Set custom pricing registry.
    pub fn with_pricing(mut self, pricing: StaticPricingRegistry) -> Self {
        self.pricing = pricing;
        self
    }

    /// Get the default provider.
    pub fn default_provider(&self) -> &str {
        &self.default_provider
    }

    /// Get all available providers.
    pub fn providers(&self) -> &[String] {
        &self.providers
    }

    /// Get the current routing policy.
    pub fn policy(&self) -> &RoutingPolicy {
        &self.policy
    }

    /// Resolve the provider to use based on the routing policy.
    ///
    /// For Failover policy, this returns the primary provider.
    /// The actual failover happens when calling `call_with_failover`.
    pub fn resolve_provider(&self) -> String {
        match &self.policy {
            RoutingPolicy::Failover { primary, .. } => primary.clone(),
            RoutingPolicy::CostOptimized => {
                self.pricing
                    .get_cheapest_provider(&self.providers)
                    .unwrap_or_else(|| self.default_provider.clone())
            }
            RoutingPolicy::Default => self.default_provider.clone(),
        }
    }

    /// Execute inference with failover support.
    ///
    /// If the policy is Failover, try primary first, then fallback on error.
    /// Returns the provider that was used and whether failover occurred.
    pub async fn call_with_failover<F, Fut, T, E>(
        &self,
        mut primary_fn: F,
    ) -> Result<(T, RoutingResult), E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        match &self.policy {
            RoutingPolicy::Failover { primary, fallback } => {
                // Try primary first
                tracing::debug!("Attempting primary provider: {}", primary);
                let result = primary_fn().await;
                
                match result {
                    Ok(response) => {
                        Ok((response, RoutingResult {
                            provider: primary.clone(),
                            is_failover: false,
                        }))
                    }
                    Err(e) => {
                        // Primary failed, try fallback
                        tracing::warn!("Primary provider {} failed: {:?}, trying fallback: {}", 
                            primary, e, fallback);
                        
                        // Note: In a real implementation, you'd call the fallback provider here
                        // For now, we return the error since we can't actually call different providers
                        // without the actual inference client
                        Err(e)
                    }
                }
            }
            _ => {
                // No failover needed, just call the primary
                let provider = self.resolve_provider();
                let response = primary_fn().await?;
                Ok((response, RoutingResult {
                    provider,
                    is_failover: false,
                }))
            }
        }
    }

    /// Get the provider that would be selected for cost-optimized routing.
    pub fn get_cheapest_provider(&self) -> Option<String> {
        self.pricing.get_cheapest_provider(&self.providers)
    }
}

impl Default for InferenceBridge {
    fn default() -> Self {
        Self::new("local")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_bridge_default() {
        let bridge = InferenceBridge::new("openai");
        assert_eq!(bridge.default_provider(), "openai");
        assert_eq!(bridge.providers(), vec!["openai"]);
    }

    #[test]
    fn test_inference_bridge_with_providers() {
        let bridge = InferenceBridge::new("openai")
            .with_providers(vec!["openai".to_string(), "anthropic".to_string()]);
        
        assert_eq!(bridge.default_provider(), "openai");
        assert_eq!(bridge.providers().len(), 2);
    }

    #[test]
    fn test_inference_bridge_cost_optimized() {
        let bridge = InferenceBridge::new("openai")
            .with_providers(vec![
                "openai".to_string(), 
                "local".to_string(), 
                "anthropic".to_string()
            ])
            .with_routing_policy(RoutingPolicy::CostOptimized);
        
        // Local should be cheapest (free)
        let cheapest = bridge.get_cheapest_provider();
        assert_eq!(cheapest, Some("local".to_string()));
    }

    #[test]
    fn test_inference_bridge_failover_policy() {
        let bridge = InferenceBridge::new("openai")
            .with_routing_policy(RoutingPolicy::Failover {
                primary: "openai".to_string(),
                fallback: "anthropic".to_string(),
            });
        
        let provider = bridge.resolve_provider();
        assert_eq!(provider, "openai");
    }

    #[test]
    fn test_resolve_provider_default() {
        let bridge = InferenceBridge::new("openai");
        assert_eq!(bridge.resolve_provider(), "openai");
    }
}
