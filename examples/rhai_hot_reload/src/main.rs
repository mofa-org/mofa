//! Rhai Plugin Hot Reload Example
//!
//! This example demonstrates:
//! 1. Loading a Rhai plugin from an external file
//! 2. Executing the plugin
//! 3. Reloading the plugin when the file changes
//! 4. Executing the updated plugin to see changes

use mofa_sdk::plugins::{AgentPlugin, PluginContext, RhaiPlugin};
use std::path::PathBuf;
use tokio::time;
use tracing::{info, warn, Level};
// ============================================================================
// Main Function
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== Rhai Plugin Hot Reload Example ===\n");

    // Path to our sample plugin
    let plugin_path = PathBuf::from("sample_plugin.rhai");
    if !plugin_path.exists() {
        warn!("⚠️  Please run this example from the examples/rhai_hot_reload directory!");
        warn!("Directory: {}", std::env::current_dir()?.display());
        return Ok(());
    }

    // Create plugin from file
    let mut plugin = RhaiPlugin::from_file("my_hot_plugin", &plugin_path).await?;

    // Initialize plugin
    let ctx = PluginContext::new("test_agent");
    plugin.load(&ctx).await?;
    plugin.init_plugin().await?;

    // Example input
    let input = "Test message from agent";

    // Execute plugin multiple times to show reload behavior
    for i in 0..10 {
        // Check if file has changed and reload if needed
        match check_and_reload(&mut plugin, &plugin_path).await {
            Ok(true) => info!("🔄 Plugin reloaded!"),
            Ok(false) => info!("✅ Plugin unchanged"),
            Err(e) => warn!("⚠️  Error checking/reloading: {}", e),
        }

        // Execute the plugin
        let result = plugin.execute(input.to_string()).await?;
        info!("Execution result {}: {}", i + 1, result);

        // Wait for 2 seconds
        time::sleep(time::Duration::from_secs(2)).await;
    }

    // Cleanup
    plugin.unload().await?;

    info!("\n=== Example Complete ===");
    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn check_and_reload(plugin: &mut RhaiPlugin, path: &PathBuf) -> Result<bool, Box<dyn std::error::Error>> {
    // Get current file modification time
    let current_mod = std::fs::metadata(path)?.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_secs();

    // Check if plugin needs reload (simple implementation)
    // TODO: Use the proper hot_reload module when fully integrated
    if plugin.last_modified() != current_mod {
        plugin.reload().await?;
        Ok(true)
    } else {
        Ok(false)
    }
}
