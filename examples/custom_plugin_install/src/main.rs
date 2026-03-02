//! Demonstrates packaging and installing a custom plugin
//!
//! This example shows:
//! 1. How to structure a plugin for distribution
//! 2. How to package it (directory or archive)
//! 3. How to install via `mofa plugin install`

use tempfile::TempDir;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Custom Plugin Install Demo\n");
    println!("This example shows how to package and install custom plugins.\n");

    let temp = TempDir::new()?;
    let plugin_name = "http-helper-plugin";
    let plugin_dir = temp.path().join(plugin_name);
    fs::create_dir_all(&plugin_dir).await?;

    println!("1. Creating plugin package structure...");

    // Plugin manifest
    let manifest = r#"[plugin]
name = "http-helper-plugin"
version = "1.0.0"
description = "Helper plugin for HTTP operations"
author = "Your Name"

[capabilities]
- http_get
- http_post
- json_parsing
"#;
    fs::write(plugin_dir.join("plugin.toml"), manifest).await?;

    // Create src directory first
    fs::create_dir_all(plugin_dir.join("src")).await?;

    // Main plugin code
    let main_code = r#"//! HTTP Helper Plugin
use serde_json::Value;

pub struct HttpHelper;

impl HttpHelper {
    pub async fn get(url: &str) -> Result<Value, String> {
        // Implementation here
        Ok(serde_json::json!({"status": "ok"}))
    }

    pub async fn post(url: &str, data: Value) -> Result<Value, String> {
        // Implementation here
        Ok(serde_json::json!({"status": "ok"}))
    }
}
"#;
    fs::write(plugin_dir.join("src").join("lib.rs"), main_code).await?;

    // Cargo.toml for the plugin
    let cargo_toml = r#"[package]
name = "http-helper-plugin"
version = "1.0.0"
edition = "2021"

[dependencies]
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
"#;
    fs::write(plugin_dir.join("Cargo.toml"), cargo_toml).await?;

    // README
    let readme = r#"# HTTP Helper Plugin

A plugin that provides HTTP GET and POST operations.

## Installation

```bash
# From local directory
mofa plugin install /path/to/http-helper-plugin

# From URL (if packaged as tar.gz)
mofa plugin install https://example.com/plugins/http-helper-plugin.tar.gz
```

## Usage

Once installed, the plugin provides HTTP helper functions.
"#;
    fs::write(plugin_dir.join("README.md"), readme).await?;

    println!("   Plugin package created at: {}", plugin_dir.display());
    println!("\n2. Plugin structure:");
    println!("   - plugin.toml (plugin manifest)");
    println!("   - src/lib.rs (plugin implementation)");
    println!("   - Cargo.toml (Rust dependencies)");
    println!("   - README.md (documentation)");

    println!("\n3. Installation options:");
    println!("\n   Option A: Install from local directory");
    println!("   mofa plugin install {}", plugin_dir.display());

    println!("\n   Option B: Package as tar.gz and install from URL");
    println!("   tar -czf {}.tar.gz {}", plugin_name, plugin_dir.display());
    println!("   mofa plugin install https://example.com/{}.tar.gz", plugin_name);

    println!("\n4. After installation:");
    println!("   - Plugin will be in ~/.mofa/plugins/{}", plugin_name);
    println!("   - Plugin spec will be saved to plugin store");
    println!("   - Use 'mofa plugin list' to verify");

    println!("\nPlugin package ready!");
    println!("\nNext steps:");
    println!("   1. Test the plugin locally");
    println!("   2. Package it (tar.gz or zip)");
    println!("   3. Host it somewhere (GitHub releases, S3, etc.)");
    println!("   4. Share the install URL with users");

    println!("\nPlugin directory: {}", plugin_dir.display());
    println!("   Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    Ok(())
}
