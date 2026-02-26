//! `mofa plugin list` command implementation

use super::catalog::{
    CachedPluginCatalog, CatalogService, PluginCatalogEntry, select_latest_release,
};
use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use mofa_kernel::agent::plugins::PluginRegistry;
use serde::Serialize;

/// Execute the `mofa plugin list` command
pub async fn run(
    ctx: &CliContext,
    installed: bool,
    available: bool,
    refresh: bool,
) -> anyhow::Result<()> {
    let want_installed = installed || !available;
    let want_available = available;

    if want_installed {
        print_installed(ctx);
    }

    if want_available {
        let service = CatalogService::new(&ctx.data_dir);
        let cached = if refresh {
            Some(service.sync(None, None).await?)
        } else {
            service.read_cache()?
        };

        match cached {
            Some(catalog) => print_available(&catalog),
            None => {
                println!(
                    "{} No catalog found. Run `mofa plugin sync` or pass `--refresh` to fetch it.",
                    "!".yellow()
                );
            }
        }
    }

    Ok(())
}

fn print_installed(ctx: &CliContext) {
    println!("{} Installed plugins", "→".green());
    println!();

    let plugins = ctx.plugin_registry.list();

    if plugins.is_empty() {
        println!("  No plugins registered.");
        println!();
        println!("  Plugins can be registered programmatically via the SDK.");
        return;
    }

    let infos: Vec<InstalledInfo> = plugins
        .iter()
        .map(|p| {
            let metadata = p.metadata();
            InstalledInfo {
                name: p.name().to_string(),
                version: metadata.version.clone(),
                description: p.description().to_string(),
                stages: metadata
                    .stages
                    .iter()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
                    .join(", "),
            }
        })
        .collect();

    let json = serde_json::to_value(&infos).unwrap_or_default();
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
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

#[derive(Debug, Clone, Serialize)]
struct InstalledInfo {
    name: String,
    version: String,
    description: String,
    stages: String,
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
