//! `mofa plugin install` command implementation

use crate::context::{CliContext, PluginSpecEntry};
use anyhow::{Context, Result};
use colored::Colorize;
use futures::StreamExt;
use mofa_kernel::agent::plugins::PluginRegistry;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Execute the `mofa plugin install` command
pub async fn run(
    ctx: &CliContext,
    name: &str,
    checksum: Option<&str>,
    verify_signature: bool,
) -> Result<()> {
    println!("{} Installing plugin: {}", "→".green(), name.cyan());

    // Determine plugin source type
    let plugin_source = determine_plugin_source(name)?;

    // Derive a stable plugin identifier used for storage.
    // For local paths we normalize the path into a filesystem-safe ID so
    // `/tmp/foo/bar` becomes `_tmp_foo_bar`, matching the expectations in tests
    // and keeping the actual plugins directory layout sane.
    use std::path::Path as StdPath;
    let mut plugin_id = name.to_string();
    if let PluginSource::LocalPath(path) = &plugin_source {
        // If the user passed an absolute or relative path, normalize it.
        if StdPath::new(name) == path {
            plugin_id = name.replace(['/', '\\'], "_");
        }
    }

    // Check if plugin is already installed (based on persisted specs)
    if ctx.plugin_store.get(&plugin_id)?.is_some() {
        anyhow::bail!("Plugin '{}' is already installed", plugin_id);
    }

    // Handle installation based on source type
    let plugin_dir = match plugin_source {
        PluginSource::LocalPath(path) => {
            println!("  {} Source: Local path", "•".bright_black());
            // Copy plugin files into the managed plugins directory; validation happens afterward
            install_from_local_path(&ctx.data_dir, &plugin_id, &path).await?
        }
        PluginSource::Url(url) => {
            println!("  {} Source: URL", "•".bright_black());
            install_from_url(&ctx.data_dir, &plugin_id, &url, checksum, verify_signature).await?
        }
        PluginSource::Registry(registry_name) => {
            println!("  {} Source: Registry", "•".bright_black());
            // For now, treat registry names as potential URLs or fail gracefully
            anyhow::bail!(
                "Plugin registry support not yet implemented. \
                 Please provide a local path or URL instead."
            );
        }
    };

    // Validate plugin structure in its final location
    validate_plugin_structure(&plugin_dir)?;

    // Create plugin spec entry
    let spec = PluginSpecEntry {
        id: plugin_id.clone(),
        kind: "external".to_string(),
        enabled: true,
        config: serde_json::json!({
            "path": plugin_dir.to_string_lossy(),
            "installed_at": chrono::Utc::now().to_rfc3339(),
        }),
    };

    // Save to plugin store
    ctx.plugin_store
        .save(&plugin_id, &spec)
        .with_context(|| format!("Failed to persist plugin spec for '{}'", plugin_id))?;

    println!(
        "{} Plugin '{}' installed successfully",
        "✓".green(),
        plugin_id
    );
    println!(
        "  {} Location: {}",
        "•".bright_black(),
        plugin_dir.display().to_string().cyan()
    );
    println!(
        "  {} Use {} to activate it",
        "•".bright_black(),
        "mofa plugin enable".yellow()
    );

    Ok(())
}

/// Determine the source type of a plugin
fn determine_plugin_source(name: &str) -> Result<PluginSource> {
    // Check if it's a URL
    if name.starts_with("http://") || name.starts_with("https://") {
        return Ok(PluginSource::Url(name.to_string()));
    }

    // Check if it's a local path
    let path = Path::new(name);
    if path.exists() {
        return Ok(PluginSource::LocalPath(path.to_path_buf()));
    }

    // Otherwise treat as registry name
    Ok(PluginSource::Registry(name.to_string()))
}

/// Install plugin from a local path
async fn install_from_local_path(
    data_dir: &Path,
    plugin_name: &str,
    source_path: &Path,
) -> Result<PathBuf> {
    let plugins_dir = data_dir.join("plugins");
    tokio::fs::create_dir_all(&plugins_dir)
        .await
        .with_context(|| "Failed to create plugins directory")?;

    let dest_dir = plugins_dir.join(plugin_name);

    // Remove existing directory if present
    if dest_dir.exists() {
        tokio::fs::remove_dir_all(&dest_dir)
            .await
            .with_context(|| {
                format!(
                    "Failed to remove existing plugin directory: {}",
                    dest_dir.display()
                )
            })?;
    }

    // Copy plugin files
    copy_dir_recursive(source_path, &dest_dir).await?;

    Ok(dest_dir)
}

