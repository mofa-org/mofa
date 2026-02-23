use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub node_id: String,
    pub cluster_size: usize,
    pub discovery_method: DiscoveryMethod,
    pub gossip_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    Static(Vec<String>),
    DNS,
    Consul,
    etcd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultToleranceConfig {
    pub replication_factor: usize,
    pub failover_timeout_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub max_retry_attempts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateReplicationConfig {
    pub replication_mode: ReplicationMode,
    pub consensus_protocol: ConsensusProtocol,
    pub sync_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplicationMode {
    SingleLeader,
    MultiLeader,
    ChainReplication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusProtocol {
    Raft,
    Paxos,
    Byzantine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    pub strategy: LoadBalancingStrategy,
    pub health_check_interval_ms: u64,
    pub capacity_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    Weighted,
    CapabilityBased,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDiscoveryConfig {
    pub registry_type: RegistryType,
    pub ttl_seconds: u64,
    pub refresh_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegistryType {
    Consul,
    etcd,
    ZooKeeper,
    DNS,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedRuntimeConfig {
    pub cluster: ClusterConfig,
    pub fault_tolerance: FaultToleranceConfig,
    pub state_replication: StateReplicationConfig,
    pub load_balancing: LoadBalancingConfig,
    pub service_discovery: ServiceDiscoveryConfig,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::now_v7().to_string(),
            cluster_size: 3,
            discovery_method: DiscoveryMethod::Static(vec![]),
            gossip_interval_ms: 1000,
        }
    }
}

impl Default for FaultToleranceConfig {
    fn default() -> Self {
        Self {
            replication_factor: 2,
            failover_timeout_ms: 5000,
            heartbeat_interval_ms: 1000,
            max_retry_attempts: 3,
        }
    }
}

impl Default for StateReplicationConfig {
    fn default() -> Self {
        Self {
            replication_mode: ReplicationMode::SingleLeader,
            consensus_protocol: ConsensusProtocol::Raft,
            sync_interval_ms: 100,
        }
    }
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalancingStrategy::LeastConnections,
            health_check_interval_ms: 5000,
            capacity_threshold: 0.8,
        }
    }
}

impl Default for ServiceDiscoveryConfig {
    fn default() -> Self {
        Self {
            registry_type: RegistryType::Consul,
            ttl_seconds: 30,
            refresh_interval_ms: 10000,
        }
    }
}

impl Default for DistributedRuntimeConfig {
    fn default() -> Self {
        Self {
            cluster: ClusterConfig::default(),
            fault_tolerance: FaultToleranceConfig::default(),
            state_replication: StateReplicationConfig::default(),
            load_balancing: LoadBalancingConfig::default(),
            service_discovery: ServiceDiscoveryConfig::default(),
        }
    }
}
