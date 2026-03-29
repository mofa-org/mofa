//! Swarm Audit Telemetry Adapter
//!
//! # Mapping
//!
//! | AuditEventKind          | DebugEvent        |
//! |-------------------------|-------------------|
//! | SwarmStarted            | WorkflowStart     |
//! | SwarmCompleted          | WorkflowEnd       |
//! | SubtaskStarted          | NodeStart         |
//! | SubtaskCompleted        | NodeEnd           |
//! | SubtaskFailed / SLABreach | Error           |
//! | HITLDecision / AgentReassigned | StateChange |
//! | All others              | NodeStart (generic) |

use mofa_kernel::workflow::telemetry::DebugEvent;

use crate::swarm::config::{AuditEvent, AuditEventKind};

/// Convert a swarm [`AuditEvent`] into a kernel [`DebugEvent`].
pub fn audit_to_debug(event: &AuditEvent) -> DebugEvent {
    let ts = u64::try_from(event.timestamp.timestamp_millis()).unwrap_or(u64::MAX);

    match &event.kind {
        // Swarm lifecycle
        AuditEventKind::SwarmStarted => {
            let swarm_id = event
                .data
                .get("swarm_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            DebugEvent::WorkflowStart {
                workflow_id: swarm_id.clone(),
                execution_id: swarm_id,
                timestamp_ms: ts,
            }
        }

        AuditEventKind::SwarmCompleted => {
            let swarm_id = event
                .data
                .get("swarm_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            DebugEvent::WorkflowEnd {
                workflow_id: swarm_id.clone(),
                execution_id: swarm_id,
                timestamp_ms: ts,
                status: "completed".to_string(),
            }
        }

        // Subtask lifecycle
        AuditEventKind::SubtaskStarted | AuditEventKind::AgentAssigned => {
            let node_id = event
                .data
                .get("subtask_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            DebugEvent::NodeStart {
                node_id,
                timestamp_ms: ts,
                state_snapshot: event.data.clone(),
            }
        }

        AuditEventKind::SubtaskCompleted => {
            let node_id = event
                .data
                .get("subtask_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            DebugEvent::NodeEnd {
                node_id,
                timestamp_ms: ts,
                state_snapshot: event.data.clone(),
                duration_ms: 0, // populated from SwarmMetrics when available
            }
        }

        AuditEventKind::SubtaskFailed | AuditEventKind::SLABreach => {
            let node_id = event
                .data
                .get("subtask_id")
                .and_then(|v| v.as_str())
                .map(str::to_string);

            DebugEvent::Error {
                node_id,
                timestamp_ms: ts,
                error: event.description.clone(),
            }
        }

        // HITL decisions
        AuditEventKind::HITLDecision => {
            let node_id = event
                .data
                .get("subtask_id")
                .and_then(|v| v.as_str())
                .unwrap_or("hitl")
                .to_string();

            DebugEvent::StateChange {
                node_id,
                timestamp_ms: ts,
                key: "hitl_decision".to_string(),
                old_value: None,
                new_value: event
                    .data
                    .get("decision")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }
        }

        // Agent reassignment
        AuditEventKind::AgentReassigned => {
            let node_id = event
                .data
                .get("subtask_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            DebugEvent::StateChange {
                node_id,
                timestamp_ms: ts,
                key: "assigned_agent".to_string(),
                old_value: event.data.get("previous_agent").cloned(),
                new_value: event
                    .data
                    .get("new_agent")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }
        }

        // informational events
        // HITLRequested, SLAWarning, TaskDecomposed, PatternSelected
        _ => {
            let node_id = event
                .data
                .get("subtask_id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{:?}", event.kind).to_lowercase());

            DebugEvent::NodeStart {
                node_id,
                timestamp_ms: ts,
                state_snapshot: event.data.clone(),
            }
        }
    }
}

