//! Gateway routing policy implementation.
//!
//! Provides:
//! - Failover routing: try primary provider, fallback on error
//! - Cost-optimized routing: select cheapest provider based on static pricing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Routing policy for the gateway.
/// 
/// This defines how requests are routed between multiple backend providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingPolicy {
    /// Failover routing: try primary provider first, fallback to secondary on error.
    Failover {
        /// Primary provider identifier.
        primary: String,
        /// Fallback provider identifier.
        fallback: String,
    },
    /// Cost-optimized routing: select the cheapest available provider.
    CostOptimized,
    /// Default routing: use the first available provider.
    Default,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self::Default
    }
}

impl std::fmt::Display for RoutingPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failover { primary, fallback } => {
                write!(f, "failover({}->{})", primary, fallback)
            }
            Self::CostOptimized => write!(f, "cost-optimized"),
            Self::Default => write!(f, "default"),
        }
    }
}

/// Provider cost information for routing decisions.
#[derive(Debug, Clone)]
pub struct ProviderCost {
    /// Provider identifier.
    pub provider: String,
    /// Cost per 1K input tokens (USD).
    pub input_cost_per_1k: f64,
    /// Cost per 1K output tokens (USD).
    pub output_cost_per_1k: f64,
}

impl ProviderCost {
    /// Create new provider cost info.
    pub fn new(provider: impl Into<String>, input_cost_per_1k: f64, output_cost_per_1k: f64) -> Self {
        Self {
            provider: provider.into(),
            input_cost_per_1k,
            output_cost_per_1k,
        }
    }

    /// Calculate total cost for a request.
    pub fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * self.input_cost_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * self.output_cost_per_1k;
        input_cost + output_cost
    }
}

/// Static provider pricing for cost-optimized routing.
#[derive(Debug, Clone, Default)]
pub struct StaticPricingRegistry {
    /// Provider costs keyed by provider name.
    costs: HashMap<String, ProviderCost>,
}

impl StaticPricingRegistry {
    /// Create a new empty pricing registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with default pricing for common providers.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        
        // Local/Ollama is free
        registry.register_provider(ProviderCost::new("local", 0.0, 0.0));
        registry.register_provider(ProviderCost::new("ollama", 0.0, 0.0));
        
        // OpenAI pricing (USD per 1K tokens)
        registry.register_provider(ProviderCost::new("openai", 2.50, 10.00));
        
        // Anthropic pricing
        registry.register_provider(ProviderCost::new("anthropic", 3.00, 15.00));
        
        // Gemini pricing
        registry.register_provider(ProviderCost::new("gemini", 1.25, 5.00));
        
        registry
    }

    /// Register a provider with its pricing.
    pub fn register_provider(&mut self, cost: ProviderCost) {
        self.costs.insert(cost.provider.clone(), cost);
    }

    /// Get the cheapest available provider.
    /// Returns the provider name with the lowest cost.
    pub fn get_cheapest_provider(&self, available_providers: &[String]) -> Option<String> {
        available_providers
            .iter()
            .filter_map(|p| self.costs.get(p))
            .min_by(|a, b| {
                a.input_cost_per_1k
                    .partial_cmp(&b.input_cost_per_1k)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|c| c.provider.clone())
    }

    /// Get cost info for a specific provider.
    pub fn get_cost(&self, provider: &str) -> Option<&ProviderCost> {
        self.costs.get(provider)
    }
}

/// Result of a routing decision.
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// The selected provider.
    pub provider: String,
    /// Whether this was a failover (true) or primary (false).
    pub is_failover: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_policy_display() {
        let failover = RoutingPolicy::Failover {
            primary: "openai".into(),
            fallback: "anthropic".into(),
        };
        assert_eq!(format!("{}", failover), "failover(openai->anthropic)");
        
        let cost_opt = RoutingPolicy::CostOptimized;
        assert_eq!(format!("{}", cost_opt), "cost-optimized");
    }

    #[test]
    fn test_static_pricing_registry_defaults() {
        let registry = StaticPricingRegistry::with_defaults();
        
        // Local should be free
        let local_cost = registry.get_cost("local").unwrap();
        assert_eq!(local_cost.input_cost_per_1k, 0.0);
        
        // OpenAI should have pricing
        let openai_cost = registry.get_cost("openai").unwrap();
        assert!(openai_cost.input_cost_per_1k > 0.0);
    }

    #[test]
    fn test_get_cheapest_provider() {
        let mut registry = StaticPricingRegistry::new();
        registry.register_provider(ProviderCost::new("expensive", 10.0, 20.0));
        registry.register_provider(ProviderCost::new("cheap", 0.5, 1.0));
        registry.register_provider(ProviderCost::new("free", 0.0, 0.0));
        
        let providers = vec!["expensive".to_string(), "cheap".to_string(), "free".to_string()];
        let cheapest = registry.get_cheapest_provider(&providers).unwrap();
        assert_eq!(cheapest, "free");
    }

    #[test]
    fn test_provider_cost_calculation() {
        let cost = ProviderCost::new("test", 1.0, 2.0);
        let total = cost.calculate_cost(1000, 500);
        // 1000 input tokens * $1/1k = $1
        // 500 output tokens * $2/1k = $1
        // Total = $2
        assert!((total - 2.0).abs() < 0.001);
    }
}