/// Install plugin from a URL
async fn install_from_url(
    data_dir: &Path,
    plugin_name: &str,
    url: &str,
    expected_checksum: Option<&str>,
    verify_signature: bool,
) -> Result<PathBuf> {
    let plugins_dir = data_dir.join("plugins");
    tokio::fs::create_dir_all(&plugins_dir)
        .await
        .with_context(|| "Failed to create plugins directory")?;

    // Download the file with progress bar
    println!("  {} Downloading from {}", "•".bright_black(), url.cyan());

    let response = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to download plugin from {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    // Get content length for progress bar
    let total_size = response.content_length();
    let pb = indicatif::ProgressBar::new(total_size.unwrap_or(0));
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Download with progress tracking
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| "Failed to read download chunk")?;
        bytes.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }
    pb.finish_with_message("Downloaded");

    // Verify checksum if provided
    if let Some(expected) = expected_checksum {
        println!("  {} Verifying checksum...", "•".bright_black());
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let computed = hasher.finalize();
        let computed_hex = hex::encode(computed);

        if computed_hex.to_lowercase() != expected.to_lowercase() {
            anyhow::bail!(
                "Checksum mismatch!\n  Expected: {}\n  Computed: {}\n\nPlugin may be corrupted or tampered with.",
                expected,
                computed_hex
            );
        }
        println!("  {} Checksum verified", "✓".green());
    }

    if verify_signature {
        println!(
            "  {} Signature verification not yet implemented",
            "⚠".yellow()
        );
        println!("  {} Consider using --checksum for now", "•".bright_black());
    }

    // Determine if it's an archive or single file
    let dest_dir = plugins_dir.join(plugin_name);
    tokio::fs::create_dir_all(&dest_dir)
        .await
        .with_context(|| "Failed to create plugin directory")?;

    // For simplicity, assume it's a tar.gz or zip based on URL
    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        extract_tar_gz(&bytes, &dest_dir)?;
    } else if url.ends_with(".zip") {
        extract_zip(&bytes, &dest_dir)?;
    } else {
        // Treat as single file, save it directly
        let filename = url.split('/').next_back().unwrap_or("plugin");
        let file_path = dest_dir.join(filename);
        tokio::fs::write(&file_path, &bytes)
            .await
            .with_context(|| "Failed to write plugin file")?;
    }

    Ok(dest_dir)
}

/// Validate that the plugin directory has required structure
fn validate_plugin_structure(plugin_dir: &Path) -> Result<()> {
    if !plugin_dir.exists() {
        anyhow::bail!("Plugin directory does not exist: {}", plugin_dir.display());
    }

    if !plugin_dir.is_dir() {
        anyhow::bail!("Plugin path is not a directory: {}", plugin_dir.display());
    }

    // Check for at least one file (skip . and .. entries)
    let mut has_files = false;
    let mut entry_count = 0;
    let entries = std::fs::read_dir(plugin_dir)
        .with_context(|| format!("Failed to read plugin directory: {}", plugin_dir.display()))?;
    for entry in entries {
        entry_count += 1;
        match entry {
            Ok(entry) => {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip hidden files and directories starting with .
                if !name_str.starts_with('.') {
                    has_files = true;
                    break;
                }
            }
            Err(_) => {
                // Continue on error - some entries might fail
                continue;
            }
        }
    }

    if !has_files {
        anyhow::bail!(
            "Plugin directory is empty or contains only hidden files (found {} entries): {}",
            entry_count,
            plugin_dir.display()
        );
    }

    // Plugin validation passed
    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive<'a>(
    src: &'a Path,
    dest: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(dest)
            .await
            .with_context(|| format!("Failed to create directory: {}", dest.display()))?;

        let mut entries = tokio::fs::read_dir(src)
            .await
            .with_context(|| format!("Failed to read source directory: {}", src.display()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .with_context(|| "Failed to read directory entry")?
        {
            let entry_path = entry.path();
            let dest_path = dest.join(entry.file_name());

            if entry_path.is_dir() {
                copy_dir_recursive(&entry_path, &dest_path).await?;
            } else {
                tokio::fs::copy(&entry_path, &dest_path)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to copy file from {} to {}",
                            entry_path.display(),
                            dest_path.display()
                        )
                    })?;
            }
        }

        Ok(())
    })
}

/// Extract a tar.gz archive
fn extract_tar_gz(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let gz = GzDecoder::new(bytes);
    let mut archive = Archive::new(gz);

    archive
        .unpack(dest_dir)
        .with_context(|| "Failed to extract tar.gz archive")?;

    Ok(())
}

