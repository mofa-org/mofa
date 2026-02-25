//! Plugin catalog types, cache handling, and resolution helpers

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_CATALOG_URL: &str = "https://registry.mofa.ai/plugins/catalog.json";
pub const DEFAULT_TIMEOUT_SECS: u64 = 10;
const CACHE_FILE: &str = "plugin-catalog.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCatalog {
    #[serde(default)]
    pub plugins: Vec<PluginCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCatalogEntry {
    pub id: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub default_config: Option<Value>,
    #[serde(default)]
    pub releases: Vec<PluginCatalogRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCatalogRelease {
    pub version: String,
    #[serde(default)]
    pub source: Option<PluginCatalogSource>,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PluginCatalogSource {
    Registry {
        package: String,
    },
    Git {
        repo: String,
        #[serde(default)]
        rev: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
    Tarball {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPluginCatalog {
    pub fetched_at: DateTime<Utc>,
    pub source: String,
    pub catalog: PluginCatalog,
}

#[derive(Debug, Clone)]
pub struct ResolvedPlugin {
    pub id: String,
    pub version: String,
    pub kind: String,
    pub config: Value,
}

pub struct CatalogService {
    cache_path: PathBuf,
}

impl CatalogService {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            cache_path: data_dir.join(CACHE_FILE),
        }
    }

    pub fn cache_path(&self) -> &Path {
        &self.cache_path
    }

    pub async fn fetch_remote(
        &self,
        url: &str,
        timeout: Duration,
    ) -> anyhow::Result<PluginCatalog> {
        let client = reqwest::Client::builder().timeout(timeout).build()?;

        let resp = client.get(url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("Failed to fetch catalog (status {}): {}", status, url);
        }

        let catalog: PluginCatalog = resp.json().await?;
        validate_catalog(&catalog)?;
        Ok(catalog)
    }

    pub fn read_cache(&self) -> anyhow::Result<Option<CachedPluginCatalog>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let payload = fs::read(&self.cache_path)?;
        let cached: CachedPluginCatalog = serde_json::from_slice(&payload)?;
        Ok(Some(cached))
    }

    pub fn write_cache(&self, cached: &CachedPluginCatalog) -> anyhow::Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_vec_pretty(cached)?;
        fs::write(&self.cache_path, payload)?;
        Ok(())
    }

    pub async fn sync(
        &self,
        url: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> anyhow::Result<CachedPluginCatalog> {
        let catalog_url = url.unwrap_or(DEFAULT_CATALOG_URL);
        let timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));
        let catalog = self.fetch_remote(catalog_url, timeout).await?;
        let cached = CachedPluginCatalog {
            fetched_at: Utc::now(),
            source: catalog_url.to_string(),
            catalog,
        };
        self.write_cache(&cached)?;
        Ok(cached)
    }

    pub fn resolve(
        &self,
        catalog: &PluginCatalog,
        name: &str,
        version: Option<&str>,
    ) -> anyhow::Result<Option<ResolvedPlugin>> {
        let entry = match catalog.plugins.iter().find(|p| p.id == name) {
            Some(entry) => entry,
            None => return Ok(None),
        };

        let release = select_release(&entry.releases, version);
        let release = match release {
            Some(r) => r,
            None => anyhow::bail!("No matching release found for plugin '{}'", name),
        };

        let kind = entry.kind.clone().unwrap_or_else(|| entry.id.clone());
        let config = entry.default_config.clone().unwrap_or(Value::Null);

        Ok(Some(ResolvedPlugin {
            id: entry.id.clone(),
            version: release.version.clone(),
            kind,
            config,
        }))
    }
}

fn validate_catalog(catalog: &PluginCatalog) -> anyhow::Result<()> {
    for entry in &catalog.plugins {
        if entry.id.trim().is_empty() {
            anyhow::bail!("Catalog entry missing id");
        }
    }
    Ok(())
}

pub fn select_latest_release(releases: &[PluginCatalogRelease]) -> Option<&PluginCatalogRelease> {
    select_release(releases, None)
}

pub fn select_release<'a>(
    releases: &'a [PluginCatalogRelease],
    requested: Option<&'a str>,
) -> Option<&'a PluginCatalogRelease> {
    if let Some(req) = requested
        && let Some(found) = releases.iter().find(|r| r.version == req)
    {
        return Some(found);
    }

    releases
        .iter()
        .filter(|r| !r.yanked)
        .max_by(|a, b| compare_versions(&a.version, &b.version))
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    match (Version::parse(a), Version::parse(b)) {
        (Ok(av), Ok(bv)) => av.cmp(&bv),
        _ => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selects_latest_semver() {
        let releases = vec![
            PluginCatalogRelease {
                version: "0.9.0".to_string(),
                source: None,
                checksum: None,
                yanked: false,
                updated_at: None,
            },
            PluginCatalogRelease {
                version: "1.2.0".to_string(),
                source: None,
                checksum: None,
                yanked: false,
                updated_at: None,
            },
        ];

        let selected = select_release(&releases, None).unwrap();
        assert_eq!(selected.version, "1.2.0");
    }

    #[test]
    fn test_selects_requested_version() {
        let releases = vec![
            PluginCatalogRelease {
                version: "0.1.0".to_string(),
                source: None,
                checksum: None,
                yanked: false,
                updated_at: None,
            },
            PluginCatalogRelease {
                version: "0.2.0".to_string(),
                source: None,
                checksum: None,
                yanked: false,
                updated_at: None,
            },
        ];

        let selected = select_release(&releases, Some("0.1.0")).unwrap();
        assert_eq!(selected.version, "0.1.0");
    }

    #[test]
    fn test_fallbacks_to_string_compare_when_not_semver() {
        let releases = vec![
            PluginCatalogRelease {
                version: "snapshot".to_string(),
                source: None,
                checksum: None,
                yanked: false,
                updated_at: None,
            },
            PluginCatalogRelease {
                version: "2024-01-01".to_string(),
                source: None,
                checksum: None,
                yanked: false,
                updated_at: None,
            },
        ];

        let selected = select_release(&releases, None).unwrap();
        assert_eq!(selected.version, "snapshot");
    }

    #[test]
    fn test_resolves_entry() {
        let catalog = PluginCatalog {
            plugins: vec![PluginCatalogEntry {
                id: "http-plugin".to_string(),
                kind: Some("builtin:http".to_string()),
                description: None,
                homepage: None,
                tags: vec![],
                default_config: Some(serde_json::json!({"url": "https://example.com"})),
                releases: vec![PluginCatalogRelease {
                    version: "1.0.0".to_string(),
                    source: None,
                    checksum: None,
                    yanked: false,
                    updated_at: None,
                }],
            }],
        };

        let service = CatalogService::new(Path::new("/tmp"));
        let resolved = service
            .resolve(&catalog, "http-plugin", None)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.id, "http-plugin");
        assert_eq!(resolved.kind, "builtin:http");
        assert_eq!(resolved.version, "1.0.0");
        assert!(resolved.config.get("url").is_some());
    }
}
