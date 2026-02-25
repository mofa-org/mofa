use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersion {
    pub version: String,
    pub created_at: u64,
    pub author: Option<String>,
    pub changelog: String,
    pub workflow_id: String,
    pub definition: WorkflowDefinitionHash,
    pub status: VersionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinitionHash {
    pub hash: String,
    pub format: SerializationFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializationFormat {
    Json,
    Yaml,
    Ron,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionStatus {
    Draft,
    Published,
    Archived,
    Deprecated,
}

impl WorkflowVersion {
    pub fn new(
        version: String,
        workflow_id: String,
        definition_hash: String,
        changelog: String,
    ) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            version,
            created_at,
            author: None,
            changelog,
            workflow_id,
            definition: WorkflowDefinitionHash {
                hash: definition_hash,
                format: SerializationFormat::Json,
            },
            status: VersionStatus::Draft,
        }
    }

    pub fn publish(&mut self) {
        self.status = VersionStatus::Published;
    }

    pub fn archive(&mut self) {
        self.status = VersionStatus::Archived;
    }

    pub fn deprecate(&mut self) {
        self.status = VersionStatus::Deprecated;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowHistory {
    pub workflow_id: String,
    pub versions: Vec<WorkflowVersion>,
    pub current_version: Option<String>,
}

impl WorkflowHistory {
    pub fn new(workflow_id: String) -> Self {
        Self {
            workflow_id,
            versions: Vec::new(),
            current_version: None,
        }
    }

    pub fn add_version(&mut self, version: WorkflowVersion) {
        self.versions.push(version);
    }

    pub fn get_version(&self, version: &str) -> Option<&WorkflowVersion> {
        self.versions.iter().find(|v| v.version == version)
    }

    pub fn set_current_version(&mut self, version: String) {
        self.current_version = Some(version);
    }

    pub fn rollback(&mut self, target_version: &str) -> Option<&WorkflowVersion> {
        if self.versions.iter().any(|v| v.version == target_version) {
            self.current_version = Some(target_version.to_string());
            self.get_version(target_version)
        } else {
            None
        }
    }
}

pub trait VersionStore: Send + Sync {
    fn save_version(&self, history: WorkflowHistory) -> Result<(), VersionError>;
    fn load_history(&self, workflow_id: &str) -> Result<Option<WorkflowHistory>, VersionError>;
    fn list_versions(&self, workflow_id: &str) -> Result<Vec<String>, VersionError>;
    fn delete_version(&self, workflow_id: &str, version: &str) -> Result<(), VersionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error(\"Version not found: {0}\")]
    NotFound(String),

    #[error(\"Invalid version: {0}\")]
    InvalidVersion(String),

    #[error(\"Storage error: {0}\")]
    StorageError(String),

    #[error(\"Circular dependency detected\")]
    CircularDependency,
}

pub type VersionResult<T> = Result<T, VersionError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDiff {
    pub from_version: String,
    pub to_version: String,
    pub changes: Vec<VersionChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionChange {
    Added { path: String, value: serde_json::Value },
    Removed { path: String, value: serde_json::Value },
    Modified { path: String, old_value: serde_json::Value, new_value: serde_json::Value },
}

pub struct VersionManager<S: VersionStore> {
    store: S,
}

impl<S: VersionStore> VersionManager<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn create_version(
        &self,
        workflow_id: String,
        definition: &str,
        version: String,
        changelog: String,
    ) -> VersionResult<WorkflowVersion> {
        let hash = calculate_hash(definition);

        let mut workflow_version = WorkflowVersion::new(version, workflow_id, hash, changelog);
        workflow_version.publish();

        let mut history = self.store.load_history(&workflow_version.workflow_id)?
            .unwrap_or_else(|| WorkflowHistory::new(workflow_version.workflow_id.clone()));

        history.add_version(workflow_version.clone());
        history.set_current_version(workflow_version.version.clone());

        self.store.save_version(history)?;

        Ok(workflow_version)
    }

    pub fn rollback(&self, workflow_id: &str, target_version: &str) -> VersionResult<WorkflowVersion> {
        let mut history = self.store.load_history(workflow_id)?
            .ok_or_else(|| VersionError::NotFound(workflow_id.to_string()))?;

        let version = history.rollback(target_version)
            .ok_or_else(|| VersionError::NotFound(target_version.to_string()))?;

        self.store.save_version(history)?;

        Ok(version.clone())
    }

    pub fn diff(&self, workflow_id: &str, from: &str, to: &str) -> VersionResult<VersionDiff> {
        let history = self.store.load_history(workflow_id)?
            .ok_or_else(|| VersionError::NotFound(workflow_id.to_string()))?;

        let from_version = history.get_version(from)
            .ok_or_else(|| VersionError::NotFound(from.to_string()))?;
        let to_version = history.get_version(to)
            .ok_or_else(|| VersionError::NotFound(to.to_string()))?;

        Ok(VersionDiff {
            from_version: from.to_string(),
            to_version: to.to_string(),
            changes: Vec::new(),
        })
    }
}

fn calculate_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!(\"{:x}\", hasher.finish())
}
