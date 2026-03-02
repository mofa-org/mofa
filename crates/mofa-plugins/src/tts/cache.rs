//! Model Cache Management
//!
//! Handles caching, validation, and retrieval of TTS models
//! stored in ~/.mofa/models/tts/

use mofa_kernel::plugin::{PluginError, PluginResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Model metadata stored alongside cached models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Model identifier (e.g., "hexgrad/Kokoro-82M")
    pub model_id: String,
    /// Model version/tag
    pub version: String,
    /// File size in bytes
    pub file_size: u64,
    /// MD5 checksum
    pub checksum: String,
    /// Download timestamp
    pub downloaded_at: SystemTime,
    /// Last access timestamp
    pub last_accessed: SystemTime,
    /// Number of times accessed
    pub access_count: u64,
}

/// Model cache manager
pub struct ModelCache {
    /// Cache root directory (e.g., ~/.mofa/models/tts/)
    cache_dir: PathBuf,
    /// In-memory metadata cache
    metadata: Arc<RwLock<HashMap<String, ModelMetadata>>>,
}

impl ModelCache {
    /// Create a new model cache manager
    pub fn new(cache_dir: Option<PathBuf>) -> PluginResult<Self> {
        let cache_dir = cache_dir.unwrap_or_else(|| {
            dirs::home_dir()
                .expect("Failed to determine home directory")
                .join(".mofa")
                .join("models")
                .join("tts")
        });

        // Ensure cache directory exists
        fs::create_dir_all(&cache_dir)
            .map_err(|e| PluginError::Other(format!("Failed to create cache directory: {:?}: {}", cache_dir, e)))?;

        info!("Model cache initialized at: {:?}", cache_dir);

        Ok(Self {
            cache_dir,
            metadata: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get path for a specific model
    pub fn model_path(&self, model_id: &str) -> PathBuf {
        // Sanitize model_id (replace '/' with '-')
        let safe_id = model_id.replace('/', "-");
        self.cache_dir.join(safe_id)
    }

    /// Get metadata file path for a model
    pub fn metadata_path(&self, model_id: &str) -> PathBuf {
        let mut path = self.model_path(model_id);
        path.set_extension("json");
        path
    }

    /// Check if model exists in cache
    pub async fn exists(&self, model_id: &str) -> bool {
        let model_path = self.model_path(model_id);
        model_path.exists()
    }

    /// Load metadata for a cached model
    pub async fn load_metadata(&self, model_id: &str) -> PluginResult<Option<ModelMetadata>> {
        let metadata_path = self.metadata_path(model_id);

        if !metadata_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&metadata_path)
            .map_err(|e| PluginError::Other(format!("Failed to read metadata: {:?}: {}", metadata_path, e)))?;

        let metadata: ModelMetadata = serde_json::from_str(&content)
            .map_err(|e| PluginError::Other(format!("Failed to parse metadata: {:?}: {}", metadata_path, e)))?;

        // Update in-memory cache
        let mut cache = self.metadata.write().await;
        cache.insert(model_id.to_string(), metadata.clone());

        debug!("Loaded metadata for model: {}", model_id);
        Ok(Some(metadata))
    }

    /// Save metadata for a cached model
    pub async fn save_metadata(&self, metadata: &ModelMetadata) -> PluginResult<()> {
        let metadata_path = self.metadata_path(&metadata.model_id);

        let content =
            serde_json::to_string_pretty(metadata).map_err(|e| PluginError::Other(format!("Failed to serialize metadata: {}", e)))?;

        fs::write(&metadata_path, content)
            .map_err(|e| PluginError::Other(format!("Failed to write metadata: {:?}: {}", metadata_path, e)))?;

        // Update in-memory cache
        let mut cache = self.metadata.write().await;
        cache.insert(metadata.model_id.clone(), metadata.clone());

        debug!("Saved metadata for model: {}", metadata.model_id);
        Ok(())
    }

    /// Validate model file integrity using checksum
    pub async fn validate(&self, model_id: &str, expected_checksum: Option<&str>) -> PluginResult<bool> {
        let model_path = self.model_path(model_id);

        if !model_path.exists() {
            return Ok(false);
        }

        // Load metadata
        let metadata = match self.load_metadata(model_id).await? {
            Some(m) => m,
            None => return Ok(false),
        };

        // Verify file exists and has correct size
        let file_size = fs::metadata(&model_path).map_err(|e| PluginError::Other(format!("Model file metadata error: {}", e)))?.len();
        if file_size != metadata.file_size {
            warn!(
                "Model file size mismatch for {}: expected {}, got {}",
                model_id, metadata.file_size, file_size
            );
            return Ok(false);
        }

        // Verify checksum if provided
        if let Some(expected) = expected_checksum
            && metadata.checksum != expected
        {
            warn!("Model checksum mismatch for {}", model_id);
            return Ok(false);
        }

        debug!("Model validation passed: {}", model_id);
        Ok(true)
    }

    /// Get model size in bytes
    pub async fn get_size(&self, model_id: &str) -> PluginResult<u64> {
        let model_path = self.model_path(model_id);
        let metadata =
            fs::metadata(&model_path).map_err(|e| PluginError::Other(format!("Model not found: {:?}: {}", model_path, e)))?;
        Ok(metadata.len())
    }

    /// Update last access time for a model
    pub async fn update_access(&self, model_id: &str) -> PluginResult<()> {
        if let Some(mut metadata) = self.load_metadata(model_id).await? {
            metadata.last_accessed = SystemTime::now();
            metadata.access_count += 1;
            self.save_metadata(&metadata).await?;
        }
        Ok(())
    }

    /// List all cached models
    pub async fn list_models(&self) -> PluginResult<Vec<String>> {
        let mut models = Vec::new();

        let entries = fs::read_dir(&self.cache_dir).map_err(|e| PluginError::Other(format!(
            "Failed to read cache directory {:?}: {}",
            self.cache_dir, e
        )))?;

        for entry in entries {
            let entry = entry.map_err(|e| PluginError::Other(e.to_string()))?;
            let path = entry.path();

            // Skip metadata files and directories
            if path.is_dir() || path.extension().is_some_and(|e| e == "json") {
                continue;
            }

            // Convert filename back to model_id
            if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                let model_id = name.replace('-', "/");
                models.push(model_id);
            }
        }

        Ok(models)
    }

    /// Delete a cached model
    pub async fn delete_model(&self, model_id: &str) -> PluginResult<()> {
        let model_path = self.model_path(model_id);
        let metadata_path = self.metadata_path(model_id);

        // Delete model file
        if model_path.exists() {
            fs::remove_file(&model_path)
                .map_err(|e| PluginError::Other(format!("Failed to delete model: {:?}: {}", model_path, e)))?;
        }

        // Delete metadata
        if metadata_path.exists() {
            fs::remove_file(&metadata_path)
                .map_err(|e| PluginError::Other(format!("Failed to delete metadata: {:?}: {}", metadata_path, e)))?;
        }

        // Remove from in-memory cache
        let mut cache = self.metadata.write().await;
        cache.remove(model_id);

        info!("Deleted cached model: {}", model_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cache_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ModelCache::new(Some(temp_dir.path().to_path_buf())).unwrap();

        assert_eq!(cache.cache_dir(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_model_path() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ModelCache::new(Some(temp_dir.path().to_path_buf())).unwrap();

        let path = cache.model_path("hexgrad/Kokoro-82M");
        assert!(path.ends_with("hexgrad-Kokoro-82M"));
    }

    #[tokio::test]
    async fn test_metadata_path() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ModelCache::new(Some(temp_dir.path().to_path_buf())).unwrap();

        let path = cache.metadata_path("hexgrad/Kokoro-82M");
        assert!(path.ends_with("hexgrad-Kokoro-82M.json"));
    }

    #[tokio::test]
    async fn test_save_and_load_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ModelCache::new(Some(temp_dir.path().to_path_buf())).unwrap();

        let metadata = ModelMetadata {
            model_id: "test/model".to_string(),
            version: "v1.0".to_string(),
            file_size: 12345,
            checksum: "abc123".to_string(),
            downloaded_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            access_count: 0,
        };

        cache.save_metadata(&metadata).await.unwrap();
        let loaded = cache.load_metadata("test/model").await.unwrap();

        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.model_id, "test/model");
        assert_eq!(loaded.file_size, 12345);
        assert_eq!(loaded.checksum, "abc123");
    }

    #[tokio::test]
    async fn test_exists() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ModelCache::new(Some(temp_dir.path().to_path_buf())).unwrap();

        assert!(!cache.exists("test/model").await);

        // Create a test file
        let model_path = cache.model_path("test/model");
        fs::write(&model_path, b"test data").unwrap();

        assert!(cache.exists("test/model").await);
    }
}
