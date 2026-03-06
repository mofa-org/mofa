//! Router pattern for dispatching tasks to specialized agents.

use super::patterns::{AgentOutput, AgentUnit};
use crate::llm::{LLMAgent, LLMError, LLMResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Routing configuration for [`Router`].
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Emit tracing logs during routing and execution.
    pub verbose: bool,
    /// Prompt template used to classify the task.
    pub classifier_prompt_template: String,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            verbose: true,
            classifier_prompt_template: concat!(
                "You are an expert task router.\n",
                "Choose the best route for the task below.\n",
                "Return only JSON: {\"route\":\"<route>\",\"reason\":\"<short why>\",\"confidence\":0.0}\n\n",
                "Available routes:\n{routes}\n\n",
                "Task:\n{task}"
            )
            .to_string(),
        }
    }
}

impl RouterConfig {
    /// Set verbose tracing mode.
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Override the classifier prompt template.
    pub fn with_classifier_prompt_template(mut self, template: impl Into<String>) -> Self {
        self.classifier_prompt_template = template.into();
        self
    }
}

#[derive(Clone)]
struct RouteEntry {
    description: Option<String>,
    unit: AgentUnit,
}

/// Structured routing decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterDecision {
    /// Route requested by the classifier.
    pub requested_route: String,
    /// Route actually executed.
    pub resolved_route: String,
    /// Short rationale from the classifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Optional confidence score from the classifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    /// Whether the router had to fall back to the default route.
    pub used_default: bool,
}

/// Result of a router dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterResult {
    /// Original task.
    pub task: String,
    /// Routing metadata.
    pub decision: RouterDecision,
    /// Output from the selected expert.
    pub output: AgentOutput,
    /// Total runtime in milliseconds.
    pub total_duration_ms: u64,
}

impl RouterResult {
    /// Render the routing trace as markdown.
    pub fn to_markdown_trace(&self) -> String {
        let mut lines = vec![
            "# Router Trace".to_string(),
            String::new(),
            format!("**Task:** {}", self.task),
            format!("**Requested Route:** {}", self.decision.requested_route),
            format!("**Resolved Route:** {}", self.decision.resolved_route),
            format!("**Used Default:** {}", self.decision.used_default),
        ];

        if let Some(reason) = &self.decision.reason {
            lines.push(format!("**Reason:** {}", reason));
        }
        if let Some(confidence) = self.decision.confidence {
            lines.push(format!("**Confidence:** {:.2}", confidence));
        }

        lines.push(String::new());
        lines.push("## Expert Output".to_string());
        lines.push("```text".to_string());
        lines.push(self.output.content.clone());
        lines.push("```".to_string());

        lines.join("\n")
    }
}

/// Classic router pattern.
pub struct Router {
    classifier: Arc<LLMAgent>,
    routes: Vec<(String, RouteEntry)>,
    default_route: Option<RouteEntry>,
    config: RouterConfig,
}

impl Router {
    /// Create a builder for [`Router`].
    pub fn builder() -> RouterBuilder {
        RouterBuilder::new()
    }

