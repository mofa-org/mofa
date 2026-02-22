//! Event definitions for intelligent operation and maintenance
//!
//! This module defines the event types and related structures that the运维Agent
//! needs to handle.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::SystemTime;

/// Event type enumeration
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum EventType {
    /// Server failure (e.g., crash, high CPU/memory, disk full)
    ServerFault,
    /// Network attack (e.g., DDoS, intrusion attempt)
    NetworkAttack,
    /// Service exception (e.g., API 500 errors, database connection failure)
    ServiceException,
    /// Resource warning (e.g., approaching disk/memory limits)
    ResourceWarning,
    /// Security vulnerability (e.g., unpatched software, weak passwords)
    SecurityVulnerability,
    /// Custom event type
    Custom(String),
}

/// Event priority levels
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    /// Informational event, no action needed
    Low,
    /// Warning event, requires attention
    Medium,
    /// Critical event, requires immediate action
    High,
    /// Emergency event, system is at risk
    Emergency,
}

impl fmt::Display for EventPriority {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EventPriority::Low => write!(f, "Low"),
            EventPriority::Medium => write!(f, "Medium"),
            EventPriority::High => write!(f, "High"),
            EventPriority::Emergency => write!(f, "Emergency"),
        }
    }
}

/// Impact scope of the event
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImpactScope {
    /// Single component
    Component(String),
    /// Single service instance
    Instance(String),
    /// Multiple service instances
    MultipleInstances(Vec<String>),
    /// Entire service
    Service(String),
    /// Multiple services
    MultipleServices(Vec<String>),
    /// Entire system
    System,
    /// Custom scope
    Custom(String),
}

/// Event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID
    pub id: String,
    /// Event type
    pub event_type: EventType,
    /// Event priority
    pub priority: EventPriority,
    /// Impact scope
    pub scope: ImpactScope,
    /// Event source (e.g., server name, monitoring system)
    pub source: String,
    /// Event description
    pub description: String,
    /// Event timestamp
    pub timestamp: SystemTime,
    /// Additional event data
    pub data: serde_json::Value,
    /// Event status
    pub status: EventStatus,
}

/// Event status
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum EventStatus {
    /// New event, not processed yet
    New,
    /// Event is being processed
    Processing,
    /// Event has been resolved
    Resolved,
    /// Event was ignored
    Ignored,
    /// Event requires manual intervention
    ManualInterventionNeeded,
}

impl Event {
    /// Create a new event with default status New
    pub fn new(
        event_type: EventType,
        priority: EventPriority,
        scope: ImpactScope,
        source: String,
        description: String,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            event_type,
            priority,
            scope,
            source,
            description,
            timestamp: SystemTime::now(),
            data,
            status: EventStatus::New,
        }
    }

    /// Convert event to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse event from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Update event status
    pub fn update_status(&mut self, new_status: EventStatus) {
        self.status = new_status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            EventType::ServerFault,
            EventPriority::Emergency,
            ImpactScope::System,
            "monitoring-system".to_string(),
            "Database server crash detected".to_string(),
            serde_json::json!({ "server": "db-01", "reason": "OOM" }),
        );

        assert_eq!(event.event_type, EventType::ServerFault);
        assert_eq!(event.priority, EventPriority::Emergency);
        assert_eq!(event.status, EventStatus::New);
        assert!(!event.id.is_empty());
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new(
            EventType::NetworkAttack,
            EventPriority::High,
            ImpactScope::Service("api-gateway".to_string()),
            "ids-system".to_string(),
            "DDoS attack detected".to_string(),
            serde_json::json!({ "source_ip": "192.168.1.100", "traffic": "10Gbps" }),
        );

        let json = event.to_json().unwrap();
        let deserialized = Event::from_json(&json).unwrap();

        assert_eq!(deserialized.event_type, EventType::NetworkAttack);
        assert_eq!(deserialized.priority, EventPriority::High);
    }
}
