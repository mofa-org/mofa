// mofa-plugin-loader/src/plugin_loader.rs
//
// Runtime hot-reload for MofaApp plugins.
// Enabled only when MOFA_HOT_RELOAD=1 is set (off by default).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use libloading::{Library, Symbol};

/// Metadata exposed by every plugin `.so` / `.dylib`.
#[derive(Clone, Debug)]
pub struct PluginInfo {
    pub id: String,
    pub version: String,
}

/// A loaded plugin.  Keeps the `Library` alive so symbols stay valid.
pub struct PluginHandle {
    _lib: Library,
    pub info: PluginInfo,
    /// C-ABI constructor: `extern "C" fn() -> *mut dyn MofaApp`
    ///
    /// Callers cast the raw pointer to the concrete app type they expect.
    pub create_raw: unsafe extern "C" fn() -> *mut (),
}

impl std::fmt::Debug for PluginHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHandle")
            .field("info", &self.info)
            .finish()
    }
}

/// Errors that can arise while loading a plugin.
#[derive(Debug, thiserror::Error)]
pub enum PluginLoadError {
    #[error("libloading error: {0}")]
    Lib(#[from] libloading::Error),

    #[error("manifest read error for {path}: {source}")]
    Manifest {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("manifest parse error for {path}: {source}")]
    ManifestParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("missing symbol `{symbol}` in {path}")]
    MissingSymbol { symbol: String, path: PathBuf },
}

// ── plugin.toml schema ────────────────────────────────────────────────────────

#[derive(serde::Deserialize, Debug)]
struct PluginManifest {
    plugin: PluginSection,
}

#[derive(serde::Deserialize, Debug)]
struct PluginSection {
    id: String,
    version: String,
    entry: String, // e.g. "libmofa_fm.so" or "libmofa_fm.dylib"
}

// ── PluginHandle ──────────────────────────────────────────────────────────────

impl PluginHandle {
    /// Load a plugin from its directory.
    ///
    /// Expects `plugin_dir/plugin.toml` and `plugin_dir/<entry>`.
    ///
    /// # Safety
    /// Loading a shared library is inherently unsafe.  The plugin ABI must
    /// match (C ABI, stable symbol names).
    pub fn load(plugin_dir: &Path) -> Result<Self, PluginLoadError> {
        // 1. Parse manifest
        let manifest_path = plugin_dir.join("plugin.toml");
        let raw = std::fs::read_to_string(&manifest_path).map_err(|e| {
            PluginLoadError::Manifest {
                path: manifest_path.clone(),
                source: e,
            }
        })?;
        let manifest: PluginManifest =
            toml::from_str(&raw).map_err(|e| PluginLoadError::ManifestParse {
                path: manifest_path,
                source: e,
            })?;

        let lib_path = plugin_dir.join(&manifest.plugin.entry);

        // 2. Load the shared library
        // SAFETY: plugin ABI contract must be upheld by the caller.
        let lib = unsafe { Library::new(&lib_path)? };

        // 3. Resolve the mandatory constructor symbol
        // SAFETY: same constraint as above.
        let create_raw: Symbol<unsafe extern "C" fn() -> *mut ()> = unsafe {
            lib.get(b"mofa_app_create\0").map_err(|e| {
                let _ = e; // rethrow with context
                PluginLoadError::MissingSymbol {
                    symbol: "mofa_app_create".into(),
                    path: lib_path.clone(),
                }
            })?
        };

        // Extend the lifetime to 'static — safe because we keep `_lib` alive.
        // SAFETY: `lib` is stored in the same struct, so symbols are valid as
        //         long as `PluginHandle` is alive.
        let create_raw: unsafe extern "C" fn() -> *mut () =
            unsafe { std::mem::transmute(create_raw.into_raw()) };

        Ok(PluginHandle {
            _lib: lib,
            info: PluginInfo {
                id: manifest.plugin.id,
                version: manifest.plugin.version,
            },
            create_raw,
        })
    }
}

// ── Plugin registry ───────────────────────────────────────────────────────────

/// Thread-safe map of `plugin_id → PluginHandle`.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    inner: Arc<Mutex<HashMap<String, PluginHandle>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a plugin.
    pub fn register(&self, handle: PluginHandle) {
        let id = handle.info.id.clone();
        let mut guard = self.inner.lock().expect("registry lock poisoned");
        if let Some(old) = guard.insert(id.clone(), handle) {
            tracing::info!(
                plugin_id = %id,
                old_version = %old.info.version,
                "unregistered old plugin version"
            );
        }
    }

    /// Remove a plugin by id, returning the old handle (which unloads the lib
    /// when dropped).
    pub fn unregister(&self, id: &str) -> Option<PluginHandle> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .remove(id)
    }

    pub fn get_ids(&self) -> Vec<String> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .keys()
            .cloned()
            .collect()
    }
}
