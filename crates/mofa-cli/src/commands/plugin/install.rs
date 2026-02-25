//! `mofa plugin install` command implementation

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
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("Plugin name cannot be empty");
    }

    // 2. Check for duplicates in the in-memory registry
    if ctx.plugin_registry.contains(name) {
        anyhow::bail!("Plugin '{}' is already installed", name);
    }

    // 3. Also check the persisted store — an enabled spec that failed to load
    //    into memory should still block a duplicate install.
    if let Ok(Some(existing)) = ctx.plugin_store.get(name)
        && existing.enabled
    {
        anyhow::bail!(
            "Plugin '{}' is already persisted as enabled. \
             Use `mofa plugin uninstall` first if you want to reinstall.",
            name
        );
    }

    println!("{} Installing plugin: {}", "→".green(), name.cyan());

    // 4. Build the spec entry
    let kind = resolve_kind(name);
    let spec = PluginSpecEntry {
        id: name.to_string(),
        kind: kind.to_string(),
        enabled: true,
        config: default_config_for_kind(kind),
    };

    // 5. Instantiate the plugin from the spec
    let plugin = instantiate_plugin_from_spec(&spec).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown plugin kind '{}'. Currently supported builtins: http-plugin",
            kind
        )
    })?;

    // 6. Register in-memory
    ctx.plugin_registry
        .register(plugin)
        .map_err(|e| anyhow::anyhow!("Failed to register plugin '{}': {}", name, e))?;

    // 7. Persist to disk — rollback in-memory registration on failure
    if let Err(e) = ctx.plugin_store.save(name, &spec) {
        // Best-effort rollback
        let _ = ctx.plugin_registry.unregister(name);
        anyhow::bail!(
            "Failed to persist plugin '{}': {}. Rolled back in-memory registration.",
            name,
            e
        );
    }

    println!("{} Plugin '{}' installed successfully", "✓".green(), name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
