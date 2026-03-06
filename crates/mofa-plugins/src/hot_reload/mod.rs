//! Hot-reload plugin support module
//!
//! Provides hot-reload capabilities for plugins:
//! - File watching for plugin changes
//! - Dynamic plugin loading/unloading
//! - State preservation during reload
//! - Plugin versioning and rollback
//! - Graceful plugin replacement

mod loader;
mod manager;
mod registry;
mod state;
mod watcher;

pub use loader::{
    DynamicPlugin, IntoPluginLoadReport, PluginLibrary, PluginLoadError, PluginLoadReport,
    PluginLoadResult, PluginLoader, PluginSymbols,
};
pub use manager::{HotReloadConfig, HotReloadManager, IntoReloadReport, ReloadError, ReloadReport, ReloadResult};
pub use registry::{PluginInfo, PluginRegistry, PluginVersion};
pub use state::{PluginState as HotReloadPluginState, StateManager, StateSnapshot};
pub use watcher::{PluginWatcher, WatchConfig, WatchEvent, WatchEventKind};

// Re-export kernel hot reload definitions except HotReloadConfig
pub use mofa_kernel::plugin::{HotReloadable, ReloadEvent, ReloadStrategy};
