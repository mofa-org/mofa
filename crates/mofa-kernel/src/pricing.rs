//! Provider Pricing Registry

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Per-model pricing (USD per 1,000 tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
}

/// Registry for looking up model pricing by provider and model name
pub trait ProviderPricingRegistry: Send + Sync {
    fn get_pricing(&self, provider: &str, model: &str) -> Option<ModelPricing>;
    fn list_models(&self) -> Vec<(String, String)>;
}

/// In-memory pricing registry with built-in prices for major providers.
/// Key format: `"provider/model"` (e.g. `"openai/gpt-4o"`)
#[derive(Debug, Clone)]
pub struct InMemoryPricingRegistry {
    prices: HashMap<String, ModelPricing>,
}

impl InMemoryPricingRegistry {
    pub fn with_defaults() -> Self {
        let mut prices = HashMap::new();

        // OpenAI (USD per 1K tokens)
        prices.insert("openai/gpt-4o".into(), ModelPricing::new(2.50, 10.00));
        prices.insert("openai/gpt-4o-mini".into(), ModelPricing::new(0.15, 0.60));
        prices.insert("openai/gpt-4-turbo".into(), ModelPricing::new(10.00, 30.00));
        prices.insert("openai/gpt-3.5-turbo".into(), ModelPricing::new(0.50, 1.50));
        prices.insert("openai/o1".into(), ModelPricing::new(15.00, 60.00));
        prices.insert("openai/o1-mini".into(), ModelPricing::new(3.00, 12.00));

        // Anthropic
        prices.insert(
            "anthropic/claude-3.5-sonnet".into(),
            ModelPricing::new(3.00, 15.00),
        );
        prices.insert(
            "anthropic/claude-3-haiku".into(),
            ModelPricing::new(0.25, 1.25),
        );
        prices.insert(
            "anthropic/claude-3-opus".into(),
            ModelPricing::new(15.00, 75.00),
        );
        prices.insert(
            "anthropic/claude-3.5-haiku".into(),
            ModelPricing::new(1.00, 5.00),
        );

        // Google Gemini
        prices.insert(
            "gemini/gemini-1.5-pro".into(),
            ModelPricing::new(1.25, 5.00),
        );
        prices.insert(
            "gemini/gemini-1.5-flash".into(),
            ModelPricing::new(0.075, 0.30),
        );
        prices.insert(
            "gemini/gemini-2.0-flash".into(),
            ModelPricing::new(0.10, 0.40),
        );

        // Local / Ollama (free)
        prices.insert("ollama/any".into(), ModelPricing::free());

        Self { prices }
    }

    pub fn empty() -> Self {
        Self {
            prices: HashMap::new(),
        }
    }

    pub fn set_pricing(
        &mut self,
        provider: impl Into<String>,
        model: impl Into<String>,
        pricing: ModelPricing,
    ) {
        let key = format!("{}/{}", provider.into(), model.into());
        self.prices.insert(key, pricing);
    }

    fn fuzzy_lookup(&self, provider: &str, model: &str) -> Option<&ModelPricing> {
        let provider_lower = provider.to_lowercase();
        let model_lower = model.to_lowercase();

        // Exact match
        let exact_key = format!("{}/{}", provider_lower, model_lower);
        if let Some(pricing) = self.prices.get(&exact_key) {
            return Some(pricing);
        }

        // Strip date suffix (e.g. "gpt-4o-2024-05-13" → "gpt-4o")
        let base_model = model_lower
            .split('-')
            .take_while(|part| part.parse::<u32>().is_err() || part.len() < 4)
            .collect::<Vec<_>>()
            .join("-");
        if base_model != model_lower {
            let base_key = format!("{}/{}", provider_lower, base_model);
            if let Some(pricing) = self.prices.get(&base_key) {
                return Some(pricing);
            }
        }

        // Ollama/local → always free
        if provider_lower == "ollama" || provider_lower == "local" {
            return self.prices.get("ollama/any");
        }

        None
    }
}

impl ProviderPricingRegistry for InMemoryPricingRegistry {
    fn get_pricing(&self, provider: &str, model: &str) -> Option<ModelPricing> {
        self.fuzzy_lookup(provider, model).cloned()
    }

    fn list_models(&self) -> Vec<(String, String)> {
        self.prices
            .keys()
            .filter_map(|key| {
                let parts: Vec<&str> = key.splitn(2, '/').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
            .collect()
    }
}

pub type SharedPricingRegistry = Arc<dyn ProviderPricingRegistry>;

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
    fn test_registry_exact_lookup() {
        let registry = InMemoryPricingRegistry::with_defaults();
        let pricing = registry.get_pricing("openai", "gpt-4o");
        assert!(pricing.is_some());
        let p = pricing.unwrap();
        assert!((p.input_cost_per_1k_tokens - 2.50).abs() < 0.001);
    }

    #[test]
    fn test_registry_case_insensitive() {
        let registry = InMemoryPricingRegistry::with_defaults();
        assert!(registry.get_pricing("OpenAI", "GPT-4o").is_some());
    }

    #[test]
    fn test_registry_ollama_any_model_free() {
        let registry = InMemoryPricingRegistry::with_defaults();
        let pricing = registry.get_pricing("ollama", "llama3.1:70b");
        assert!(pricing.is_some());
        assert!((pricing.unwrap().input_cost_per_1k_tokens).abs() < f64::EPSILON);
    }

    #[test]
    fn test_registry_unknown_model_returns_none() {
        let registry = InMemoryPricingRegistry::with_defaults();
        assert!(
            registry
                .get_pricing("unknown_provider", "unknown_model")
                .is_none()
        );
    }

    #[test]
    fn test_registry_custom_override() {
        let mut registry = InMemoryPricingRegistry::with_defaults();
        registry.set_pricing("custom", "my-model", ModelPricing::new(1.00, 2.00));
        let p = registry.get_pricing("custom", "my-model").unwrap();
        assert!((p.input_cost_per_1k_tokens - 1.00).abs() < 0.001);
    }

    #[test]
    fn test_registry_anthropic_claude() {
        let registry = InMemoryPricingRegistry::with_defaults();
        let p = registry
            .get_pricing("anthropic", "claude-3.5-sonnet")
            .unwrap();
        assert!((p.input_cost_per_1k_tokens - 3.00).abs() < 0.001);
        assert!((p.output_cost_per_1k_tokens - 15.00).abs() < 0.001);
    }

    #[test]
    fn test_list_models() {
        let registry = InMemoryPricingRegistry::with_defaults();
        let models = registry.list_models();
        assert!(models.len() >= 10);
    }

    #[test]
    fn test_zero_tokens() {
        let pricing = ModelPricing::new(2.50, 10.00);
        assert!((pricing.calculate_cost(0, 0)).abs() < f64::EPSILON);
    }
}
