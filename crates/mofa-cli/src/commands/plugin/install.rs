//! `mofa plugin install` command implementation

use crate::context::{CliContext, PluginSpecEntry, instantiate_plugin_from_spec};
use crate::plugin_catalog::{DEFAULT_PLUGIN_REPO_ID, find_catalog_entry};
use colored::Colorize;
use mofa_kernel::agent::plugins::PluginRegistry;

/// Execute the `mofa plugin install` command
pub async fn run(ctx: &CliContext, name: &str) -> anyhow::Result<()> {
    let normalized = name.trim();
    if normalized.is_empty() {
        anyhow::bail!("Plugin name cannot be empty");
    }

    let (repo_id, plugin_id) = parse_plugin_reference(normalized)?;
    let entry = find_catalog_entry(&repo_id, &plugin_id).ok_or_else(|| {
        anyhow::anyhow!("Plugin '{}' not found in repository '{}'", plugin_id, repo_id)
    })?;

    if ctx.plugin_registry.contains(&entry.id) {
        anyhow::bail!("Plugin '{}' is already installed", entry.id);
    }

    if let Ok(Some(existing)) = ctx.plugin_store.get(&entry.id)
        && existing.enabled
    {
        anyhow::bail!(
            "Plugin '{}' is already persisted as enabled. Use `mofa plugin uninstall` first if you want to reinstall.",
            entry.id
        );
    }

    println!("{} Installing plugin: {}", "→".green(), entry.id.cyan());

    let spec = PluginSpecEntry {
        id: entry.id.clone(),
        kind: entry.kind.clone(),
        enabled: true,
        config: entry.config.clone(),
        description: Some(entry.description.clone()),
        repo_id: Some(entry.repo_id.clone()),
    };

    let plugin = instantiate_plugin_from_spec(&spec).ok_or_else(|| {
        anyhow::anyhow!(
            "CLI installer does not support plugin kind '{}'",
            spec.kind
        )
    })?;

    ctx.plugin_registry
        .register(plugin)
        .map_err(|e| anyhow::anyhow!("Failed to register plugin '{}': {}", entry.id, e))?;

    if let Err(e) = ctx.plugin_store.save(&spec.id, &spec) {
        let _ = ctx.plugin_registry.unregister(&spec.id);
        anyhow::bail!(
            "Failed to persist plugin '{}': {}. Rolled back in-memory registration.",
            spec.id,
            e
        );
    }

    println!(
        "{} Installed plugin '{}' from repository '{}'",
        "✓".green(),
        spec.id,
        repo_id
    );
    Ok(())
}

fn parse_plugin_reference(value: &str) -> anyhow::Result<(String, String)> {
    if let Some((repo, plugin)) = value.split_once('/') {
        let repo = repo.trim();
        let plugin = plugin.trim();

        if repo.is_empty() || plugin.is_empty() {
            anyhow::bail!("Plugin reference must be '<repo>/<plugin>'");
        }

        Ok((repo.to_string(), plugin.to_string()))
    } else {
        Ok((DEFAULT_PLUGIN_REPO_ID.to_string(), value.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use crate::plugin_catalog::DEFAULT_PLUGIN_REPO_ID;
    use mofa_kernel::agent::plugins::PluginRegistry as PluginRegistryTrait;
    use tempfile::TempDir;

    fn disable_default_http_plugin(ctx: &CliContext) {
        let _ = ctx.plugin_registry.unregister("http-plugin");
        if let Ok(Some(mut spec)) = ctx.plugin_store.get("http-plugin") {
            spec.enabled = false;
            ctx.plugin_store.save("http-plugin", &spec).unwrap();
        }
    }

    #[tokio::test]
    async fn test_install_registers_and_persists_builtin_plugin() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        disable_default_http_plugin(&ctx);

        run(&ctx, "http-plugin").await.unwrap();

        assert!(PluginRegistryTrait::contains(
            ctx.plugin_registry.as_ref(),
            "http-plugin"
        ));

        let spec = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        assert!(spec.enabled);
        assert_eq!(spec.repo_id.as_deref(), Some(DEFAULT_PLUGIN_REPO_ID));
    }

    #[tokio::test]
    async fn test_install_rejects_duplicate_plugin() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let err = run(&ctx, "http-plugin").await.unwrap_err();
        assert!(err.to_string().contains("already installed"));
    }

    #[tokio::test]
    async fn test_install_rejects_empty_name() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let err = run(&ctx, "   ").await.unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_install_supports_repo_prefixed_reference() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        disable_default_http_plugin(&ctx);

        run(&ctx, "official/http-plugin").await.unwrap();

        let spec = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        assert_eq!(spec.repo_id.as_deref(), Some(DEFAULT_PLUGIN_REPO_ID));
        assert!(spec.enabled);
    }

    #[tokio::test]
    async fn test_install_rejects_unknown_catalog_entry() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let err = run(&ctx, "official/not-real").await.unwrap_err();
        assert!(err.to_string().contains("not found in repository"));
    }
}
