//! Provider Pricing Registry — traits and core data types only.
//! Concrete implementations live in `mofa-foundation::cost`.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Per-model pricing (USD per 1,000 tokens)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModelPricing {
    pub input_cost_per_1k_tokens: f64,
    pub output_cost_per_1k_tokens: f64,
}

impl ModelPricing {
    pub fn new(input_cost_per_1k: f64, output_cost_per_1k: f64) -> Self {
        Self {
            input_cost_per_1k_tokens: input_cost_per_1k,
            output_cost_per_1k_tokens: output_cost_per_1k,
        }
    }

    pub fn free() -> Self {
        Self::new(0.0, 0.0)
    }

    pub fn calculate_cost(&self, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        let input_cost = (prompt_tokens as f64 / 1000.0) * self.input_cost_per_1k_tokens;
        let output_cost = (completion_tokens as f64 / 1000.0) * self.output_cost_per_1k_tokens;
        input_cost + output_cost
    }

    pub fn calculate_cost_detailed(
        &self,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> CostBreakdown {
        let input_cost = (prompt_tokens as f64 / 1000.0) * self.input_cost_per_1k_tokens;
        let output_cost = (completion_tokens as f64 / 1000.0) * self.output_cost_per_1k_tokens;
        CostBreakdown {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            currency: "USD".to_string(),
        }
    }
}

/// Detailed cost breakdown for a single LLM call
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CostBreakdown {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
}

/// Registry for looking up model pricing by provider and model name.
/// Concrete implementations (e.g. `InMemoryPricingRegistry`) live in `mofa-foundation::cost`.
pub trait ProviderPricingRegistry: Send + Sync {
    fn get_pricing(&self, provider: &str, model: &str) -> Option<ModelPricing>;
    fn list_models(&self) -> Vec<(String, String)>;
}

/// Type alias for a shared, dynamically-dispatched pricing registry.
pub type SharedPricingRegistry = Arc<dyn ProviderPricingRegistry>;

/// Convenience helper: calculate cost from a `ModelPricing` reference.
pub fn calculate_cost(pricing: &ModelPricing, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    pricing.calculate_cost(prompt_tokens, completion_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_pricing_calculation() {
        let pricing = ModelPricing::new(2.50, 10.00);
        let cost = pricing.calculate_cost(1000, 500);
        assert!((cost - 7.50).abs() < 0.001);
    }

    #[test]
    fn test_free_pricing() {
        let pricing = ModelPricing::free();
        let cost = pricing.calculate_cost(10000, 5000);
        assert!((cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cost_breakdown() {
        let pricing = ModelPricing::new(3.00, 15.00);
        let breakdown = pricing.calculate_cost_detailed(2000, 1000);
        assert!((breakdown.input_cost - 6.00).abs() < 0.001);
        assert!((breakdown.output_cost - 15.00).abs() < 0.001);
        assert!((breakdown.total_cost - 21.00).abs() < 0.001);
        assert_eq!(breakdown.currency, "USD");
    }

    #[test]
    fn test_zero_tokens() {
        let pricing = ModelPricing::new(2.50, 10.00);
        assert!((pricing.calculate_cost(0, 0)).abs() < f64::EPSILON);
    }
}
