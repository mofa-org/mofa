//! File based checkpoint store

use mofa_kernel::checkpoint::{CheckpointError, CheckpointStore, CheckpointSummary};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// File based checkpoint store
pub struct FileCheckpointStore {
    dir: PathBuf,
}

impl FileCheckpointStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self, CheckpointError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)
            .map_err(|e| CheckpointError::Storage(format!("create dir: {}", e)))?;
        Ok(Self { dir })
    }

    fn file_path(&self, execution_id: &str, label: &str) -> PathBuf {
        let safe_id = execution_id.replace(['/', '\\', ':', '*', '?'], "_");
        let safe_label = label.replace(['/', '\\', ':', '*', '?'], "_");
        self.dir.join(format!("{}_{}.json", safe_id, safe_label))
    }

    fn read_wrapper(&self, path: &Path) -> Result<CheckpointWrapper, CheckpointError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| CheckpointError::Storage(e.to_string()))?;
        serde_json::from_str(&contents)
            .map_err(|e| CheckpointError::Serialization(e.to_string()))
    }
}

/// On-disk checkpoint envelope
#[derive(serde::Serialize, serde::Deserialize)]
struct CheckpointWrapper {
    execution_id: String,
    workflow_id: String,
    label: String,
    created_at: u64,
    node_count: usize,
    data: serde_json::Value,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[async_trait]
impl CheckpointStore for FileCheckpointStore {
    async fn save(
        &self,
        execution_id: &str,
        workflow_id: &str,
        label: &str,
        data: &serde_json::Value,
    ) -> Result<(), CheckpointError> {
        let node_count = data
            .get("node_outputs")
            .and_then(|v| v.as_object())
            .map(|m| m.len())
            .unwrap_or(0);

        let wrapper = CheckpointWrapper {
            execution_id: execution_id.to_string(),
            workflow_id: workflow_id.to_string(),
            label: label.to_string(),
            created_at: now_ms(),
            node_count,
            data: data.clone(),
        };

        let json = serde_json::to_string_pretty(&wrapper)
            .map_err(|e| CheckpointError::Serialization(e.to_string()))?;

        let path = self.file_path(execution_id, label);
        std::fs::write(&path, json)
            .map_err(|e| CheckpointError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn load(
        &self,
        execution_id: &str,
    ) -> Result<Option<serde_json::Value>, CheckpointError> {

        let prefix = format!(
            "{}_",
            execution_id.replace(['/', '\\', ':', '*', '?'], "_")
        );

        let mut best: Option<CheckpointWrapper> = None;

        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| CheckpointError::Storage(e.to_string()))?;

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(".json") {
                if let Ok(wrapper) = self.read_wrapper(&entry.path()) {
                    if best.as_ref().map_or(true, |b| wrapper.created_at > b.created_at) {
                        best = Some(wrapper);
                    }
                }
            }
        }

        Ok(best.map(|w| w.data))
    }

    async fn list(
        &self,
        workflow_id: Option<&str>,
    ) -> Result<Vec<CheckpointSummary>, CheckpointError> {
        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| CheckpointError::Storage(e.to_string()))?;

