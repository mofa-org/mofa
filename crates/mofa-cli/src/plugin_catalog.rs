use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_PLUGIN_REPO_ID: &str = "official";
pub const DEFAULT_PLUGIN_REPO_URL: &str = "https://plugins.mofa.dev";
pub const DEFAULT_PLUGIN_REPO_DESCRIPTION: &str = "Official MoFA plugin catalog.";

/// Plugin repository metadata stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRepoEntry {
    pub id: String,
    pub url: String,
    pub description: Option<String>,
    pub last_synced: Option<DateTime<Utc>>,
}

/// Describes a plugin that can be installed from a repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCatalogEntry {
    pub id: String,
    pub repo_id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub config: Value,
}

fn base_catalog() -> Vec<PluginCatalogEntry> {
    vec![PluginCatalogEntry {
        id: "http-plugin".to_string(),
        repo_id: DEFAULT_PLUGIN_REPO_ID.to_string(),
        name: "HTTP Helper".to_string(),
        description:
            "Implements the builtin HTTP helper plugin and exposes an HTTP client to agents."
                .to_string(),
        kind: "builtin:http".to_string(),
        config: serde_json::json!({ "url": "https://example.com" }),
    }]
}

/// Returns the catalog repositories that should be seeded by default.
pub fn default_repos() -> Vec<PluginRepoEntry> {
    vec![PluginRepoEntry {
        id: DEFAULT_PLUGIN_REPO_ID.to_string(),
        url: DEFAULT_PLUGIN_REPO_URL.to_string(),
        description: Some(DEFAULT_PLUGIN_REPO_DESCRIPTION.to_string()),
        last_synced: None,
    }]
}

/// Returns the directory where plugin catalogs are cached.
pub fn catalog_cache_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("catalog_cache")
}

/// Returns all catalog entries available to the CLI, merged from builtin and cached repositories.
pub fn catalog_entries(data_dir: &std::path::Path) -> Vec<PluginCatalogEntry> {
    let mut entries = base_catalog();

    let cache_dir = catalog_cache_dir(data_dir);
    if let Ok(read_dir) = std::fs::read_dir(cache_dir) {
        for entry in read_dir.flatten() {
            if let Ok(file_content) = std::fs::read_to_string(entry.path()) {
                if let Ok(mut repo_entries) =
                    serde_json::from_str::<Vec<PluginCatalogEntry>>(&file_content)
                {
                    entries.append(&mut repo_entries);
                }
            }
        }
    }

    entries
}

/// Fetch a plugin catalog from a remote repository URL.
pub async fn fetch_remote_catalog(url: &str) -> Result<Vec<PluginCatalogEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch catalog from {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Server returned error status: {} for {}",
            response.status(),
            url
        ));
    }

    let entries = response
        .json::<Vec<PluginCatalogEntry>>()
        .await
        .map_err(|e| format!("Failed to parse catalog JSON: {}", e))?;

    Ok(entries)
}

/// Returns catalog entries filtered by repository identifier.
pub fn catalog_for_repo(repo_id: &str, data_dir: &std::path::Path) -> Vec<PluginCatalogEntry> {
    catalog_entries(data_dir)
        .into_iter()
        .filter(|entry| entry.repo_id == repo_id)
        .collect()
}

/// Find a single catalog entry by repository and plugin id.
pub fn find_catalog_entry(
    repo_id: &str,
    plugin_id: &str,
    data_dir: &std::path::Path,
) -> Option<PluginCatalogEntry> {
    catalog_entries(data_dir)
        .into_iter()
        .find(|entry| entry.repo_id == repo_id && entry.id == plugin_id)
}