    /// Route and execute a task.
    pub async fn run(&self, task: impl Into<String>) -> LLMResult<RouterResult> {
        let task = task.into();
        let start = std::time::Instant::now();
        let route_catalog = self.render_routes();
        let prompt = fill_template(
            &self.config.classifier_prompt_template,
            &[("routes", route_catalog.as_str()), ("task", task.as_str())],
        );

        let classifier_raw = self.classifier.ask(&prompt).await?;
        let mut decision = self.parse_decision(&classifier_raw);

        let selected_entry = if let Some((name, entry)) = self.find_route(&decision.requested_route)
        {
            decision.resolved_route = name.to_string();
            decision.used_default = false;
            entry
        } else if let Some(default_route) = &self.default_route {
            decision.resolved_route = "default".to_string();
            decision.used_default = true;
            default_route
        } else {
            return Err(LLMError::Other(format!(
                "Router selected unknown route '{}'",
                decision.requested_route
            )));
        };

        if self.config.verbose {
            tracing::info!(
                "[Router] requested='{}' resolved='{}'",
                decision.requested_route,
                decision.resolved_route
            );
        }

        let output = selected_entry.unit.run(task.clone()).await?;

        Ok(RouterResult {
            task,
            decision,
            output,
            total_duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Alias for [`Self::run`].
    pub async fn execute(&self, task: impl Into<String>) -> LLMResult<RouterResult> {
        self.run(task).await
    }

    fn render_routes(&self) -> String {
        self.routes
            .iter()
            .map(|(name, entry)| match &entry.description {
                Some(description) => format!("- {}: {}", name, description),
                None => format!("- {}", name),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn parse_decision(&self, raw: &str) -> RouterDecision {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
            if let Some(route) = value.get("route").and_then(|route| route.as_str()) {
                return RouterDecision {
                    requested_route: route.to_string(),
                    resolved_route: route.to_string(),
                    reason: value
                        .get("reason")
                        .and_then(|reason| reason.as_str())
                        .map(ToOwned::to_owned),
                    confidence: value
                        .get("confidence")
                        .and_then(|confidence| confidence.as_f64())
                        .map(|confidence| confidence as f32),
                    used_default: false,
                };
            }
        }

        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            if let Some((name, _)) = self.find_route(trimmed) {
                return RouterDecision {
                    requested_route: name.to_string(),
                    resolved_route: name.to_string(),
                    reason: None,
                    confidence: None,
                    used_default: false,
                };
            }

            let lowered = trimmed.to_lowercase();
            if let Some((name, _)) = self
                .routes
                .iter()
                .find(|(name, _)| lowered.contains(&name.to_lowercase()))
            {
                return RouterDecision {
                    requested_route: name.to_string(),
                    resolved_route: name.to_string(),
                    reason: None,
                    confidence: None,
                    used_default: false,
                };
            }
        }

        RouterDecision {
            requested_route: trimmed.to_string(),
            resolved_route: trimmed.to_string(),
            reason: None,
            confidence: None,
            used_default: false,
        }
    }

    fn find_route(&self, requested_route: &str) -> Option<(&str, &RouteEntry)> {
        self.routes
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(requested_route))
            .map(|(name, entry)| (name.as_str(), entry))
    }
}

/// Builder for [`Router`].
pub struct RouterBuilder {
    classifier: Option<Arc<LLMAgent>>,
    routes: Vec<(String, RouteEntry)>,
    default_route: Option<RouteEntry>,
    config: RouterConfig,
}

impl RouterBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            classifier: None,
            routes: Vec::new(),
            default_route: None,
            config: RouterConfig::default(),
        }
    }

    /// Set the classifier agent.
    pub fn with_classifier(mut self, classifier: Arc<LLMAgent>) -> Self {
        self.classifier = Some(classifier);
        self
    }

    /// Add a named route backed by a ReAct agent.
    pub fn with_route(mut self, name: impl Into<String>, agent: Arc<super::ReActAgent>) -> Self {
        self.routes.push((
            name.into(),
            RouteEntry {
                description: None,
                unit: AgentUnit::react(agent),
            },
        ));
        self
    }

    /// Add a named route backed by an LLM agent.
    pub fn with_route_llm(mut self, name: impl Into<String>, agent: Arc<LLMAgent>) -> Self {
        self.routes.push((
            name.into(),
            RouteEntry {
                description: None,
                unit: AgentUnit::llm(agent),
            },
        ));
        self
    }

    /// Attach a human-readable description to a route.
    pub fn describe_route(
        mut self,
        route_name: impl AsRef<str>,
        description: impl Into<String>,
    ) -> Self {
        if let Some((_, entry)) = self
            .routes
            .iter_mut()
            .find(|(name, _)| name == route_name.as_ref())
        {
            entry.description = Some(description.into());
        }
        self
    }

    /// Set the default ReAct route.
    pub fn with_default(mut self, agent: Arc<super::ReActAgent>) -> Self {
        self.default_route = Some(RouteEntry {
            description: Some("Fallback route".to_string()),
            unit: AgentUnit::react(agent),
        });
        self
    }

