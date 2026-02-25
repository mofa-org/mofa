//! `mofa plugin install` command implementation

use super::catalog::CatalogService;
use crate::context::{CliContext, PluginSpecEntry, instantiate_plugin_from_spec};
use colored::Colorize;
use mofa_kernel::agent::plugins::PluginRegistry;

/// Map a user-supplied plugin name to its internal kind identifier.
///
/// Recognised builtin aliases are mapped to their canonical kind string;
/// unknown names are returned as-is so that future plugin kinds can be
/// installed without changes here.
fn resolve_kind(name: &str) -> &str {
    match name {
        "http-plugin" | "http" => "builtin:http",
        _ => name,
    }
}

/// Build a default `serde_json::Value` config for a given plugin kind.
fn default_config_for_kind(kind: &str) -> serde_json::Value {
    match kind {
        "builtin:http" => serde_json::json!({ "url": "https://example.com" }),
        _ => serde_json::Value::Null,
    }
}

/// Execute the `mofa plugin install` command
pub async fn run(ctx: &CliContext, name: &str) -> anyhow::Result<()> {
    // 1. Validate and normalise input
    let (name, requested_version) = parse_name_and_version(name);
    if name.is_empty() {
        anyhow::bail!("Plugin name cannot be empty");
    }

    let target_id = name.as_str();

    // 2. Check for duplicates in the in-memory registry
    if ctx.plugin_registry.contains(target_id) {
        anyhow::bail!("Plugin '{}' is already installed", target_id);
    }

    // 3. Also check the persisted store — an enabled spec that failed to load
    //    into memory should still block a duplicate install.
    if let Ok(Some(existing)) = ctx.plugin_store.get(target_id)
        && existing.enabled
    {
        anyhow::bail!(
            "Plugin '{}' is already persisted as enabled. \
             Use `mofa plugin uninstall` first if you want to reinstall.",
            target_id
        );
    }

    // 4. Try resolving via catalog cache (no network here; sync is explicit)
    let service = CatalogService::new(&ctx.data_dir);
    let resolved_from_catalog = match service.read_cache()? {
        Some(cached) => {
            service.resolve(&cached.catalog, target_id, requested_version.as_deref())?
        }
        None => None,
    };

    let (spec, reported_version) = if let Some(resolved) = resolved_from_catalog {
        println!(
            "{} Installing plugin: {} (version {})",
            "→".green(),
            resolved.id.cyan(),
            resolved.version
        );
        (
            PluginSpecEntry {
                id: resolved.id,
                kind: resolved.kind,
                enabled: true,
                config: resolved.config,
            },
            Some(resolved.version),
        )
    } else {
        println!("{} Installing plugin: {}", "→".green(), target_id.cyan());
        let kind = resolve_kind(target_id);
        (
            PluginSpecEntry {
                id: target_id.to_string(),
                kind: kind.to_string(),
                enabled: true,
                config: default_config_for_kind(kind),
            },
            None,
        )
    };

    // 5. Instantiate the plugin from the spec
    let plugin = instantiate_plugin_from_spec(&spec).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown plugin kind '{}'. Currently supported builtins: http-plugin",
            spec.kind
        )
    })?;

    // 6. Register in-memory
    ctx.plugin_registry
        .register(plugin)
        .map_err(|e| anyhow::anyhow!("Failed to register plugin '{}': {}", target_id, e))?;

    // 7. Persist to disk — rollback in-memory registration on failure
    if let Err(e) = ctx.plugin_store.save(target_id, &spec) {
        // Best-effort rollback
        let _ = ctx.plugin_registry.unregister(target_id);
        anyhow::bail!(
            "Failed to persist plugin '{}': {}. Rolled back in-memory registration.",
            target_id,
            e
        );
    }

    if let Some(version) = reported_version {
        println!(
            "{} Plugin '{}' installed successfully (version {})",
            "✓".green(),
            target_id,
            version
        );
    } else {
        println!(
            "{} Plugin '{}' installed successfully",
            "✓".green(),
            target_id
        );
    }

    Ok(())
}

