//! `mofa plugin list` command implementation

use crate::CliError;
use crate::context::CliContext;
use crate::output::Table;
use crate::plugin_catalog::catalog_entries;
use colored::Colorize;
use mofa_kernel::agent::plugins::PluginRegistry;

/// Execute the `mofa plugin list` command
pub async fn run(ctx: &CliContext, installed_only: bool, available: bool) -> Result<(), CliError> {
    let show_available = available;
    let show_installed = installed_only || !available;

    if show_available {
        println!("{} Available plugin catalog", "→".green());
        println!();
        print_available(ctx)?;
    }

    if show_installed {
        println!("{} Installed plugins", "→".green());
        println!();
        print_installed(ctx)?;
    }

    if !show_available && !show_installed {
        println!("{} No plugin listing requested.", "→".yellow());
    }

    Ok(())
}

fn print_available(ctx: &CliContext) -> Result<(), CliError> {
    let entries = catalog_entries();
    if entries.is_empty() {
        println!("  No catalog entries available.");
        return Ok(());
    }

    let mut table = Table::builder().headers(&["ID", "Name", "Repo", "Kind", "Description", "Installed"]);
    for entry in entries {
        let installed = ctx.plugin_registry.contains(&entry.id);
        table = table.add_row(&[
            entry.id.as_str(),
            entry.name.as_str(),
            entry.repo_id.as_str(),
            entry.kind.as_str(),
            entry.description.as_str(),
            if installed { "yes" } else { "no" },
        ]);
    }

    println!("{}", table.build());
    Ok(())
}

fn print_installed(ctx: &CliContext) -> Result<(), CliError> {
    let specs = ctx.plugin_store.list()?;
    let mut table = Table::builder().headers(&["ID", "Kind", "Repo", "Description", "Enabled"]);
    let mut found = false;
    for (_, spec) in specs {
        if !spec.enabled {
            continue;
        }
        found = true;
        table = table.add_row(&[
            spec.id.as_str(),
            spec.kind.as_str(),
            spec.repo_id.as_deref().unwrap_or("local"),
            spec.description.as_deref().unwrap_or(""),
            "yes",
        ]);
    }

    if found {
        println!("{}", table.build());
    } else {
        println!("  No plugins installed.");
    }
    println!();
}

fn print_available(catalog: &CachedPluginCatalog) {
    println!(
        "{} Available plugins (source: {})",
        "→".green(),
        catalog.source
    );
    println!("  Fetched at: {}", catalog.fetched_at);
    println!();

    if catalog.catalog.plugins.is_empty() {
        println!("  No plugins found in catalog.");
        return;
    }

    let infos: Vec<AvailableInfo> = catalog
        .catalog
        .plugins
        .iter()
        .map(AvailableInfo::from_entry)
        .collect();

    let json = serde_json::to_value(&infos).unwrap_or_default();
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn run_default_list() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        run(&ctx, false, false).await.unwrap();
    }

    #[tokio::test]
    async fn run_available_list() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        run(&ctx, false, true).await.unwrap();
    }
}

#[derive(Debug, Clone, Serialize)]
struct AvailableInfo {
    name: String,
    latest_version: String,
    kind: String,
    tags: String,
    description: String,
}

impl AvailableInfo {
    fn from_entry(entry: &PluginCatalogEntry) -> Self {
        let latest = entry
            .releases
            .iter()
            .max_by(|a, b| super::catalog::compare_versions(&a.version, &b.version))
            .or_else(|| select_latest_release(&entry.releases));

        Self {
            name: entry.id.clone(),
            latest_version: latest
                .map(|r| r.version.clone())
                .unwrap_or_else(|| "-".to_string()),
            kind: entry.kind.clone().unwrap_or_else(|| entry.id.clone()),
            tags: if entry.tags.is_empty() {
                "-".to_string()
            } else {
                entry.tags.join(", ")
            },
            description: entry.description.clone().unwrap_or_default(),
        }
    }
}
