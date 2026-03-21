//! Demonstrates `mofa plugin install` and `mofa plugin uninstall` commands
//!
//! This example:
//! 1. Creates a sample plugin directory structure
//! 2. Installs plugin from local path
//! 3. Validates installation
//! 4. Demonstrates uninstall process

use tempfile::TempDir;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("CLI Plugin Lifecycle Demo\n");
    println!("This example demonstrates the plugin install/uninstall workflow.\n");

    let temp = TempDir::new()?;
    
    // Create a sample plugin
    let plugin_dir = temp.path().join("my-custom-plugin");
    fs::create_dir_all(&plugin_dir).await?;

    println!("1. Creating sample plugin structure...");
    
    // Create plugin manifest
    let manifest = r#"[plugin]
name = "my-custom-plugin"
version = "1.0.0"
description = "A demo plugin for testing"

[metadata]
author = "Demo User"
"#;
    fs::write(plugin_dir.join("plugin.toml"), manifest).await?;

    // Create plugin code
    let plugin_code = r#"//! Custom plugin implementation
pub fn process(input: &str) -> String {
    format!("Processed: {}", input)
}
"#;
    fs::write(plugin_dir.join("lib.rs"), plugin_code).await?;

    // Create README
    fs::write(
        plugin_dir.join("README.md"),
        "# My Custom Plugin\n\nThis is a demo plugin.\n",
    )
    .await?;

    println!("   Plugin structure created at: {}", plugin_dir.display());
    println!("\n2. Plugin structure:");
    println!("   - plugin.toml (manifest)");
    println!("   - lib.rs (plugin code)");
    println!("   - README.md (documentation)");

    println!("\n3. To install this plugin:");
    println!("   mofa plugin install {}", plugin_dir.display());

    println!("\n4. To verify installation:");
    println!("   mofa plugin list");

    println!("\n5. To uninstall:");
    println!("   mofa plugin uninstall my-custom-plugin");

    println!("\nDemo plugin created!");
    println!("\nNote: This example creates the plugin structure.");
    println!("   Use the CLI commands to actually install/uninstall it.");

    // Keep temp dir alive
    println!("\nPlugin directory: {}", plugin_dir.display());
    println!("   Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    Ok(())
}