/// Extract a zip archive
fn extract_zip(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).with_context(|| "Failed to read zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("Failed to read zip entry {}", i))?;

        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)
                .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }
            let mut outfile = std::fs::File::create(&outpath)
                .with_context(|| format!("Failed to create file: {}", outpath.display()))?;
            std::io::copy(&mut file, &mut outfile)
                .with_context(|| "Failed to write file contents")?;
        }
    }

    Ok(())
}

/// Plugin source types
enum PluginSource {
    LocalPath(PathBuf),
    Url(String),
    Registry(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_determine_plugin_source_url() {
        let source = determine_plugin_source("https://example.com/plugin.tar.gz").unwrap();
        assert!(matches!(source, PluginSource::Url(_)));
    }

    #[test]
    fn test_determine_plugin_source_local() {
        let temp_dir = TempDir::new().unwrap();
        let source = determine_plugin_source(temp_dir.path().to_str().unwrap()).unwrap();
        assert!(matches!(source, PluginSource::LocalPath(_)));
    }

    #[test]
    fn test_determine_plugin_source_registry() {
        let source = determine_plugin_source("my-plugin").unwrap();
        assert!(matches!(source, PluginSource::Registry(_)));
    }

    #[tokio::test]
    async fn test_validate_plugin_structure() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("test-plugin");
        tokio::fs::create_dir(&plugin_dir).await.unwrap();

        // Empty directory should fail
        assert!(validate_plugin_structure(&plugin_dir).is_err());

        // Directory with a file should pass
        tokio::fs::write(plugin_dir.join("plugin.rs"), b"// test")
            .await
            .unwrap();
        assert!(validate_plugin_structure(&plugin_dir).is_ok());
    }

    #[tokio::test]
    async fn test_install_from_local_path() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Create a source plugin directory
        let source_plugin = temp.path().join("source-plugin");
        tokio::fs::create_dir_all(&source_plugin).await.unwrap();
        tokio::fs::write(
            source_plugin.join("plugin.toml"),
            b"[plugin]\nname = \"test\"\n",
        )
        .await
        .unwrap();
        tokio::fs::write(source_plugin.join("main.rs"), b"fn main() {}\n")
            .await
            .unwrap();

        // Ensure files are synced to disk
        tokio::fs::metadata(&source_plugin).await.unwrap();

        // Install the plugin using the path as the name parameter
        let plugin_path_str = source_plugin.to_str().unwrap();
        let result = run(&ctx, plugin_path_str, None, false).await;
        if let Err(e) = &result {
            eprintln!("Installation failed: {}", e);
        }
        assert!(result.is_ok(), "Plugin installation should succeed");

        // Verify plugin was installed (plugin name is the full path, sanitized)
        let plugin_name = plugin_path_str.replace('/', "_").replace('\\', "_");
        let plugin_dir = ctx.data_dir.join("plugins").join(&plugin_name);
        assert!(
            plugin_dir.exists(),
            "Plugin dir should exist at: {}",
            plugin_dir.display()
        );
        assert!(plugin_dir.join("plugin.toml").exists());
        assert!(plugin_dir.join("main.rs").exists());

        // Verify plugin spec was saved
        let spec = ctx.plugin_store.get(&plugin_name).unwrap();
        assert!(spec.is_some());
        assert!(spec.unwrap().enabled);
    }

    #[tokio::test]
    async fn test_install_already_exists() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Create a plugin directory and install it first
        let source_plugin = temp.path().join("test-plugin");
        tokio::fs::create_dir_all(&source_plugin).await.unwrap();
        tokio::fs::write(source_plugin.join("file.txt"), b"test")
            .await
            .unwrap();

        // First installation should succeed
        let result1 = run(&ctx, source_plugin.to_str().unwrap(), None, false).await;
        assert!(result1.is_ok());

        // Try to install again - should fail because plugin already exists
        let result2 = run(&ctx, source_plugin.to_str().unwrap(), None, false).await;
        assert!(result2.is_err());
        assert!(
            result2
                .unwrap_err()
                .to_string()
                .contains("already installed")
        );
    }

    #[tokio::test]
    async fn test_install_invalid_plugin_structure() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Create an empty source directory (invalid)
        let source_plugin = temp.path().join("empty-plugin");
        tokio::fs::create_dir_all(&source_plugin).await.unwrap();

        // Try to install - should fail validation
        let result = run(&ctx, source_plugin.to_str().unwrap(), None, false).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("empty") || err_msg.contains("Plugin directory"));
    }
}
