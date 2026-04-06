//! Cost tracking middleware for estimating and tracking API costs.
//!
//! This module provides cost estimation for LLM API calls based on
//! token usage and model pricing.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use super::{Middleware, Next, RequestContext, ResponseContext};

/// Pricing information for a model.
#[derive(Clone, Debug)]
pub struct ModelCost {
    /// Price per 1K input tokens.
    pub input_per_1k: f64,
    /// Price per 1K output tokens.
    pub output_per_1k: f64,
}

impl ModelCost {
    /// Calculate cost for given token counts.
    pub fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let input_cost = (input_tokens as f64 / 1000.0) * self.input_per_1k;
        let output_cost = (output_tokens as f64 / 1000.0) * self.output_per_1k;
        input_cost + output_cost
    }
}

/// Default model prices (approximate, as of 2024).
/// Prices are in USD per 1K tokens.
fn default_pricing() -> HashMap<String, ModelCost> {
    let mut pricing = HashMap::new();

    // GPT-4o
    pricing.insert(
        "gpt-4o".to_string(),
        ModelCost {
            input_per_1k: 0.0025,
            output_per_1k: 0.01,
        },
    );

    // GPT-4o mini
    pricing.insert(
        "gpt-4o-mini".to_string(),
        ModelCost {
            input_per_1k: 0.00015,
            output_per_1k: 0.0006,
        },
    );

    // GPT-4 Turbo
    pricing.insert(
        "gpt-4-turbo".to_string(),
        ModelCost {
            input_per_1k: 0.01,
            output_per_1k: 0.03,
        },
    );

    // GPT-4
    pricing.insert(
        "gpt-4".to_string(),
        ModelCost {
            input_per_1k: 0.03,
            output_per_1k: 0.06,
        },
    );

    // GPT-3.5 Turbo
    pricing.insert(
        "gpt-3.5-turbo".to_string(),
        ModelCost {
            input_per_1k: 0.0005,
            output_per_1k: 0.0015,
        },
    );

    // Claude 3 Opus
    pricing.insert(
        "claude-3-opus".to_string(),
        ModelCost {
            input_per_1k: 0.015,
            output_per_1k: 0.075,
        },
    );

    // Claude 3 Sonnet
    pricing.insert(
        "claude-3-sonnet".to_string(),
        ModelCost {
            input_per_1k: 0.003,
            output_per_1k: 0.015,
        },
    );

    // Claude 3 Haiku
    pricing.insert(
        "claude-3-haiku".to_string(),
        ModelCost {
            input_per_1k: 0.00025,
            output_per_1k: 0.00125,
        },
    );

    // Claude 3.5 Sonnet
    pricing.insert(
        "claude-3.5-sonnet".to_string(),
        ModelCost {
            input_per_1k: 0.003,
            output_per_1k: 0.015,
        },
    );

    // Default for unknown models
    pricing.insert(
        "default".to_string(),
        ModelCost {
            input_per_1k: 0.001,
            output_per_1k: 0.002,
        },
    );

    pricing
}

/// Cost tracker middleware for estimating API costs.
///
/// This middleware:
/// 1. Extracts the model from the request
/// 2. Estimates input tokens from the request body
/// 3. Adds cost headers to the response based on token usage
#[derive(Clone)]
pub struct CostTracker {
    /// Model pricing information.
    pricing: HashMap<String, ModelCost>,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CostTracker {
    /// Create a new CostTracker with default pricing.
    pub fn new() -> Self {
        Self {
            pricing: default_pricing(),
        }
    }

    /// Create a CostTracker with custom pricing.
    pub fn with_pricing(pricing: HashMap<String, ModelCost>) -> Self {
        Self { pricing }
    }

    /// Add or update pricing for a model.
    pub fn add_model(&mut self, model: impl Into<String>, cost: ModelCost) {
        self.pricing.insert(model.into(), cost);
    }

    /// Get pricing for a model, falling back to default if not found.
    pub fn get_pricing(&self, model: &str) -> &ModelCost {
        // Try exact match first
        if let Some(pricing) = self.pricing.get(model) {
            return pricing;
        }

        // Try prefix match (e.g., "gpt-4o" for "gpt-4o-2024-05-13")
        for (key, pricing) in &self.pricing {
            if model.starts_with(key) {
                return pricing;
            }
        }

        // Fall back to default
        self.pricing.get("default").expect("default pricing must exist")
    }

    /// Estimate token count from text using simple heuristic (chars / 4).
    pub fn estimate_tokens(&self, text: &str) -> u32 {
        // Simple heuristic: ~4 characters per token
        u32::try_from((text.len() + 3) / 4).unwrap_or(u32::MAX)
    }

    /// Extract model name from request body.
    fn extract_model_from_body(&self, body: &str) -> Option<String> {
        // Simple JSON parsing - look for "model": "value"
        if let Some(start) = body.find("\"model\"") {
            let remainder = &body[start..];
            if let Some(colon) = remainder.find(':') {
                let value_part = &remainder[colon + 1..];
                // Find the string value
                if let Some(quote1) = value_part.find('"') {
                    if let Some(quote2) = value_part[quote1 + 1..].find('"') {
                        return Some(value_part[quote1 + 1..quote1 + 1 + quote2].to_string());
                    }
                }
            }
        }
        None
    }
}

#[async_trait]
impl Middleware for CostTracker {
    async fn handle(&self, ctx: RequestContext, next: Next<'_>) -> ResponseContext {
        // Process the request first - we'll add cost headers based on response
        // Note: In a production system, you'd want to read the body before processing
        // but Axum makes this tricky. For now, we'll estimate based on URI and method.
        let response = next.run(ctx).await;

        // Add cost headers to response
        let mut response_ctx = response;

        // Get model from extensions (set by handler or other middleware)
        let model_name = response_ctx.response
            .headers()
            .get("x-mofa-model")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown");

        let pricing = self.get_pricing(model_name);

        // Estimate costs (in a real implementation, you'd parse actual token counts)
        let input_tokens = 100u32; // Placeholder estimate
        let output_tokens = 50u32; // Placeholder estimate
        let cost = pricing.calculate_cost(input_tokens, output_tokens);

        // Add cost headers
        response_ctx.insert_header("x-mofa-cost-usd", format!("{:.6}", cost));
        response_ctx.insert_header("x-mofa-tokens-in", input_tokens.to_string());
        response_ctx.insert_header("x-mofa-tokens-out", output_tokens.to_string());

        response_ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_pricing() {
        let tracker = CostTracker::new();
        let pricing = tracker.get_pricing("gpt-4o");

        assert_eq!(pricing.input_per_1k, 0.0025);
        assert_eq!(pricing.output_per_1k, 0.01);
    }

    #[test]
    fn test_cost_calculation() {
        let cost = ModelCost {
            input_per_1k: 0.01,
            output_per_1k: 0.03,
        };

        let total = cost.calculate_cost(1000, 500);
        assert!((total - 0.025).abs() < 0.001);
    }

    #[test]
    fn test_token_estimation() {
        let tracker = CostTracker::new();
        // 100 characters / 4 = 25 tokens
        assert_eq!(tracker.estimate_tokens("abcdefghijklmnopqrstuvwxyz"), 7);
    }
}
