//! `mofa plugin repository` subcommands

use crate::CliError;
use crate::context::CliContext;
use crate::output::Table;
use crate::plugin_catalog::{self, PluginRepoEntry};
use chrono::Utc;
use colored::Colorize;

/// List configured plugin repositories
pub async fn list(ctx: &CliContext) -> Result<(), CliError> {
    println!("{} Registered plugin repositories", "→".green());
    println!();

    let repos = ctx.plugin_repo_store.list()?;
    if repos.is_empty() {
        println!("  No repositories configured.");
        return Ok(());
    }

    let mut table = Table::builder().headers(&["ID", "URL", "Description", "Last Synced"]);
    for (_, repo) in repos {
        let last_synced = repo
            .last_synced
            .map(|ts| ts.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "-".to_string());
        table = table.add_row(&[
            repo.id.as_str(),
            repo.url.as_str(),
            repo.description.as_deref().unwrap_or(""),
            last_synced.as_str(),
        ]);
    }

    println!("{}", table.build());
    Ok(())
}

/// Add a new plugin repository
pub async fn add(
    ctx: &CliContext,
    id: &str,
    url: &str,
    description: Option<&str>,
) -> Result<(), CliError> {
    if ctx.plugin_repo_store.get(id)?.is_some() {
        return Err(CliError::PluginError(format!(
            "Repository '{}' already exists",
            id
        )));
    }

    let repo = PluginRepoEntry {
        id: id.to_string(),
        url: url.to_string(),
        description: description.map(|d| d.to_string()),
        last_synced: Some(Utc::now()),
    };

    ctx.plugin_repo_store.save(id, &repo)?;
    println!(
        "{} Repository '{}' added ({})",
        "✓".green(),
        id,
        repo.url.cyan()
    );
    Ok(())
}

/// Remove a plugin repository
pub async fn remove(ctx: &CliContext, id: &str) -> Result<(), CliError> {
    if ctx.plugin_repo_store.get(id)?.is_none() {
        return Err(CliError::PluginError(format!(
            "Repository '{}' does not exist",
            id
        )));
    }

    ctx.plugin_repo_store.delete(id)?;

    // Also remove the cached catalog file
    let cache_dir = plugin_catalog::catalog_cache_dir(ctx.data_dir());
    let cache_file = cache_dir.join(format!("{}.json", id));
    if cache_file.exists() {
        let _ = std::fs::remove_file(cache_file);
    }

    println!("{} Repository '{}' removed", "✓".green(), id);
    Ok(())
}

/// Synchronize plugin catalogs from remote repositories
pub async fn sync(ctx: &CliContext, id: Option<&str>) -> Result<(), CliError> {
    let repos = if let Some(target_id) = id {
        let repo = ctx.plugin_repo_store.get(target_id)?.ok_or_else(|| {
            CliError::PluginError(format!("Repository '{}' does not exist", target_id))
        })?;
        vec![repo]
    } else {
        ctx.plugin_repo_store.list()?.into_values().collect()
    };

    if repos.is_empty() {
        println!("{} No repositories configured for synchronization.", "→".yellow());
        return Ok(());
    }

    let cache_dir = plugin_catalog::catalog_cache_dir(ctx.data_dir());
    std::fs::create_dir_all(&cache_dir).map_err(|e| {
        CliError::PluginError(format!("Failed to create catalog cache directory: {}", e))
    })?;

    for mut repo in repos {
        println!("{} Syncing repository '{}'...", "↻".blue(), repo.id.cyan());

        match plugin_catalog::fetch_remote_catalog(&repo.url).await {
            Ok(entries) => {
                let cache_file = cache_dir.join(format!("{}.json", repo.id));
                let json = serde_json::to_string_pretty(&entries).map_err(|e| {
                    CliError::PluginError(format!("Failed to serialize catalog: {}", e))
                })?;

                std::fs::write(&cache_file, json).map_err(|e| {
                    CliError::PluginError(format!("Failed to write catalog cache: {}", e))
                })?;

                repo.last_synced = Some(Utc::now());
                ctx.plugin_repo_store.save(&repo.id, &repo)?;

                println!(
                    "{} Repository '{}' synced successfully ({} entries)",
                    "✓".green(),
                    repo.id,
                    entries.len()
                );
            }
            Err(e) => {
                eprintln!(
                    "{} Failed to sync repository '{}': {}",
                    "✗".red(),
                    repo.id,
                    e
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn list_runs_without_error() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        list(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn add_persists_repository() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        add(
            &ctx,
            "custom",
            "https://example.org/plugins",
            Some("Example repo"),
        )
        .await
        .unwrap();

        let stored = ctx.plugin_repo_store.get("custom").unwrap().unwrap();
        assert_eq!(stored.url, "https://example.org/plugins");
        assert!(stored.last_synced.is_some());
    }
}
