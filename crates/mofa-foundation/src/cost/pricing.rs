//! In-memory pricing registry — concrete implementation of `ProviderPricingRegistry`.

use mofa_kernel::pricing::{ModelPricing, ProviderPricingRegistry};
use std::collections::HashMap;

/// In-memory pricing registry.
///
/// Prices set with [`InMemoryPricingRegistry::set_pricing`] always win. Everything else is
/// priced from [`llm_prices`], a table of provider list prices in which every row cites the
/// provider's pricing page and the date it was checked. `ModelPricing` is USD per 1,000
/// tokens, so the per-1M list prices are divided by 1,000 here.
///
/// Key format for overrides: `"provider/model"` (e.g. `"openai/gpt-4o"`)
#[derive(Debug, Clone)]
pub struct InMemoryPricingRegistry {
    prices: HashMap<String, ModelPricing>,
    list_prices: bool,
}

/// Provider names MoFA uses, mapped to the names `llm_prices` expects.
fn list_price_provider(provider: &str) -> &str {
    match provider {
        "gemini" | "google" => "google",
        "claude" => "anthropic",
        other => other,
    }
}

/// The provider's list price, converted from USD per 1M tokens to USD per 1K tokens.
fn list_price(provider: &str, model: &str) -> Option<ModelPricing> {
    let p = llm_prices::lookup_provider(list_price_provider(provider), model)?;
    let input = p.input?;
    // Embedding rows have no output price; they produce no output tokens.
    let output = p.output.unwrap_or(0.0);
    Some(ModelPricing::new(input / 1000.0, output / 1000.0))
}

impl InMemoryPricingRegistry {
    /// Build a registry that prices OpenAI, Anthropic, Gemini, DeepSeek, Mistral, xAI and the
    /// other providers in `llm_prices` at list price, and Ollama / local models at zero.
    pub fn with_defaults() -> Self {
        let mut prices = HashMap::new();

        // Local / Ollama (free)
        prices.insert("ollama/any".into(), ModelPricing::free());

        Self {
            prices,
            list_prices: true,
        }
    }

    /// Build an empty registry (no pre-loaded prices).
    pub fn empty() -> Self {
        Self {
            prices: HashMap::new(),
            list_prices: false,
        }
    }

    /// Insert or overwrite pricing for a specific provider/model pair.
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
        if let Some(pricing) = self.fuzzy_lookup(provider, model) {
            return Some(pricing.clone());
        }
        if self.list_prices {
            return list_price(&provider.to_lowercase(), model);
        }
        None
    }

    fn list_models(&self) -> Vec<(String, String)> {
        let mut models: Vec<(String, String)> = self
            .prices
            .keys()
            .filter_map(|key| {
                let parts: Vec<&str> = key.splitn(2, '/').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
            .collect();
        if self.list_prices {
            models.extend(
                llm_prices::all()
                    .iter()
                    .filter(|p| p.status == "listed" && p.input.is_some())
                    .map(|p| (p.provider.to_string(), p.model.to_string())),
            );
        }
        models
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_exact_lookup() {
        let registry = InMemoryPricingRegistry::with_defaults();
        let pricing = registry.get_pricing("openai", "gpt-4o");
        assert!(pricing.is_some());
        let p = pricing.unwrap();
        // $2.50 per 1M input tokens is $0.0025 per 1K.
        assert!((p.input_cost_per_1k_tokens - 0.0025).abs() < 1e-12);
        assert!((p.output_cost_per_1k_tokens - 0.010).abs() < 1e-12);
    }

    #[test]
    fn test_registry_costs_are_per_1k_tokens() {
        // 1M input + 1M output tokens on gpt-4o is $2.50 + $10.00 at list price.
        let registry = InMemoryPricingRegistry::with_defaults();
        let p = registry.get_pricing("openai", "gpt-4o").unwrap();
        let cost = p.calculate_cost(1_000_000, 1_000_000);
        assert!((cost - 12.50).abs() < 1e-9, "{cost}");
    }

    #[test]
    fn test_registry_prices_current_models() {
        let registry = InMemoryPricingRegistry::with_defaults();
        for (provider, model) in [
            ("openai", "gpt-5"),
            ("openai", "gpt-4.1-mini"),
            ("anthropic", "claude-sonnet-4-6"),
            ("anthropic", "claude-opus-4-5-20251101"),
            ("gemini", "gemini-2.5-pro"),
            ("gemini", "gemini-2.5-flash"),
        ] {
            let p = registry.get_pricing(provider, model);
            assert!(p.is_some(), "{provider}/{model} has no price");
            assert!(p.unwrap().input_cost_per_1k_tokens > 0.0);
        }
    }

    #[test]
    fn test_registry_dated_id_uses_base_price() {
        let registry = InMemoryPricingRegistry::with_defaults();
        let dated = registry
            .get_pricing("anthropic", "claude-3-5-haiku-20241022")
            .unwrap();
        // Claude Haiku 3.5 lists at $0.80 / $4 per 1M tokens.
        assert!((dated.input_cost_per_1k_tokens - 0.0008).abs() < 1e-12);
        assert!((dated.output_cost_per_1k_tokens - 0.004).abs() < 1e-12);
    }

    #[test]
    fn test_override_beats_list_price() {
        let mut registry = InMemoryPricingRegistry::with_defaults();
        registry.set_pricing("openai", "gpt-4o", ModelPricing::new(0.001, 0.002));
        let p = registry.get_pricing("openai", "gpt-4o").unwrap();
        assert!((p.input_cost_per_1k_tokens - 0.001).abs() < 1e-12);
    }

    #[test]
    fn test_empty_registry_has_no_list_prices() {
        let registry = InMemoryPricingRegistry::empty();
        assert!(registry.get_pricing("openai", "gpt-4o").is_none());
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
            .get_pricing("anthropic", "claude-sonnet-4-5")
            .unwrap();
        assert!((p.input_cost_per_1k_tokens - 0.003).abs() < 1e-12);
        assert!((p.output_cost_per_1k_tokens - 0.015).abs() < 1e-12);
    }

    #[test]
    fn test_list_models() {
        let registry = InMemoryPricingRegistry::with_defaults();
        let models = registry.list_models();
        assert!(models.len() >= 10);
    }
}
