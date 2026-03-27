use std::fmt;

use super::orchestration::{InferenceRequest, RoutedBackend};

/// A set of candidate backends for a routing decision.
#[derive(Debug, Clone)]
pub struct CandidateSet {
    pub candidates: Vec<RoutedBackend>,
}

/// Represents custom filtering/routing logic that can be provided by the user.
/// It MUST be `Send + Sync` to participate in multi-threaded execution.
pub trait RoutingPredicate: Send + Sync + fmt::Debug {
    /// Evaluates candidate backends against a specific inference request.
    /// Returns a score or admissibility metric for the candidates.
    fn evaluate(&self, request: &InferenceRequest, candidates: &CandidateSet) -> f32;
}

/// The specific optimization goal the routing policy should prioritize.
/// E.g., choosing to optimize for money vs latency or user-defined traits.
#[non_exhaustive]
#[derive(Debug)]
pub enum RoutingObjective {
    CostOptimized,
    LatencyOptimized,
    QualityMaximized,
    ComplianceFirst,
    Custom(Box<dyn RoutingPredicate>),
}

/// Structural weight used to multiply against different routing objective scores.
/// Enforces safe boundaries for valid calculation via Builder-pattern.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RoutingWeight {
    cost: f32,
    latency: f32,
    quality: f32,
    compliance: f32,
}

impl RoutingWeight {
    pub const MIN_WEIGHT: f32 = 0.0;
    pub const MAX_WEIGHT: f32 = 1.0;

    /// Creates a new `RoutingWeight` with default weights of 0.0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the weight for the cost objective.
    /// Returns `Err` if `w` is not between 0.0 and 1.0.
    pub fn with_cost(mut self, w: f32) -> Result<Self, &'static str> {
        if !(Self::MIN_WEIGHT..=Self::MAX_WEIGHT).contains(&w) {
            return Err("Cost weight must be between 0.0 and 1.0");
        }
        self.cost = w;
        Ok(self)
    }

    pub fn with_latency(mut self, w: f32) -> Result<Self, &'static str> {
        if !(Self::MIN_WEIGHT..=Self::MAX_WEIGHT).contains(&w) {
            return Err("Latency weight must be between 0.0 and 1.0");
        }
        self.latency = w;
        Ok(self)
    }

    pub fn with_quality(mut self, w: f32) -> Result<Self, &'static str> {
        if !(Self::MIN_WEIGHT..=Self::MAX_WEIGHT).contains(&w) {
            return Err("Quality weight must be between 0.0 and 1.0");
        }
        self.quality = w;
        Ok(self)
    }

    pub fn with_compliance(mut self, w: f32) -> Result<Self, &'static str> {
        if !(Self::MIN_WEIGHT..=Self::MAX_WEIGHT).contains(&w) {
            return Err("Compliance weight must be between 0.0 and 1.0");
        }
        self.compliance = w;
        Ok(self)
    }

    pub fn cost(&self) -> f32 {
        self.cost
    }

    pub fn latency(&self) -> f32 {
        self.latency
    }

    pub fn quality(&self) -> f32 {
        self.quality
    }

    pub fn compliance(&self) -> f32 {
        self.compliance
    }

    /// Calculates the weighted score based on individual objective scores.
    /// Objectives compose additively.
    pub fn calculate_score(
        &self,
        cost_score: f32,
        latency_score: f32,
        quality_score: f32,
        compliance_score: f32,
    ) -> f32 {
        self.cost * cost_score
            + self.latency * latency_score
            + self.quality * quality_score
            + self.compliance * compliance_score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_routing_weight() {
        let weight = RoutingWeight::new()
            .with_cost(0.5)
            .unwrap()
            .with_latency(0.3)
            .unwrap()
            .with_quality(1.0)
            .unwrap()
            .with_compliance(0.0)
            .unwrap();

        assert_eq!(weight.cost(), 0.5);
        assert_eq!(weight.latency(), 0.3);
        assert_eq!(weight.quality(), 1.0);
        assert_eq!(weight.compliance(), 0.0);
    }

    #[test]
    fn test_invalid_routing_weight_cost() {
        assert!(RoutingWeight::new().with_cost(-0.1).is_err());
        assert!(RoutingWeight::new().with_cost(1.1).is_err());
    }

    #[test]
    fn test_invalid_routing_weight_latency() {
        assert!(RoutingWeight::new().with_latency(-0.1).is_err());
        assert!(RoutingWeight::new().with_latency(1.1).is_err());
    }

    #[test]
    fn test_score_calculation_additive() {
        let weight = RoutingWeight::new()
            .with_cost(0.5)
            .unwrap()
            .with_latency(0.3)
            .unwrap()
            .with_quality(0.2)
            .unwrap();

        // Scores
        let cost_score = 100.0;
        let latency_score = 50.0;
        let quality_score = 80.0;
        let compliance_score = 0.0;

        let total =
            weight.calculate_score(cost_score, latency_score, quality_score, compliance_score);
        // 0.5 * 100.0 + 0.3 * 50.0 + 0.2 * 80.0 + 0 * 0 = 50.0 + 15.0 + 16.0 = 81.0
        assert_eq!(total, 81.0);
    }
}