fn parse_name_and_version(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim();
    if let Some((name, version)) = trimmed.split_once('@') {
        (name.trim().to_string(), Some(version.trim().to_string()))
    } else {
        (trimmed.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::plugin::catalog::{
        CachedPluginCatalog, PluginCatalog, PluginCatalogEntry, PluginCatalogRelease,
    };
    use crate::context::CliContext;
    use mofa_kernel::agent::plugins::PluginRegistry as PluginRegistryTrait;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_install_registers_and_persists_builtin_plugin() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Uninstall the default http-plugin first so we can reinstall it
        let _ = ctx.plugin_registry.unregister("http-plugin");
        let mut spec = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        spec.enabled = false;
        ctx.plugin_store.save("http-plugin", &spec).unwrap();

        let result = run(&ctx, "http-plugin").await;
        assert!(result.is_ok(), "install should succeed: {:?}", result);
        assert!(ctx.plugin_registry.contains("http-plugin"));

        let persisted = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        assert!(persisted.enabled);
        assert_eq!(persisted.kind, "builtin:http");
    }

    #[tokio::test]
    async fn test_install_rejects_duplicate_plugin() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // http-plugin is registered by default
        let result = run(&ctx, "http-plugin").await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("already installed"),
            "error should mention 'already installed'"
        );
    }

    #[tokio::test]
    async fn test_install_persists_across_context_instances() {
        let temp = TempDir::new().unwrap();

        // First context: uninstall then reinstall
        let ctx1 = CliContext::with_temp_dir(temp.path()).await.unwrap();
        let _ = ctx1.plugin_registry.unregister("http-plugin");
        let mut spec = ctx1.plugin_store.get("http-plugin").unwrap().unwrap();
        spec.enabled = false;
        ctx1.plugin_store.save("http-plugin", &spec).unwrap();

        run(&ctx1, "http-plugin").await.unwrap();
        drop(ctx1);

        // Second context: verify it's still there
        let ctx2 = CliContext::with_temp_dir(temp.path()).await.unwrap();
        assert!(ctx2.plugin_registry.contains("http-plugin"));
        let persisted = ctx2.plugin_store.get("http-plugin").unwrap().unwrap();
        assert!(persisted.enabled);
    }

    #[tokio::test]
    async fn test_install_rejects_empty_name() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("cannot be empty"),
            "error should mention 'cannot be empty'"
        );
    }

    #[tokio::test]
    async fn test_install_rejects_whitespace_only_name() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "   ").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("cannot be empty"),
            "whitespace-only name should be treated as empty"
        );
    }

    #[tokio::test]
    async fn test_install_trims_whitespace_padded_name() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Remove default http-plugin so we can reinstall with padded name
        let _ = ctx.plugin_registry.unregister("http-plugin");
        let mut spec = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        spec.enabled = false;
        ctx.plugin_store.save("http-plugin", &spec).unwrap();

        let result = run(&ctx, "  http-plugin  ").await;
        assert!(
            result.is_ok(),
            "padded name should be trimmed: {:?}",
            result
        );
        assert!(ctx.plugin_registry.contains("http-plugin"));
    }

    #[tokio::test]
    async fn test_install_supports_name_with_version_suffix() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Remove default plugin so we can reinstall it
        let _ = ctx.plugin_registry.unregister("http-plugin");
        let mut spec = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        spec.enabled = false;
        ctx.plugin_store.save("http-plugin", &spec).unwrap();

        let result = run(&ctx, "http-plugin@1.0.0").await;
        assert!(
            result.is_ok(),
            "install with version suffix should succeed: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_install_uses_catalog_when_available() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Disable default http-plugin so we can reinstall using the catalog data
        let _ = ctx.plugin_registry.unregister("http-plugin");
        let mut spec = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        spec.enabled = false;
        ctx.plugin_store.save("http-plugin", &spec).unwrap();

        // Prepare catalog cache with a custom plugin id
        let service = CatalogService::new(&ctx.data_dir);
        let cached = CachedPluginCatalog {
            fetched_at: chrono::Utc::now(),
            source: "test".to_string(),
            catalog: PluginCatalog {
                plugins: vec![PluginCatalogEntry {
                    id: "http-plugin".to_string(),
                    kind: Some("builtin:http".to_string()),
                    description: None,
                    homepage: None,
                    tags: vec!["network".to_string()],
                    default_config: Some(serde_json::json!({"url": "https://alt.example"})),
                    releases: vec![PluginCatalogRelease {
                        version: "2.0.0".to_string(),
                        source: None,
                        checksum: None,
                        yanked: false,
                        updated_at: None,
                    }],
                }],
            },
        };
        service.write_cache(&cached).unwrap();

        // Install using the catalog entry
        let result = run(&ctx, "http-plugin").await;
        assert!(result.is_ok(), "install should use catalog: {:?}", result);
        assert!(ctx.plugin_registry.contains("http-plugin"));

        let persisted = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        assert_eq!(persisted.kind, "builtin:http");
        assert_eq!(
            persisted.config.get("url").and_then(|v| v.as_str()),
            Some("https://alt.example")
        );
    }

    #[test]
    fn test_parse_name_and_version() {
        let (name, version) = parse_name_and_version("foo@1.2.3");
        assert_eq!(name, "foo");
        assert_eq!(version.as_deref(), Some("1.2.3"));

        let (name, version) = parse_name_and_version("bar");
        assert_eq!(name, "bar");
        assert!(version.is_none());
    }
}
