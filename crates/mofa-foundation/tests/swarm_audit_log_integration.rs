use std::sync::{Arc, Mutex};

use chrono::Utc;
use mofa_foundation::swarm::{
    AuditEntry, AuditEvent, AuditEventKind, SwarmAuditLog, SwarmAuditor,
};

fn evt(kind: AuditEventKind, desc: &str) -> AuditEvent {
    AuditEvent::new(kind, desc)
}

#[test]
fn test_new_log_is_empty() {
    let log = SwarmAuditLog::new();
    assert!(log.is_empty());
    assert_eq!(log.len(), 0);
}

#[test]
fn test_record_increments_len() {
    let log = SwarmAuditLog::new();
    log.record(evt(AuditEventKind::SwarmStarted, "started"));
    assert_eq!(log.len(), 1);
    log.record(evt(AuditEventKind::SubtaskStarted, "t1"));
    assert_eq!(log.len(), 2);
}

#[test]
fn test_all_entries_preserves_order() {
    let log = SwarmAuditLog::new();
    log.record(evt(AuditEventKind::SwarmStarted, "a"));
    log.record(evt(AuditEventKind::SubtaskStarted, "b"));
    log.record(evt(AuditEventKind::SwarmCompleted, "c"));
    let entries = log.all_entries();
    assert_eq!(entries[0].event.kind, AuditEventKind::SwarmStarted);
    assert_eq!(entries[1].event.kind, AuditEventKind::SubtaskStarted);
    assert_eq!(entries[2].event.kind, AuditEventKind::SwarmCompleted);
}

#[test]
fn test_entries_by_kind_filters_correctly() {
    let log = SwarmAuditLog::new();
    log.record(evt(AuditEventKind::SubtaskStarted, "t1"));
    log.record(evt(AuditEventKind::SubtaskCompleted, "t1"));
    log.record(evt(AuditEventKind::SubtaskStarted, "t2"));
    log.record(evt(AuditEventKind::SubtaskFailed, "t3"));

    assert_eq!(log.entries_by_kind(&AuditEventKind::SubtaskStarted).len(), 2);
    assert_eq!(log.entries_by_kind(&AuditEventKind::SubtaskCompleted).len(), 1);
    assert_eq!(log.entries_by_kind(&AuditEventKind::SubtaskFailed).len(), 1);
    assert_eq!(log.entries_by_kind(&AuditEventKind::SwarmStarted).len(), 0);
}

#[test]
fn test_entries_since_boundary() {
    let log = SwarmAuditLog::new();
    let before = Utc::now();
    log.record(evt(AuditEventKind::SwarmStarted, "s"));
    log.record(evt(AuditEventKind::SubtaskStarted, "t"));

    assert_eq!(log.entries_since(before).len(), 2);

    let future = Utc::now() + chrono::Duration::hours(1);
    assert!(log.entries_since(future).is_empty());
}

#[test]
fn test_each_entry_has_unique_id() {
    let log = SwarmAuditLog::new();
    for _ in 0..10 {
        log.record(evt(AuditEventKind::SubtaskStarted, "t"));
    }
    let ids: std::collections::HashSet<String> =
        log.all_entries().into_iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 10);
}

#[test]
fn test_clone_shares_underlying_log() {
    let log = SwarmAuditLog::new();
    let log2 = log.clone();
    log.record(evt(AuditEventKind::SwarmStarted, "s"));
    // both clones see the same entry
    assert_eq!(log2.len(), 1);
    assert_eq!(log2.all_entries()[0].event.kind, AuditEventKind::SwarmStarted);
}

#[test]
fn test_to_audit_events_exports_correctly() {
    let log = SwarmAuditLog::new();
    log.record(evt(AuditEventKind::SwarmStarted, "start"));
    log.record(evt(AuditEventKind::SwarmCompleted, "done"));
    let events = log.to_audit_events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, AuditEventKind::SwarmStarted);
    assert_eq!(events[1].kind, AuditEventKind::SwarmCompleted);
}

#[test]
fn test_with_data_preserved() {
    let log = SwarmAuditLog::new();
    log.record(
        AuditEvent::new(AuditEventKind::AdmissionChecked, "gate passed")
            .with_data(serde_json::json!({"policies": 3, "denied": 0})),
    );
    let entries = log.all_entries();
    assert_eq!(entries[0].event.data["policies"], 3);
    assert_eq!(entries[0].event.data["denied"], 0);
}

struct CapturingAuditor {
    calls: Arc<Mutex<Vec<String>>>,
}

impl SwarmAuditor for CapturingAuditor {
    fn on_entry(&self, entry: &AuditEntry) {
        self.calls
            .lock()
            .unwrap()
            .push(entry.event.description.clone());
    }
}

#[test]
fn test_auditor_observer_fires_in_order() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let log = SwarmAuditLog::new().with_auditor(CapturingAuditor { calls: calls.clone() });
    log.record(evt(AuditEventKind::SubtaskStarted, "task-a"));
    log.record(evt(AuditEventKind::SubtaskCompleted, "task-a done"));
    let c = calls.lock().unwrap();
    assert_eq!(c.as_slice(), ["task-a", "task-a done"]);
}

#[test]
fn test_admission_checked_and_denied_variants() {
    let log = SwarmAuditLog::new();
    log.record(evt(AuditEventKind::AdmissionChecked, "gate checked"));
    log.record(evt(AuditEventKind::AdmissionDenied, "gate denied"));
    assert_eq!(log.entries_by_kind(&AuditEventKind::AdmissionChecked).len(), 1);
    assert_eq!(log.entries_by_kind(&AuditEventKind::AdmissionDenied).len(), 1);
}

#[test]
fn test_scheduler_lifecycle_variants() {
    let log = SwarmAuditLog::new();
    log.record(
        AuditEvent::new(AuditEventKind::SchedulerStarted, "sequential started")
            .with_data(serde_json::json!({"pattern": "sequential", "task_count": 5})),
    );
    log.record(
        AuditEvent::new(AuditEventKind::SchedulerCompleted, "sequential done")
            .with_data(serde_json::json!({"succeeded": 5, "failed": 0, "wall_time_ms": 420})),
    );
    let started = log.entries_by_kind(&AuditEventKind::SchedulerStarted);
    let completed = log.entries_by_kind(&AuditEventKind::SchedulerCompleted);
    assert_eq!(started[0].event.data["task_count"], 5);
    assert_eq!(completed[0].event.data["succeeded"], 5);
}

#[tokio::test]
async fn test_concurrent_records_all_arrive() {
    let log = SwarmAuditLog::new();
    let mut handles = Vec::new();
    for i in 0..30 {
        let l = log.clone();
        handles.push(tokio::spawn(async move {
            l.record(evt(AuditEventKind::SubtaskStarted, &format!("t{}", i)));
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    assert_eq!(log.len(), 30);
}
