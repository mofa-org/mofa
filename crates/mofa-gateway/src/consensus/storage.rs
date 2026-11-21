//! Persistent storage for Raft state.
//!
//! This module provides persistent storage for Raft's persistent state:
//! - Current term
//! - Voted for (candidate that received vote in current term)
//! - Log entries
//!
//! Uses [redb](https://crates.io/crates/redb) — a pure-Rust embedded
//! key-value store — for efficient storage without external C/C++ dependencies.
//!
//! # Implementation Status
//!
//! **Complete** - Persistent storage backed by redb with an in-memory test helper

use crate::error::{ConsensusError, ConsensusResult};
use crate::types::{LogEntry, LogIndex, NodeId, Term};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::path::Path;

/// Single table holding all Raft persistent state.
const RAFT_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("raft_state");

/// Convert any redb error into a [`ConsensusError::Storage`].
fn storage_err<E: std::fmt::Display>(e: E) -> ConsensusError {
    ConsensusError::Storage(format!("redb error: {}", e))
}

/// Persistent storage for Raft state.
pub struct RaftStorage {
    db: Database,
    /// Optional temporary directory backing this database (tests).
    ///
    /// When `RaftStorage` is created via `RaftStorage::new()` for tests, we
    /// hold on to the `TempDir` so that the directory outlives the database
    /// and is cleaned up on drop instead of being leaked.
    #[cfg(test)]
    _temp_dir: Option<tempfile::TempDir>,
}

impl Default for RaftStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftStorage {
    /// Create a new Raft storage instance backed by a temporary directory.
    ///
    /// Mostly useful for tests; production code should use [`RaftStorage::open`]
    /// with a persistent path.
    pub fn new() -> Self {
        let tmp_dir = tempfile::TempDir::new()
            .expect("failed to create temporary directory for RaftStorage tests");
        let path = tmp_dir.path().to_path_buf();

        // Keep the TempDir alive inside RaftStorage so the directory outlives
        // the database for the duration of the test process, and is cleaned up
        // when the storage is dropped.
        let storage = Self::open(path).expect("failed to open redb-backed RaftStorage");
        Self {
            db: storage.db,
            #[cfg(test)]
            _temp_dir: Some(tmp_dir),
        }
    }

