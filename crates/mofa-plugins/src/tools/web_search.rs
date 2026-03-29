use super::*;
use crate::PluginError;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;

/// Represents a single web search result.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResult {
    /// The title of the search result.
    pub title: String,
    /// The URL of the search result.
    pub url: String,
    /// A short snippet or description of the result.
    pub snippet: String,
}

/// A provider-neutral interface for performing web searches.
#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Returns the unique name of this provider.
    fn name(&self) -> &str;

    /// Performs a search for the given query.
    async fn search(&self, query: &str, max_results: usize) -> PluginResult<Vec<SearchResult>>;
}

// ============================================================================
// DuckDuckGo Provider
// ============================================================================

/// Search provider using the official DuckDuckGo Instant Answer API.
pub struct DuckDuckGoProvider {
    client: Client,
}

impl Default for DuckDuckGoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DuckDuckGoProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl SearchProvider for DuckDuckGoProvider {
    fn name(&self) -> &str {
        "duckduckgo"
    }

    async fn search(&self, query: &str, _max_results: usize) -> PluginResult<Vec<SearchResult>> {
        let url = "https://api.duckduckgo.com/";
        let response = self
            .client
            .get(url)
            .query(&[
                ("q", query),
                ("format", "json"),
                ("no_html", "1"),
                ("skip_disambig", "1"),
            ])
            .send()
            .await
            .map_err(|e| PluginError::ExecutionFailed(format!("DuckDuckGo request failed: {e}")))?;

        let data_res: Result<Value, reqwest::Error> = response.json::<Value>().await;
        let data: Value = data_res.map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to parse DuckDuckGo response: {e}"))
        })?;

        let mut results = Vec::new();

        if let Some(abstract_text) = data["AbstractText"].as_str() {
            let abstract_text: &str = abstract_text;
            if !abstract_text.is_empty() {
                results.push(SearchResult {
                    title: data["Heading"].as_str().unwrap_or("Abstract").to_string(),
                    url: data["AbstractURL"].as_str().unwrap_or("").to_string(),
                    snippet: abstract_text.to_string(),
                });
            }
        }

        if let Some(related) = data["RelatedTopics"].as_array() {
            for topic in related {
                let text: &str = topic["Text"].as_str().unwrap_or("");
                let link: &str = topic["FirstURL"].as_str().unwrap_or("");
                if !text.is_empty() && !link.is_empty() {
                    let title: &str = text.split(" - ").next().unwrap_or(text);
                    results.push(SearchResult {
                        title: title.to_string(),
                        url: link.to_string(),
                        snippet: text.to_string(),
                    });
                }
            }
        }

        Ok(results)
    }
}

// ============================================================================
// Brave Search Provider
// ============================================================================

/// Search provider using the Brave Search API.
pub struct BraveSearchProvider {
    client: Client,
    api_key: String,
}

impl BraveSearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl SearchProvider for BraveSearchProvider {
    fn name(&self) -> &str {
        "brave"
    }

    async fn search(&self, query: &str, max_results: usize) -> PluginResult<Vec<SearchResult>> {
        let url = "https://api.search.brave.com/res/v1/web/search";
        let response = self
            .client
            .get(url)
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .query(&[("q", query), ("count", &max_results.to_string())])
            .send()
            .await
            .map_err(|e| {
                PluginError::ExecutionFailed(format!("Brave Search request failed: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(PluginError::ExecutionFailed(format!(
                "Brave Search API error: {status}"
            )));
        }

        let data_res: Result<Value, reqwest::Error> = response.json::<Value>().await;
        let data: Value = data_res.map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to parse Brave Search response: {e}"))
        })?;

        let mut results = Vec::new();
        if let Some(web_results) = data["web"]["results"].as_array() {
            for res in web_results {
                let title: &str = res["title"].as_str().unwrap_or("");
                let url: &str = res["url"].as_str().unwrap_or("");
                let desc: &str = res["description"].as_str().unwrap_or("");
                results.push(SearchResult {
                    title: title.to_string(),
                    url: url.to_string(),
                    snippet: desc.to_string(),
                });
            }
        }

        Ok(results)
    }
}

// ============================================================================
// WebSearchPlugin (ToolExecutor)
// ============================================================================

/// Tool for performing web searches.
pub struct WebSearchTool {
    definition: ToolDefinition,
    providers: Vec<Box<dyn SearchProvider>>,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        let mut providers: Vec<Box<dyn SearchProvider>> = Vec::new();
        providers.push(Box::new(DuckDuckGoProvider::new()));

