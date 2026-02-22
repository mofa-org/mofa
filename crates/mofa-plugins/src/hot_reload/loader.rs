//! Dynamic plugin loader
//!
//! Handles loading and unloading of dynamic plugins from shared libraries

use crate::{AgentPlugin, PluginMetadata};
use libloading::{Library, Symbol};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Plugin load error types
#[derive(Debug, thiserror::Error)]
pub enum PluginLoadError {
    #[error("Failed to load library: {0}")]
    LibraryLoad(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Plugin creation failed: {0}")]
    CreationFailed(String),

    #[error("Invalid plugin: {0}")]
    InvalidPlugin(String),

    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Plugin already loaded: {0}")]
    AlreadyLoaded(String),

    #[error("Plugin not found: {0}")]
    NotFound(String),
}

/// Required symbols for a plugin library
pub struct PluginSymbols {
    /// Create plugin function
    pub create: Symbol<'static, unsafe extern "C" fn() -> *mut dyn AgentPlugin>,
    /// Destroy plugin function
    pub destroy: Symbol<'static, unsafe extern "C" fn(*mut dyn AgentPlugin)>,
    /// Get plugin metadata function
    pub metadata: Symbol<'static, unsafe extern "C" fn() -> PluginMetadata>,
    /// Get API version function
    pub api_version: Symbol<'static, unsafe extern "C" fn() -> u32>,
}

/// Represents a loaded plugin library
pub struct PluginLibrary {
    /// Path to the library file
    path: PathBuf,
    /// The loaded library
    library: Library,
    /// File hash for change detection
    hash: String,
    /// Load timestamp
    loaded_at: std::time::Instant,
    /// Plugin metadata
    metadata: PluginMetadata,
    /// API version
    api_version: u32,
}

impl PluginLibrary {
    /// Get the library path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the file hash
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Get when the library was loaded
    pub fn loaded_at(&self) -> std::time::Instant {
        self.loaded_at
    }

    /// Get plugin metadata
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// Get API version
    pub fn api_version(&self) -> u32 {
        self.api_version
    }

    /// Create a new plugin instance from this library
    ///
    /// # Safety
    /// This function calls extern "C" functions from a dynamic library
    pub unsafe fn create_instance(&self) -> Result<Box<dyn AgentPlugin>, PluginLoadError> {
        unsafe {
            let create_fn: Symbol<unsafe extern "C" fn() -> *mut dyn AgentPlugin> = self
                .library
                .get(b"_plugin_create")
                .map_err(|e| PluginLoadError::SymbolNotFound(format!("_plugin_create: {}", e)))?;

            let raw_plugin = create_fn();
            if raw_plugin.is_null() {
                return Err(PluginLoadError::CreationFailed(
                    "Plugin creation returned null".to_string(),
                ));
            }

            Ok(Box::from_raw(raw_plugin))
        }
    }

    /// Destroy a plugin instance
    ///
    /// # Safety
    /// This function calls extern "C" functions from a dynamic library
    pub unsafe fn destroy_instance(
        &self,
        plugin: Box<dyn AgentPlugin>,
    ) -> Result<(), PluginLoadError> {
        unsafe {
            let destroy_fn: Symbol<unsafe extern "C" fn(*mut dyn AgentPlugin)> = self
                .library
                .get(b"_plugin_destroy")
                .map_err(|e| PluginLoadError::SymbolNotFound(format!("_plugin_destroy: {}", e)))?;

            let raw = Box::into_raw(plugin);
            destroy_fn(raw);
            Ok(())
        }
    }
}

impl Drop for PluginLibrary {
    fn drop(&mut self) {
        debug!("Unloading plugin library: {:?}", self.path);
    }
}

/// A dynamically loaded plugin wrapper
pub struct DynamicPlugin {
    /// The plugin instance
    plugin: Box<dyn AgentPlugin>,
    /// Reference to the library
    library_path: PathBuf,
    /// Instance ID for tracking
    instance_id: String,
    /// Creation time
    created_at: std::time::Instant,
}

impl DynamicPlugin {
    /// Create a new dynamic plugin
    pub fn new(plugin: Box<dyn AgentPlugin>, library_path: PathBuf) -> Self {
        Self {
            plugin,
            library_path,
            instance_id: uuid::Uuid::now_v7().to_string(),
            created_at: std::time::Instant::now(),
        }
    }