    /// Open or create a Raft storage at the given path.
    ///
    /// The path may be either a directory (a `raft.redb` database file is
    /// created inside it) or the path of the database file itself. Missing
    /// parent directories are created automatically.
    pub fn open<P: AsRef<Path>>(path: P) -> ConsensusResult<Self> {
        let path = path.as_ref();
        let db_path = if path.is_dir() {
            path.join("raft.redb")
        } else {
            path.to_path_buf()
        };
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConsensusError::Storage(format!("io error: {}", e)))?;
        }

        let db = Database::create(db_path).map_err(storage_err)?;

        // Ensure the table exists before any read occurs.
        let write_txn = db.begin_write().map_err(storage_err)?;
        {
            let _table = write_txn.open_table(RAFT_TABLE).map_err(storage_err)?;
        }
        write_txn.commit().map_err(storage_err)?;

        Ok(Self {
            db,
            #[cfg(test)]
            _temp_dir: None,
        })
    }

    /// Save the current term.
    pub fn save_term(&self, term: Term) -> ConsensusResult<()> {
        let value = bincode::serialize(&term.0)
            .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
        let write_txn = self.db.begin_write().map_err(storage_err)?;
        {
            let mut table = write_txn.open_table(RAFT_TABLE).map_err(storage_err)?;
            table
                .insert("current_term", value.as_slice())
                .map_err(storage_err)?;
        }
        write_txn.commit().map_err(storage_err)?;
        Ok(())
    }

    /// Load the current term.
    pub fn load_term(&self) -> ConsensusResult<Option<Term>> {
        let read_txn = self.db.begin_read().map_err(storage_err)?;
        let table = read_txn.open_table(RAFT_TABLE).map_err(storage_err)?;
        match table.get("current_term").map_err(storage_err)? {
            Some(value) => {
                let term: u64 = bincode::deserialize(value.value())
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                Ok(Some(Term::new(term)))
            }
            None => Ok(None),
        }
    }

    /// Save the voted-for candidate.
    pub fn save_voted_for(&self, node_id: Option<&NodeId>) -> ConsensusResult<()> {
        let write_txn = self.db.begin_write().map_err(storage_err)?;
        {
            let mut table = write_txn.open_table(RAFT_TABLE).map_err(storage_err)?;
            match node_id {
                Some(id) => {
                    let value = bincode::serialize(&id.0)
                        .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                    table
                        .insert("voted_for", value.as_slice())
                        .map_err(storage_err)?;
                }
                None => {
                    table.remove("voted_for").map_err(storage_err)?;
                }
            }
        }
        write_txn.commit().map_err(storage_err)?;
        Ok(())
    }

    /// Load the voted-for candidate.
    pub fn load_voted_for(&self) -> ConsensusResult<Option<NodeId>> {
        let read_txn = self.db.begin_read().map_err(storage_err)?;
        let table = read_txn.open_table(RAFT_TABLE).map_err(storage_err)?;
        match table.get("voted_for").map_err(storage_err)? {
            Some(value) => {
                let id: String = bincode::deserialize(value.value())
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                Ok(Some(NodeId::new(id)))
            }
            None => Ok(None),
        }
    }

    /// Append a log entry.
    pub fn append_log_entry(&self, entry: &LogEntry) -> ConsensusResult<()> {
        let key = format!("log:{}", entry.index.0);
        let value = bincode::serialize(entry)
            .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
        let write_txn = self.db.begin_write().map_err(storage_err)?;
        {
            let mut table = write_txn.open_table(RAFT_TABLE).map_err(storage_err)?;
            table
                .insert(key.as_str(), value.as_slice())
                .map_err(storage_err)?;
        }
        write_txn.commit().map_err(storage_err)?;
        Ok(())
    }

    /// Load a log entry at the given index.
    pub fn load_log_entry(&self, index: LogIndex) -> ConsensusResult<Option<LogEntry>> {
        let key = format!("log:{}", index.0);
        let read_txn = self.db.begin_read().map_err(storage_err)?;
        let table = read_txn.open_table(RAFT_TABLE).map_err(storage_err)?;
        match table.get(key.as_str()).map_err(storage_err)? {
            Some(value) => {
                let entry: LogEntry = bincode::deserialize(value.value())
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Get all log entries.
    pub fn load_all_log_entries(&self) -> ConsensusResult<Vec<LogEntry>> {
        let mut entries = Vec::new();
        let read_txn = self.db.begin_read().map_err(storage_err)?;
        let table = read_txn.open_table(RAFT_TABLE).map_err(storage_err)?;

        for item_result in table.iter().map_err(storage_err)? {
            let (key, value) = item_result.map_err(storage_err)?;
            if key.value().starts_with("log:") {
                let entry: LogEntry = bincode::deserialize(value.value())
                    .map_err(|e: bincode::Error| ConsensusError::Storage(e.to_string()))?;
                entries.push(entry);
            }
        }

        // Sort by index (lexicographic key order does not match numeric order)
        entries.sort_by_key(|e| e.index.0);
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_term() {
        let temp_dir = TempDir::new().unwrap();
        let storage = RaftStorage::open(temp_dir.path()).unwrap();

        // Save and load term
        storage.save_term(Term::new(5)).unwrap();
        let term = storage.load_term().unwrap();
        assert_eq!(term, Some(Term::new(5)));
    }

    #[tokio::test]
    async fn test_storage_persistence_across_reopen() {
        let temp_dir = TempDir::new().unwrap();
        {
            let storage = RaftStorage::open(temp_dir.path()).unwrap();
            storage.save_term(Term::new(7)).unwrap();
            let node_id = NodeId::new("node-1");
            storage.save_voted_for(Some(&node_id)).unwrap();
            storage
                .append_log_entry(&LogEntry {
                    term: Term::new(1),
                    index: LogIndex::new(1),
                    data: b"entry1".to_vec(),
                })
                .unwrap();
        }
        // Reopen from the same file: state must survive the restart
        let storage = RaftStorage::open(temp_dir.path()).unwrap();
        assert_eq!(storage.load_term().unwrap(), Some(Term::new(7)));
        assert_eq!(
            storage.load_voted_for().unwrap(),
            Some(NodeId::new("node-1"))
        );
        assert_eq!(storage.load_all_log_entries().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_storage_voted_for() {
        let temp_dir = TempDir::new().unwrap();
        let storage = RaftStorage::open(temp_dir.path()).unwrap();

        // Save and load voted_for
        let node_id = NodeId::new("node-1");
        storage.save_voted_for(Some(&node_id)).unwrap();
        let loaded = storage.load_voted_for().unwrap();
        assert_eq!(loaded, Some(node_id));

        // Clear voted_for
        storage.save_voted_for(None).unwrap();
        let loaded = storage.load_voted_for().unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_storage_log_entries() {
        let temp_dir = TempDir::new().unwrap();
        let storage = RaftStorage::open(temp_dir.path()).unwrap();

        // Append log entries
        let entry1 = LogEntry {
            term: Term::new(1),
            index: LogIndex::new(1),
            data: b"entry1".to_vec(),
        };
        let entry2 = LogEntry {
            term: Term::new(1),
            index: LogIndex::new(2),
            data: b"entry2".to_vec(),
        };

        storage.append_log_entry(&entry1).unwrap();
        storage.append_log_entry(&entry2).unwrap();

        // Load entries
        let loaded1 = storage.load_log_entry(LogIndex::new(1)).unwrap();
        assert_eq!(loaded1.as_ref().map(|e| &e.data), Some(&b"entry1".to_vec()));

        let all = storage.load_all_log_entries().unwrap();
        assert_eq!(all.len(), 2);
    }
}
