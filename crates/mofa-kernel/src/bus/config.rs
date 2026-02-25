use std::collections::HashMap;

/// Strategy to handle backpressure when the queue is full
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackpressureStrategy {
    /// Block the sender until space is available in the receiver's queue
    Block,
    /// Drop the oldest message in the receiver's queue
    DropOldest,
    /// Drop a low priority message in the queue to make room for a higher priority message.
    /// If there are no lower priority messages, drop the new message.
    DropLowPriority,
}

/// Configuration for the Event Bus
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Default capacity for channels
    pub default_capacity: usize,
    /// Default backpressure strategy
    pub default_strategy: BackpressureStrategy,
    /// Topic specific configurations (topic -> (capacity, strategy))
    pub topic_configs: HashMap<String, (usize, BackpressureStrategy)>,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            default_capacity: 100,
            default_strategy: BackpressureStrategy::DropOldest, // Legacy tokio broadcast behavior
            topic_configs: HashMap::new(),
        }
    }
}

impl EventBusConfig {
    /// Create a new configuration with specific defaults
    pub fn new(capacity: usize, strategy: BackpressureStrategy) -> Self {
        Self {
            default_capacity: capacity,
            default_strategy: strategy,
            topic_configs: HashMap::new(),
        }
    }

    /// Add a topic specific configuration
    pub fn with_topic(mut self, topic: String, capacity: usize, strategy: BackpressureStrategy) -> Self {
        self.topic_configs.insert(topic, (capacity, strategy));
        self
    }

    /// Get the configuration for a specific topic
    pub fn get_topic_config(&self, topic: &str) -> (usize, BackpressureStrategy) {
        self.topic_configs
            .get(topic)
            .cloned()
            .unwrap_or((self.default_capacity, self.default_strategy.clone()))
    }
}