    /// Get the inner plugin
    pub fn plugin(&self) -> &dyn AgentPlugin {
        self.plugin.as_ref()
    }

    /// Get mutable reference to the inner plugin
    pub fn plugin_mut(&mut self) -> &mut dyn AgentPlugin {
        self.plugin.as_mut()
    }

    /// Get the library path
    pub fn library_path(&self) -> &Path {
        &self.library_path
    }

    /// Get the instance ID
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Get creation time
    pub fn created_at(&self) -> std::time::Instant {
        self.created_at
    }

    /// Consume and return the inner plugin
    pub fn into_inner(self) -> Box<dyn AgentPlugin> {
        self.plugin
    }
}

/// Plugin loader for managing dynamic plugin loading
pub struct PluginLoader {
    /// Loaded libraries
    libraries: Arc<RwLock<HashMap<PathBuf, Arc<PluginLibrary>>>>,
    /// Plugin search paths
    search_paths: Vec<PathBuf>,
    /// Expected API version
    api_version: u32,
    /// Enable unsafe loading (skip validation)
    unsafe_mode: bool,
}

impl PluginLoader {
    /// Current API version
    pub const CURRENT_API_VERSION: u32 = 1;

    /// Create a new plugin loader
    pub fn new() -> Self {
        Self {
            libraries: Arc::new(RwLock::new(HashMap::new())),
            search_paths: Vec::new(),
            api_version: Self::CURRENT_API_VERSION,
            unsafe_mode: false,
        }
    }

    /// Add a search path for plugins
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        self.search_paths.push(path.as_ref().to_path_buf());
    }

    /// Enable unsafe mode (skip validation)
    pub fn set_unsafe_mode(&mut self, enabled: bool) {
        self.unsafe_mode = enabled;
    }

    /// Calculate file hash
    fn calculate_hash(path: &Path) -> Result<String, PluginLoadError> {
        let contents = std::fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&contents);
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Find plugin file by name
    pub fn find_plugin(&self, name: &str) -> Option<PathBuf> {
        let lib_name = if cfg!(target_os = "windows") {
            format!("{}.dll", name)
        } else if cfg!(target_os = "macos") {
            format!("lib{}.dylib", name)
        } else {
            format!("lib{}.so", name)
        };

        // First check if it's an absolute path
        let direct_path = PathBuf::from(name);
        if direct_path.exists() {
            return Some(direct_path);
        }

        // Search in search paths
        for search_path in &self.search_paths {
            let full_path = search_path.join(&lib_name);
            if full_path.exists() {
                return Some(full_path);
            }
        }

        // Check current directory
        let current_path = PathBuf::from(&lib_name);
        if current_path.exists() {
            return Some(current_path);
        }

        None
    }

    /// Load a plugin library from file
    ///
    /// # Safety
    /// Loading dynamic libraries is inherently unsafe
    pub async fn load_library<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Arc<PluginLibrary>, PluginLoadError> {
        let path = path.as_ref().to_path_buf();

        // Check if already loaded
        {
            let libraries = self.libraries.read().await;
            if let Some(lib) = libraries.get(&path) {
                return Ok(lib.clone());
            }
        }

        info!("Loading plugin library: {:?}", path);

        // Calculate hash
        let hash = Self::calculate_hash(&path)?;

        // Load the library
        let library = unsafe {
            Library::new(&path).map_err(|e| PluginLoadError::LibraryLoad(e.to_string()))?
        };

        // Get API version
        let api_version = unsafe {
            let version_fn: Result<Symbol<unsafe extern "C" fn() -> u32>, _> =
                library.get(b"_plugin_api_version");

            match version_fn {
                Ok(func) => func(),
                Err(_) => 1, // Default to version 1 if not specified
            }
        };

        // Validate API version
        if !self.unsafe_mode && api_version != self.api_version {
            return Err(PluginLoadError::VersionMismatch {
                expected: self.api_version.to_string(),
                actual: api_version.to_string(),
            });
        }

        // Get metadata
        let metadata = unsafe {
            let metadata_fn: Symbol<unsafe extern "C" fn() -> PluginMetadata> = library
                .get(b"_plugin_metadata")
                .map_err(|e| PluginLoadError::SymbolNotFound(format!("_plugin_metadata: {}", e)))?;
            metadata_fn()
        };

        let plugin_lib = Arc::new(PluginLibrary {
            path: path.clone(),
            library,
            hash,
            loaded_at: std::time::Instant::now(),
            metadata,
            api_version,
        });

        // Store in cache
        {
            let mut libraries = self.libraries.write().await;
            libraries.insert(path.clone(), plugin_lib.clone());
        }

        info!(
            "Loaded plugin: {} v{}",
            plugin_lib.metadata.name, plugin_lib.metadata.version
        );

        Ok(plugin_lib)
    }

