use std::fmt;

use super::orchestration::{InferenceRequest, RoutedBackend};

/// A set of candidate backends for a routing decision.
#[derive(Debug, Clone)]
pub struct CandidateSet {
    pub candidates: Vec<RoutedBackend>,
}

pub trait RoutingPredicateClone {
    fn clone_box(&self) -> Box<dyn RoutingPredicate>;
}

impl<T> RoutingPredicateClone for T
where
    T: 'static + RoutingPredicate + Clone,
{
    fn clone_box(&self) -> Box<dyn RoutingPredicate> {
        Box::new(self.clone())
    }
}

/// Represents custom filtering/routing logic that can be provided by the user.
/// It MUST be `Send + Sync` to participate in multi-threaded execution.
pub trait RoutingPredicate: Send + Sync + fmt::Debug + RoutingPredicateClone {
    /// Evaluates candidate backends against a specific inference request.
    /// Returns a score or admissibility metric for the candidates.
    fn evaluate(&self, request: &InferenceRequest, candidates: &CandidateSet) -> f32;
}

impl Clone for Box<dyn RoutingPredicate> {
    fn clone(&self) -> Box<dyn RoutingPredicate> {
        self.clone_box()
    }
}

/// The specific optimization goal the routing policy should prioritize.
/// E.g., choosing to optimize for money vs latency or user-defined traits.
#[non_exhaustive]
pub enum RoutingObjective {
    LocalOnly,
    CloudOnly,
    LocalFirstWithCloudFallback,
    CostOptimized,
    LatencyOptimized,
    QualityMaximized,
    ComplianceFirst,
    Custom(Box<dyn RoutingPredicate>),
}

impl Default for RoutingObjective {
    fn default() -> Self {
        RoutingObjective::LocalFirstWithCloudFallback
    }
}

impl Clone for RoutingObjective {
    fn clone(&self) -> Self {
        match self {
            Self::LocalOnly => Self::LocalOnly,
            Self::CloudOnly => Self::CloudOnly,
            Self::LocalFirstWithCloudFallback => Self::LocalFirstWithCloudFallback,
            Self::CostOptimized => Self::CostOptimized,
            Self::LatencyOptimized => Self::LatencyOptimized,
            Self::QualityMaximized => Self::QualityMaximized,
            Self::ComplianceFirst => Self::ComplianceFirst,
            Self::Custom(predicate) => Self::Custom(predicate.clone()),
        }
    }
}

impl fmt::Debug for RoutingObjective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalOnly => write!(f, "LocalOnly"),
            Self::CloudOnly => write!(f, "CloudOnly"),
            Self::LocalFirstWithCloudFallback => write!(f, "LocalFirstWithCloudFallback"),
            Self::CostOptimized => write!(f, "CostOptimized"),
            Self::LatencyOptimized => write!(f, "LatencyOptimized"),
            Self::QualityMaximized => write!(f, "QualityMaximized"),
            Self::ComplianceFirst => write!(f, "ComplianceFirst"),
            Self::Custom(predicate) => write!(f, "Custom({:?})", predicate),
        }
    }
}

impl PartialEq for RoutingObjective {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::LocalOnly, Self::LocalOnly) => true,
            (Self::CloudOnly, Self::CloudOnly) => true,
            (Self::LocalFirstWithCloudFallback, Self::LocalFirstWithCloudFallback) => true,
            (Self::CostOptimized, Self::CostOptimized) => true,
            (Self::LatencyOptimized, Self::LatencyOptimized) => true,
            (Self::QualityMaximized, Self::QualityMaximized) => true,
            (Self::ComplianceFirst, Self::ComplianceFirst) => true,
            // We cannot easily compare two trait objects, so we consider them unequal
            // by default for PartialEq, unless they are the exact same pointer.
            // But since this is just an enum for policy equality checking, false is safe.
            (Self::Custom(_), Self::Custom(_)) => false,
            _ => false,
        }
    }
}

use serde::{Deserialize, Serialize, Serializer, Deserializer, de::Error};

impl Serialize for RoutingObjective {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::LocalOnly => serializer.serialize_str("local_only"),
            Self::CloudOnly => serializer.serialize_str("cloud_only"),
            Self::LocalFirstWithCloudFallback => serializer.serialize_str("local_first_with_cloud_fallback"),
            Self::CostOptimized => serializer.serialize_str("cost_optimized"),
            Self::LatencyOptimized => serializer.serialize_str("latency_optimized"),
            Self::QualityMaximized => serializer.serialize_str("quality_maximized"),
            Self::ComplianceFirst => serializer.serialize_str("compliance_first"),
            Self::Custom(_) => serializer.serialize_str("custom"),
        }
    }
}

impl<'de> Deserialize<'de> for RoutingObjective {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "local_only" => Ok(Self::LocalOnly),
            "cloud_only" => Ok(Self::CloudOnly),
            "local_first_with_cloud_fallback" => Ok(Self::LocalFirstWithCloudFallback),
            "cost_optimized" => Ok(Self::CostOptimized),
            "latency_optimized" => Ok(Self::LatencyOptimized),
            "quality_maximized" => Ok(Self::QualityMaximized),
            "compliance_first" => Ok(Self::ComplianceFirst),
            "local-only" => Ok(Self::LocalOnly),
            "cloud-only" => Ok(Self::CloudOnly),
            "local-first" => Ok(Self::LocalFirstWithCloudFallback),
            "cost-optimized" => Ok(Self::CostOptimized),
            "latency-optimized" => Ok(Self::LatencyOptimized),
            // Custom predicates cannot be deserialized cleanly this way.
            _ => Err(D::Error::custom("unknown or unsupported routing objective variant")),
        }
    }
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
