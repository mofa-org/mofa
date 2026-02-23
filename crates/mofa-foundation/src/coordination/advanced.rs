use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalConfig {
    pub max_levels: usize,
    pub delegation_threshold: f64,
    pub supervision_ratio: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConfig {
    pub bidding_strategy: BiddingStrategy,
    pub auction_timeout_ms: u64,
    pub min_bidders: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BiddingStrategy {
    Capability,
    Load,
    Random,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub voting_rounds: usize,
    pub quorum_threshold: f64,
    pub weight_by_reputation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    pub neighbor_radius: f64,
    pub influence_decay: f64,
    pub convergence_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedConfig {
    pub privacy_budget: f64,
    pub aggregation_method: AggregationMethod,
    pub cross_org_communication: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationMethod {
    FedAvg,
    FedProx,
    SecureAggregation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AdvancedCoordination {
    Hierarchical(HierarchicalConfig),
    Market(MarketConfig),
    Consensus(ConsensusConfig),
    Swarm(SwarmConfig),
    Federated(FederatedConfig),
}

impl Default for HierarchicalConfig {
    fn default() -> Self {
        Self {
            max_levels: 3,
            delegation_threshold: 0.7,
            supervision_ratio: 5,
        }
    }
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            bidding_strategy: BiddingStrategy::Capability,
            auction_timeout_ms: 5000,
            min_bidders: 2,
        }
    }
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            voting_rounds: 3,
            quorum_threshold: 0.5,
            weight_by_reputation: true,
        }
    }
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            neighbor_radius: 1.0,
            influence_decay: 0.9,
            convergence_threshold: 0.01,
        }
    }
}

impl Default for FederatedConfig {
    fn default() -> Self {
        Self {
            privacy_budget: 1.0,
            aggregation_method: AggregationMethod::FedAvg,
            cross_org_communication: false,
        }
    }
}
