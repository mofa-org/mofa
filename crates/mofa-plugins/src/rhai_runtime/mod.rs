//! Rhai Runtime Plugin
//!
//! Provides Rhai script-based runtime plugin support for MoFA agent system
//! - Load and execute Rhai scripts as plugins
//! - Full lifecycle management (load, init, start, stop, unload)
//! - Plugin metadata and configuration
//! - Safe execution environment
//! - Hot reload support
//!
//! A Rhai plugin is a script that defines:
//! ```rhai
//! // Plugin metadata (optional)
//! plugin_name = "my_rhai_plugin";
//! plugin_version = "1.0.0";
//! plugin_description = "A sample Rhai plugin";
//! plugin_author = "Example Author";
//!
//! // Lifecycle functions
//! fn init() {
//!     // Initialization logic
//!     print("Plugin initialized");
//! }
//!
//! fn start() {
//!     // Start logic
//!     print("Plugin started");
//! }
//!
//! fn stop() {
//!     // Stop logic
//!     print("Plugin stopped");
//! }
//!
//! fn unload() {
//!     // Unload logic
//!     print("Plugin unloaded");
//! }
//!
//! // Execution function
//! fn execute(input) {
//!     // Handle execution requests
//!     "Rhai plugin executed: " + input
//! }
//!
//! // Event handler example
//! fn on_event(event) {
//!     print("Event received: " + event.name);
//! }
//! ```

mod plugin;
mod types;

pub use plugin::{RhaiPlugin, RhaiPluginConfig, RhaiPluginState};
pub use types::{PluginMetadata, RhaiPluginError, RhaiPluginResult};
