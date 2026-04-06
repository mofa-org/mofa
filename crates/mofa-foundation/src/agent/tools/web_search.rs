use crate::agent::components::tool::{SimpleTool, ToolCategory};
use async_trait::async_trait;
use mofa_kernel::agent::components::tool::{ToolInput, ToolMetadata, ToolResult};
use mofa_plugins::tools::web_search::{BraveSearchProvider, DuckDuckGoProvider, SearchProvider};
use serde_json::{Value, json};
use std::env;

/// A tool for performing web searches.
///
/// Implements [`SimpleTool`] and supports multiple search providers (DuckDuckGo, Brave).
pub struct WebSearchTool {
    providers: Vec<Box<dyn SearchProvider>>,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    /// Creates a new `WebSearchTool` with available providers.
    ///
    /// Automatically detects `BRAVE_SEARCH_API_KEY` in the environment.
    pub fn new() -> Self {
        let mut providers: Vec<Box<dyn SearchProvider>> = Vec::new();
        providers.push(Box::new(DuckDuckGoProvider::new()));

        if let Ok(key) = env::var("BRAVE_SEARCH_API_KEY") {
            if !key.trim().is_empty() {
                providers.push(Box::new(BraveSearchProvider::new(key)));
            }
        }

        Self { providers }
    }

    /// Creates a new `WebSearchTool` with a custom set of providers.
    pub fn with_providers(providers: Vec<Box<dyn SearchProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl SimpleTool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for real-time information. \
         Returns a list of results with titles, URLs, and snippets. \
         Supports DuckDuckGo (default) and Brave Search."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query (e.g., 'current price of Bitcoin')"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Defaults to 5.",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 20
                },
                "provider": {
                    "type": "string",
                    "enum": ["auto", "duckduckgo", "brave"],
                    "description": "Optional search provider preference. 'auto' selects the best available."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        let query = match input.get_str("query") {
            Some(q) => q,
            None => return ToolResult::failure("Missing required parameter: query"),
        };

        let max_results = input.get_number("max_results").unwrap_or(5.0) as usize;
        let provider_pref = input.get_str("provider").unwrap_or("auto");

        let provider = if provider_pref == "auto" {
            self.providers
                .iter()
                .find(|p| p.name() == "brave")
                .or_else(|| self.providers.iter().find(|p| p.name() == "duckduckgo"))
        } else {
            self.providers.iter().find(|p| p.name() == provider_pref)
        };

        let Some(provider) = provider else {
            return ToolResult::failure(format!(
                "Search provider '{}' is not available or not configured.",
                provider_pref
            ));
        };

        match provider.search(query, max_results).await {
            Ok(results) => ToolResult::success(json!({
                "query": query,
                "provider": provider.name(),
                "results": results
            })),
            Err(err) => ToolResult::failure(format!("Search failed: {err}")),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::new().needs_network()
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Web
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::context::AgentContext;

    struct MockSearchProvider {
        name: String,
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
        ) -> mofa_plugins::PluginResult<Vec<mofa_plugins::tools::web_search::SearchResult>>
        {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_foundation_web_search_tool_metadata() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        assert!(tool.metadata().requires_network);
        assert_eq!(tool.category(), ToolCategory::Web);
    }

    #[tokio::test]
    async fn test_foundation_web_search_tool_params() {
        let tool = WebSearchTool::new();
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("query"))
        );
    }

    #[tokio::test]
    async fn test_foundation_web_search_execute_missing_query() {
        let tool = WebSearchTool::new();
        let input = ToolInput::from_json(json!({ "max_results": 10 }));
        let result = tool.execute(input).await;
        assert!(!result.success);
        assert!(
            result
                .error
                .unwrap()
                .contains("Missing required parameter: query")
        );
    }
}