    /// Unload a plugin library
    pub async fn unload_library<P: AsRef<Path>>(&self, path: P) -> Result<(), PluginLoadError> {
        let path = path.as_ref().to_path_buf();

        let mut libraries = self.libraries.write().await;
        if libraries.remove(&path).is_some() {
            info!("Unloaded plugin library: {:?}", path);
            Ok(())
        } else {
            Err(PluginLoadError::NotFound(path.display().to_string()))
        }
    }

    /// Check if a library has changed
    pub async fn has_changed<P: AsRef<Path>>(&self, path: P) -> Result<bool, PluginLoadError> {
        let path = path.as_ref().to_path_buf();

        let libraries = self.libraries.read().await;
        if let Some(lib) = libraries.get(&path) {
            let current_hash = Self::calculate_hash(&path)?;
            Ok(current_hash != lib.hash)
        } else {
            Ok(true) // Not loaded, so consider it "changed"
        }
    }

    /// Get a loaded library
    pub async fn get_library<P: AsRef<Path>>(&self, path: P) -> Option<Arc<PluginLibrary>> {
        let libraries = self.libraries.read().await;
        libraries.get(path.as_ref()).cloned()
    }

    /// List all loaded libraries
    pub async fn list_libraries(&self) -> Vec<PathBuf> {
        let libraries = self.libraries.read().await;
        libraries.keys().cloned().collect()
    }

    /// Create a plugin instance from a loaded library
    pub async fn create_plugin<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<DynamicPlugin, PluginLoadError> {
        let path = path.as_ref().to_path_buf();

        let library = self.load_library(&path).await?;

        let plugin = unsafe { library.create_instance()? };

        Ok(DynamicPlugin::new(plugin, path))
    }

    /// Reload a plugin library
    pub async fn reload_library<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Arc<PluginLibrary>, PluginLoadError> {
        let path = path.as_ref().to_path_buf();

        // Unload existing
        let _ = self.unload_library(&path).await;

        // Small delay to ensure file handle is released
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Load fresh
        self.load_library(&path).await
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Macro to create a plugin export
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $create_fn:expr) => {
        #[no_mangle]
        pub extern "C" fn _plugin_create() -> *mut dyn $crate::plugin::AgentPlugin {
            let plugin: Box<dyn $crate::plugin::AgentPlugin> = Box::new($create_fn);
            Box::into_raw(plugin)
        }

        #[no_mangle]
        pub extern "C" fn _plugin_destroy(plugin: *mut dyn $crate::plugin::AgentPlugin) {
            if !plugin.is_null() {
                unsafe {
                    let _ = Box::from_raw(plugin);
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn _plugin_api_version() -> u32 {
            $crate::hot_reload::PluginLoader::CURRENT_API_VERSION
        }

        #[no_mangle]
        pub extern "C" fn _plugin_metadata() -> $crate::plugin::PluginMetadata {
            let plugin: $plugin_type = $create_fn;
            plugin.metadata().clone()
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_loader_new() {
        let loader = PluginLoader::new();
        assert_eq!(loader.api_version, PluginLoader::CURRENT_API_VERSION);
        assert!(!loader.unsafe_mode);
    }

    #[tokio::test]
    async fn test_search_paths() {
        let mut loader = PluginLoader::new();
        loader.add_search_path("/usr/lib/plugins");
        loader.add_search_path("/opt/plugins");
        assert_eq!(loader.search_paths.len(), 2);
    }

    #[test]
    fn test_calculate_hash() {
        // Create a temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, b"test content").unwrap();

        let hash1 = PluginLoader::calculate_hash(&file_path).unwrap();
        let hash2 = PluginLoader::calculate_hash(&file_path).unwrap();

        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());
    }
}