/// Convert a slice of swarm audit events into a `Vec<DebugEvent>`.
pub fn audit_batch_to_debug(events: &[AuditEvent]) -> Vec<DebugEvent> {
    events.iter().map(audit_to_debug).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::config::{AuditEvent, AuditEventKind};
    use serde_json::json;

    fn make_event(kind: AuditEventKind, desc: &str, data: serde_json::Value) -> AuditEvent {
        AuditEvent::new(kind, desc).with_data(data)
    }

    #[test]
    fn test_swarm_started_maps_to_workflow_start() {
        let event = make_event(
            AuditEventKind::SwarmStarted,
            "Swarm started",
            json!({ "swarm_id": "swarm-abc" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::WorkflowStart { .. }));
        if let DebugEvent::WorkflowStart { workflow_id, .. } = debug {
            assert_eq!(workflow_id, "swarm-abc");
        }
    }

    #[test]
    fn test_swarm_completed_maps_to_workflow_end() {
        let event = make_event(
            AuditEventKind::SwarmCompleted,
            "Swarm completed",
            json!({ "swarm_id": "swarm-abc" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::WorkflowEnd { .. }));
        if let DebugEvent::WorkflowEnd { status, .. } = debug {
            assert_eq!(status, "completed");
        }
    }

    #[test]
    fn test_subtask_started_maps_to_node_start() {
        let event = make_event(
            AuditEventKind::SubtaskStarted,
            "Subtask started",
            json!({ "subtask_id": "task-1" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::NodeStart { .. }));
        if let DebugEvent::NodeStart { node_id, .. } = debug {
            assert_eq!(node_id, "task-1");
        }
    }

    #[test]
    fn test_agent_assigned_maps_to_node_start() {
        let event = make_event(
            AuditEventKind::AgentAssigned,
            "Agent assigned to subtask",
            json!({ "subtask_id": "task-1", "agent_id": "agent-3" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::NodeStart { .. }));
        if let DebugEvent::NodeStart {
            node_id,
            state_snapshot,
            ..
        } = debug
        {
            assert_eq!(node_id, "task-1");
            assert_eq!(state_snapshot["agent_id"], json!("agent-3"));
        }
    }

    #[test]
    fn test_subtask_completed_maps_to_node_end() {
        let event = make_event(
            AuditEventKind::SubtaskCompleted,
            "Subtask completed",
            json!({ "subtask_id": "task-1", "output": "done" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::NodeEnd { .. }));
        if let DebugEvent::NodeEnd { node_id, .. } = debug {
            assert_eq!(node_id, "task-1");
        }
    }

    #[test]
    fn test_subtask_failed_maps_to_error() {
        let event = make_event(
            AuditEventKind::SubtaskFailed,
            "timeout after 30s",
            json!({ "subtask_id": "task-2" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::Error { .. }));
        if let DebugEvent::Error { node_id, error, .. } = debug {
            assert_eq!(node_id.as_deref(), Some("task-2"));
            assert_eq!(error, "timeout after 30s");
        }
    }

    #[test]
    fn test_sla_breach_maps_to_error_without_node_id() {
        let event = make_event(
            AuditEventKind::SLABreach,
            "deadline exceeded",
            json!({}), // no subtask_id
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::Error { node_id: None, .. }));
    }

    #[test]
    fn test_hitl_decision_maps_to_state_change() {
        let event = make_event(
            AuditEventKind::HITLDecision,
            "approved",
            json!({ "subtask_id": "task-3", "decision": "approve" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::StateChange { .. }));
        if let DebugEvent::StateChange { key, new_value, .. } = debug {
            assert_eq!(key, "hitl_decision");
            assert_eq!(new_value, json!("approve"));
        }
    }

    #[test]
    fn test_agent_reassigned_maps_to_state_change() {
        let event = make_event(
            AuditEventKind::AgentReassigned,
            "agent swapped",
            json!({ "subtask_id": "task-4", "previous_agent": "a1", "new_agent": "a2" }),
        );
        let debug = audit_to_debug(&event);
        assert!(matches!(debug, DebugEvent::StateChange { .. }));
        if let DebugEvent::StateChange {
            key,
            old_value,
            new_value,
            ..
        } = debug
        {
            assert_eq!(key, "assigned_agent");
            assert_eq!(old_value, Some(json!("a1")));
            assert_eq!(new_value, json!("a2"));
        }
    }

    #[test]
    fn test_generic_events_map_to_node_start() {
        for kind in [
            AuditEventKind::HITLRequested,
            AuditEventKind::SLAWarning,
            AuditEventKind::TaskDecomposed,
            AuditEventKind::PatternSelected,
        ] {
            let event = make_event(kind, "informational", json!({}));
            let debug = audit_to_debug(&event);
            assert!(
                matches!(debug, DebugEvent::NodeStart { .. }),
                "expected NodeStart for informational event"
            );
        }
    }

    #[test]
    fn test_audit_batch_to_debug_preserves_order() {
        let events = vec![
            make_event(
                AuditEventKind::SwarmStarted,
                "start",
                json!({ "swarm_id": "s1" }),
            ),
            make_event(
                AuditEventKind::SubtaskStarted,
                "t1",
                json!({ "subtask_id": "t1" }),
            ),
            make_event(
                AuditEventKind::SwarmCompleted,
                "done",
                json!({ "swarm_id": "s1" }),
            ),
        ];
        let debug_events = audit_batch_to_debug(&events);
        assert_eq!(debug_events.len(), 3);
        assert!(matches!(debug_events[0], DebugEvent::WorkflowStart { .. }));
        assert!(matches!(debug_events[1], DebugEvent::NodeStart { .. }));
        assert!(matches!(debug_events[2], DebugEvent::WorkflowEnd { .. }));
    }

    #[test]
    fn test_timestamp_is_preserved() {
        let event = make_event(
            AuditEventKind::SubtaskStarted,
            "check ts",
            json!({ "subtask_id": "t" }),
        );
        let expected_ts = event.timestamp.timestamp_millis() as u64;
        let debug = audit_to_debug(&event);
        assert_eq!(debug.timestamp_ms(), expected_ts);
    }
}
