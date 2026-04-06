use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::swarm::config::{AuditEvent, AuditEventKind};

/// a single timestamped entry with a stable id for deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub event: AuditEvent,
}

impl AuditEntry {
    fn new(event: AuditEvent) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            event,
        }
    }
}

/// observer called synchronously on every recorded entry
pub trait SwarmAuditor: Send + Sync {
    fn on_entry(&self, _entry: &AuditEntry) {}
}

#[derive(Default)]
struct LogInner {
    entries: Vec<AuditEntry>,
}

/// thread-safe structured audit log; cheap to clone — all clones share the same log
#[derive(Clone)]
pub struct SwarmAuditLog {
    inner: Arc<Mutex<LogInner>>,
    auditor: Option<Arc<dyn SwarmAuditor>>,
}

impl Default for SwarmAuditLog {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogInner::default())),
            auditor: None,
        }
    }
}

impl SwarmAuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_auditor(mut self, auditor: impl SwarmAuditor + 'static) -> Self {
        self.auditor = Some(Arc::new(auditor));
        self
    }

    pub fn record(&self, event: AuditEvent) {
        let entry = AuditEntry::new(event);
        if let Some(ref a) = self.auditor {
            a.on_entry(&entry);
        }
        self.inner
            .lock()
            .expect("audit log poisoned")
            .entries
            .push(entry);
    }

    pub fn all_entries(&self) -> Vec<AuditEntry> {
        self.inner
            .lock()
            .expect("audit log poisoned")
            .entries
            .clone()
    }

    pub fn entries_by_kind(&self, kind: &AuditEventKind) -> Vec<AuditEntry> {
        self.inner
            .lock()
            .expect("audit log poisoned")
            .entries
            .iter()
            .filter(|e| &e.event.kind == kind)
            .cloned()
            .collect()
    }

    pub fn entries_since(&self, since: DateTime<Utc>) -> Vec<AuditEntry> {
        self.inner
            .lock()
            .expect("audit log poisoned")
            .entries
            .iter()
            .filter(|e| e.event.timestamp >= since)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("audit log poisoned")
            .entries
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// exports entries as the plain vec expected by SwarmResult.audit_events
    pub fn to_audit_events(&self) -> Vec<AuditEvent> {
        self.all_entries().into_iter().map(|e| e.event).collect()
    }
}
