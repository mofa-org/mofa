//! Generic file-based persisted store for CLI state.

use crate::CliError;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

pub struct PersistedStore<T> {
    dir: PathBuf,
    _phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> PersistedStore<T> {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self, CliError> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            _phantom: PhantomData,
        })
    }

    pub fn save(&self, id: &str, item: &T) -> Result<(), CliError> {
        let path = self.path_for(id);
        let payload = serde_json::to_vec_pretty(item)?;
        fs::write(path, payload)?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<T>, CliError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(None);
        }

        let payload = fs::read(path)?;
        let item = serde_json::from_slice(&payload)?;
        Ok(Some(item))
    }

    pub fn list(&self) -> Result<Vec<(String, T)>, CliError> {
        let mut items = Vec::new();

        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let id = match path.file_stem().and_then(|stem| stem.to_str()) {
                Some(stem) => stem.to_string(),
                None => continue,
            };

            let payload = fs::read(path)?;
            let item = serde_json::from_slice(&payload)?;
            items.push((id, item));
        }

        items.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(items)
    }

    pub fn delete(&self, id: &str) -> Result<bool, CliError> {
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(path)?;
        Ok(true)
    }

    fn path_for(&self, id: &str) -> PathBuf {
        let safe_id: String = id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        let file_name = if safe_id.is_empty() {
            "_".to_string()
        } else {
            safe_id
        };

        self.dir.join(format!("{}.json", file_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
    struct TestEntry {
        name: String,
        value: u32,
    }

    #[test]
    fn test_save_and_get() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();
        let entry = TestEntry {
            name: "alpha".to_string(),
            value: 1,
        };

        store.save("alpha", &entry).unwrap();
        let loaded = store.get("alpha").unwrap();
        assert_eq!(loaded, Some(entry));
    }

    #[test]
    fn test_get_returns_none_for_missing() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        assert!(store.get("missing").unwrap().is_none());
    }

    #[test]
    fn test_list_returns_all() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();
        store
            .save(
                "a",
                &TestEntry {
                    name: "a".to_string(),
                    value: 1,
                },
            )
            .unwrap();
        store
            .save(
                "b",
                &TestEntry {
                    name: "b".to_string(),
                    value: 2,
                },
            )
            .unwrap();

        let items = store.list().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, "a");
        assert_eq!(items[1].0, "b");
    }

    #[test]
    fn test_delete() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();
        store
            .save(
                "x",
                &TestEntry {
                    name: "x".to_string(),
                    value: 9,
                },
            )
            .unwrap();

        assert!(store.delete("x").unwrap());
        assert!(store.get("x").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_returns_false() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        assert!(!store.delete("ghost").unwrap());
    }

    #[test]
    fn test_overwrite() {
        let temp = TempDir::new().unwrap();
        let store = PersistedStore::<TestEntry>::new(temp.path()).unwrap();

        store
            .save(
                "k",
                &TestEntry {
                    name: "old".to_string(),
                    value: 1,
                },
            )
            .unwrap();
        store
            .save(
                "k",
                &TestEntry {
                    name: "new".to_string(),
                    value: 2,
                },
            )
            .unwrap();

        let loaded = store.get("k").unwrap().unwrap();
        assert_eq!(loaded.name, "new");
        assert_eq!(loaded.value, 2);
    }

    #[test]
    fn test_survives_new_instance() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().to_path_buf();

        {
            let store = PersistedStore::<TestEntry>::new(&path).unwrap();
            store
                .save(
                    "persisted",
                    &TestEntry {
                        name: "persisted".to_string(),
                        value: 7,
                    },
                )
                .unwrap();
        }

        let new_store = PersistedStore::<TestEntry>::new(&path).unwrap();
        let loaded = new_store.get("persisted").unwrap();
        assert_eq!(
            loaded,
            Some(TestEntry {
                name: "persisted".to_string(),
                value: 7
            })
        );
    }
}