        let mut summaries = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "json") {
                continue;
            }
            if let Ok(wrapper) = self.read_wrapper(&path) {
                if let Some(wid) = workflow_id {
                    if wrapper.workflow_id != wid {
                        continue;
                    }
                }
                summaries.push(CheckpointSummary {
                    execution_id: wrapper.execution_id,
                    workflow_id: wrapper.workflow_id,
                    created_at: wrapper.created_at,
                    node_count: wrapper.node_count,
                    label: wrapper.label,
                });
            }
        }

        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(summaries)
    }

    async fn delete(&self, execution_id: &str) -> Result<bool, CheckpointError> {
        let prefix = format!(
            "{}_",
            execution_id.replace(['/', '\\', ':', '*', '?'], "_")
        );

        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| CheckpointError::Storage(e.to_string()))?;

        let mut deleted = false;
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with(&prefix) {
                let _ = std::fs::remove_file(entry.path());
                deleted = true;
            }
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn sample_snapshot(exec_id: &str, wf_id: &str) -> serde_json::Value {
        json!({
            "version": 1,
            "workflow_id": wf_id,
            "execution_id": exec_id,
            "input": null,
            "node_outputs": {
                "step_1": "result_1",
                "step_2": "result_2"
            },
            "node_statuses": {
                "step_1": "Completed",
                "step_2": "Completed"
            },
            "variables": {},
            "checkpoints": [],
            "paused_at": null,
            "last_waiting_node": null,
            "total_wait_time_ms": 0
        })
    }

    fn temp_store() -> (TempDir, FileCheckpointStore) {
        let dir = TempDir::new().unwrap();
        let store = FileCheckpointStore::new(dir.path()).unwrap();
        (dir, store)
    }

    // Roundtrip: save checkpoint then load it back
    #[tokio::test]
    async fn test_save_and_load() {
        let (_dir, store) = temp_store();
        let data = sample_snapshot("exec-1", "wf-1");
        store.save("exec-1", "wf-1", "auto_1", &data).await.unwrap();

        let loaded = store.load("exec-1").await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded["workflow_id"], "wf-1");
        assert_eq!(loaded["node_outputs"]["step_1"], "result_1");
    }

    // Loading a missing execution returns None
    #[tokio::test]
    async fn test_load_nonexistent() {
        let (_dir, store) = temp_store();
        assert!(store.load("nope").await.unwrap().is_none());
    }

    // List returns all stored checkpoints
    #[tokio::test]
    async fn test_list_all() {
        let (_dir, store) = temp_store();
        store.save("exec-1", "wf-1", "cp1", &sample_snapshot("exec-1", "wf-1")).await.unwrap();
        store.save("exec-2", "wf-2", "cp1", &sample_snapshot("exec-2", "wf-2")).await.unwrap();

        let all = store.list(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    // List filters by workflow_id correctly
    #[tokio::test]
    async fn test_list_by_workflow() {
        let (_dir, store) = temp_store();
        store.save("exec-1", "wf-1", "cp1", &sample_snapshot("exec-1", "wf-1")).await.unwrap();
        store.save("exec-2", "wf-2", "cp1", &sample_snapshot("exec-2", "wf-2")).await.unwrap();
        store.save("exec-3", "wf-1", "cp1", &sample_snapshot("exec-3", "wf-1")).await.unwrap();

        let wf1 = store.list(Some("wf-1")).await.unwrap();
        assert_eq!(wf1.len(), 2);
        assert!(wf1.iter().all(|s| s.workflow_id == "wf-1"));
    }

    // Delete removes files, second delete returns false
    #[tokio::test]
    async fn test_delete() {
        let (_dir, store) = temp_store();
        store.save("exec-1", "wf-1", "cp1", &sample_snapshot("exec-1", "wf-1")).await.unwrap();
        assert!(store.delete("exec-1").await.unwrap());
        assert!(!store.delete("exec-1").await.unwrap());
        assert!(store.load("exec-1").await.unwrap().is_none());
    }

    // Same (exec_id, label) overwrites the previous file
    #[tokio::test]
    async fn test_overwrite_same_label() {
        let (_dir, store) = temp_store();
        let mut data = sample_snapshot("exec-1", "wf-1");
        store.save("exec-1", "wf-1", "latest", &data).await.unwrap();

        data["node_outputs"]["step_3"] = json!("result_3");
        store.save("exec-1", "wf-1", "latest", &data).await.unwrap();

        let loaded = store.load("exec-1").await.unwrap().unwrap();
        assert_eq!(loaded["node_outputs"]["step_3"], "result_3");
    }

    // node_count is extracted from the JSON payload
    #[tokio::test]
    async fn test_node_count_extracted() {
        let (_dir, store) = temp_store();
        store.save("exec-1", "wf-1", "cp1", &sample_snapshot("exec-1", "wf-1")).await.unwrap();
        let list = store.list(None).await.unwrap();
        assert_eq!(list[0].node_count, 2);
    }
}