        if let Ok(key) = env::var("BRAVE_SEARCH_API_KEY") {
            let key = key.trim();
            if !key.is_empty() {
                providers.push(Box::new(BraveSearchProvider::new(key.to_string())));
            }
        }

        Self {
            definition: ToolDefinition {
                name: "web_search".to_string(),
                description: "Search the web for information using multiple providers.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "The search query" },
                        "max_results": { "type": "integer", "default": 5, "minimum": 1, "maximum": 20 },
                        "provider": { "type": "string", "enum": ["auto", "duckduckgo", "brave"], "default": "auto" }
                    },
                    "required": ["query"]
                }),
                requires_confirmation: false,
            },
            providers,
        }
    }
}

#[async_trait]
impl ToolExecutor for WebSearchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: Value) -> PluginResult<Value> {
        let query: &str = arguments["query"]
            .as_str()
            .ok_or_else(|| PluginError::ExecutionFailed("query is required".to_string()))?;

        let max_results = arguments["max_results"].as_u64().unwrap_or(5) as usize;
        let provider_pref: &str = arguments["provider"].as_str().unwrap_or("auto");

        let provider = if provider_pref == "auto" {
            self.providers
                .iter()
                .find(|p| p.name() == "brave")
                .or_else(|| self.providers.iter().find(|p| p.name() == "duckduckgo"))
        } else {
            self.providers.iter().find(|p| p.name() == provider_pref)
        };

        let Some(provider) = provider else {
            return Err(PluginError::ExecutionFailed(format!(
                "Provider '{provider_pref}' not found"
            )));
        };

        let results = provider.search(query, max_results).await?;

        Ok(json!({
            "query": query,
            "provider": provider.name(),
            "results": results
        }))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSearchProvider {
        name: String,
        results: Vec<SearchResult>,
        should_fail: bool,
    }

    impl MockSearchProvider {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                results: vec![SearchResult {
                    title: format!("Result from {name}"),
                    url: format!("https://{name}.com"),
                    snippet: "Mock snippet".to_string(),
                }],
                should_fail: false,
            }
        }
    }

    #[async_trait]
    impl SearchProvider for MockSearchProvider {
        fn name(&self) -> &str {
            &self.name
        }

        async fn search(
            &self,
            _query: &str,
            _max_results: usize,
        ) -> PluginResult<Vec<SearchResult>> {
            if self.should_fail {
                return Err(PluginError::ExecutionFailed("Mock failure".to_string()));
            }
            Ok(self.results.clone())
        }
    }

    #[tokio::test]
    async fn test_web_search_tool_provider_selection() {
        let mut tool = WebSearchTool::new();
        // Replace real providers with mocks for deterministic testing
        tool.providers = vec![
            Box::new(MockSearchProvider::new("duckduckgo")),
            Box::new(MockSearchProvider::new("brave")),
        ];

        // Test explicit selection
        let args = json!({ "query": "test", "provider": "brave" });
        let result = tool.execute(args).await.unwrap();
        assert_eq!(result["provider"], "brave");

        // Test auto selection (Brave should be preferred if available)
        let args = json!({ "query": "test", "provider": "auto" });
        let result = tool.execute(args).await.unwrap();
        assert_eq!(result["provider"], "brave");
    }

    #[tokio::test]
    async fn test_web_search_tool_missing_query() {
        let tool = WebSearchTool::new();
        let args = json!({ "max_results": 5 });
        let result = tool.execute(args).await;
        assert!(result.is_err());
    }

    #[ignore]
    #[tokio::test]
    async fn test_real_duckduckgo_search() {
        let provider = DuckDuckGoProvider::new();
        let results = provider.search("Rust programming", 3).await.unwrap();
        assert!(!results.is_empty());
        for res in results {
            assert!(!res.title.is_empty());
            assert!(!res.url.is_empty());
        }
    }

    #[ignore]
    #[tokio::test]
    async fn test_real_brave_search() {
        let Ok(key) = env::var("BRAVE_SEARCH_API_KEY") else {
            return; // Skip if no key
        };
        let provider = BraveSearchProvider::new(key);
        let results = provider.search("OpenAI", 3).await.unwrap();
        assert!(!results.is_empty());
        for res in results {
            assert!(!res.title.is_empty());
            assert!(!res.url.is_empty());
        }
    }
}
