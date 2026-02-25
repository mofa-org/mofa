//! `mofa plugin sync` command implementation

use super::catalog::{CatalogService, DEFAULT_CATALOG_URL, DEFAULT_TIMEOUT_SECS};
use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa plugin sync` command
pub async fn run(ctx: &CliContext, url: Option<&str>, timeout: Option<u64>) -> anyhow::Result<()> {
    let service = CatalogService::new(&ctx.data_dir);
    let catalog_url = url.unwrap_or(DEFAULT_CATALOG_URL);
    let timeout_secs = timeout.unwrap_or(DEFAULT_TIMEOUT_SECS);

    println!(
        "{} Syncing plugin catalog from {} (timeout: {}s)",
        "→".green(),
        catalog_url,
        timeout_secs
    );

    let cached = service.sync(Some(catalog_url), Some(timeout_secs)).await?;

    println!(
        "{} Synced {} plugins (source: {}, fetched_at: {})",
        "✓".green(),
        cached.catalog.plugins.len(),
        cached.source,
        cached.fetched_at,
    );

    Ok(())
}