    /// Set the default LLM route.
    pub fn with_default_llm(mut self, agent: Arc<LLMAgent>) -> Self {
        self.default_route = Some(RouteEntry {
            description: Some("Fallback route".to_string()),
            unit: AgentUnit::llm(agent),
        });
        self
    }

    /// Set the runtime config.
    pub fn with_config(mut self, config: RouterConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the router.
    pub fn build(self) -> Result<Router, LLMError> {
        let classifier = self
            .classifier
            .ok_or_else(|| LLMError::ConfigError("Router requires a classifier".to_string()))?;
        if self.routes.is_empty() {
            return Err(LLMError::ConfigError(
                "Router requires at least one route".to_string(),
            ));
        }

        Ok(Router {
            classifier,
            routes: self.routes,
            default_route: self.default_route,
            config: self.config,
        })
    }
}

impl Default for RouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn fill_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in replacements {
        rendered = rendered.replace(&format!("{{{}}}", key), value);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{MockLLMProvider, simple_llm_agent};

    #[test]
    fn builder_requires_classifier_and_routes() {
        let err = Router::builder().build().err().unwrap();
        assert!(matches!(err, LLMError::ConfigError(_)));
    }

    #[tokio::test]
    async fn router_selects_named_route() {
        let classifier_provider = Arc::new(MockLLMProvider::new("classifier"));
        classifier_provider
            .add_response(r#"{"route":"billing","reason":"invoice question","confidence":0.93}"#)
            .await;
        let billing_provider = Arc::new(MockLLMProvider::new("billing"));
        billing_provider
            .add_response("Billing specialist response")
            .await;
        let technical_provider = Arc::new(MockLLMProvider::new("technical"));
        technical_provider
            .add_response("Technical specialist response")
            .await;

        let router = Router::builder()
            .with_classifier(Arc::new(simple_llm_agent(
                "classifier",
                classifier_provider,
                "You classify tasks by specialty.",
            )))
            .with_route_llm(
                "technical",
                Arc::new(simple_llm_agent(
                    "technical",
                    technical_provider,
                    "You answer technical questions.",
                )),
            )
            .with_route_llm(
                "billing",
                Arc::new(simple_llm_agent(
                    "billing",
                    billing_provider,
                    "You answer billing questions.",
                )),
            )
            .with_config(RouterConfig::default().with_verbose(false))
            .build()
            .unwrap();

        let result = router.run("Why was I charged twice?").await.unwrap();

        assert_eq!(result.decision.resolved_route, "billing");
        assert_eq!(result.output.content, "Billing specialist response");
    }

    #[tokio::test]
    async fn router_falls_back_to_default() {
        let classifier_provider = Arc::new(MockLLMProvider::new("classifier"));
        classifier_provider
            .add_response(r#"{"route":"legal","reason":"not a supported expert"}"#)
            .await;
        let technical_provider = Arc::new(MockLLMProvider::new("technical"));
        technical_provider
            .add_response("Technical specialist response")
            .await;
        let default_provider = Arc::new(MockLLMProvider::new("default"));
        default_provider
            .add_response("General fallback response")
            .await;

        let router = Router::builder()
            .with_classifier(Arc::new(simple_llm_agent(
                "classifier",
                classifier_provider,
                "You classify tasks by specialty.",
            )))
            .with_route_llm(
                "technical",
                Arc::new(simple_llm_agent(
                    "technical",
                    technical_provider,
                    "You answer technical questions.",
                )),
            )
            .with_default_llm(Arc::new(simple_llm_agent(
                "default",
                default_provider,
                "You handle fallback questions.",
            )))
            .with_config(RouterConfig::default().with_verbose(false))
            .build()
            .unwrap();

        let result = router
            .run("Please review my contract language")
            .await
            .unwrap();

        assert!(result.decision.used_default);
        assert_eq!(result.decision.resolved_route, "default");
        assert_eq!(result.output.content, "General fallback response");
    }
}
