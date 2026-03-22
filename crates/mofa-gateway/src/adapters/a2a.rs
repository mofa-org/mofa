//! Agent-to-Agent (A2A) protocol adapter.
//!
//! Provides the ability to discover and interact with remote MoFA agents
//! over the network via standard HTTP endpoints (`/.well-known/agent-card`, `/tasks`).

use async_trait::async_trait;
use mofa_kernel::gateway::{GatewayAdapter, GatewayContext, GatewayRequest, GatewayResponse, DispatchError};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Error type for A2A operations.
#[derive(Debug, thiserror::Error)]
pub enum A2aError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
}

impl From<A2aError> for DispatchError {
    fn from(err: A2aError) -> Self {
        DispatchError::AdapterInvocationFailed {
            adapter: "a2a".into(),
            reason: err.to_string(),
        }
    }
}

/// Description of a capability provided by a remote agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub name: String,
    pub description: String,
}

/// An Agent Card declaring the identity and capabilities of an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<CapabilityDescriptor>,
    pub endpoint: String,
}

/// Status of an asynchronous task on a remote agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum A2aTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A representation of a remote task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTask {
    pub id: String,
    pub status: A2aTaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Adapter for invoking and managing tasks on external agents.
pub struct A2aAdapter {
    http: Client,
    base_url: String,
}

impl A2aAdapter {
    /// Create a new A2A adapter targeting a default base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Discover the remote agent's capabilities via its Agent Card.
    pub async fn discover(&self, agent_url: &str) -> Result<AgentCard, A2aError> {
        let url = format!("{}/.well-known/agent-card", agent_url.trim_end_matches('/'));
        let res = self.http.get(&url).send().await?.error_for_status()?;
        Ok(res.json().await?)
    }

    /// Create a new task on the remote agent.
    pub async fn create_task(
        &self,
        agent_url: &str,
        input: serde_json::Value,
    ) -> Result<A2aTask, A2aError> {
        let url = format!("{}/tasks", agent_url.trim_end_matches('/'));
        let res = self.http.post(&url).json(&input).send().await?.error_for_status()?;
        Ok(res.json().await?)
    }

    /// Poll the current status of a specific task.
    pub async fn poll_task(&self, agent_url: &str, task_id: &str) -> Result<A2aTask, A2aError> {
        let url = format!("{}/tasks/{}", agent_url.trim_end_matches('/'), task_id);
        let res = self.http.get(&url).send().await?.error_for_status()?;
        Ok(res.json().await?)
    }

    /// Cancel an ongoing task.
    pub async fn cancel_task(&self, agent_url: &str, task_id: &str) -> Result<(), A2aError> {
        let url = format!("{}/tasks/{}", agent_url.trim_end_matches('/'), task_id);
        self.http.delete(&url).send().await?.error_for_status()?;
        Ok(())
    }
}

#[async_trait]
impl GatewayAdapter for A2aAdapter {
    fn name(&self) -> &str {
        "a2a"
    }

    async fn invoke(
        &self,
        req: &GatewayRequest,
        _ctx: &GatewayContext,
    ) -> Result<GatewayResponse, DispatchError> {
        // Fallback target URL; in a complete implementation, this would be retrieved
        // from routing metadata or context attributes. We check headers for tests/simplicity.
        let target_url = req
            .headers
            .get("x-a2a-target-url")
            .map(|s| s.as_str())
            .unwrap_or(&self.base_url);

        // Attempt to parse the body as JSON input for the remote task.
        let input: serde_json::Value = if req.body.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_slice(&req.body).map_err(|e| A2aError::Api(e.to_string()))?
        };

        // If the path contains a task ID suffix, we poll. Otherwise, we create.
        // E.g., `/tasks/t-123`
        let path_segments: Vec<&str> = req.path.trim_matches('/').split('/').collect();
        let (task, status_code) = if path_segments.len() >= 2 && path_segments[0] == "tasks" {
            let task_id = path_segments[1];
            (self.poll_task(target_url, task_id).await?, 200)
        } else {
            (self.create_task(target_url, input).await?, 202)
        };

        let mut res = GatewayResponse::new(status_code, "a2a-adapter");
        res.body = serde_json::to_vec(&task).unwrap_or_default();
        
        Ok(res.with_header("content-type", "application/json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::gateway::route::HttpMethod;
    use mockito;

    fn capability_descriptor_from_agent_card() {
        let json = r#"
        {
            "id": "agent-1",
            "name": "Math Agent",
            "capabilities": [
                {
                    "name": "add",
                    "description": "Adds two numbers"
                }
            ],
            "endpoint": "http://localhost:8080"
        }
        "#;
        let card: AgentCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.capabilities.len(), 1);
        assert_eq!(card.capabilities[0].name, "add");
    }

    #[test]
    fn capability_descriptor_from_agent_card_test() {
        capability_descriptor_from_agent_card()
    }

    #[tokio::test]
    async fn discover_parses_agent_card() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mock = server.mock("GET", "/.well-known/agent-card")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"a1","name":"Test","capabilities":[],"endpoint":"/here"}"#)
            .create_async().await;

        let adapter = A2aAdapter::new(&url);
        let card = adapter.discover(&url).await.unwrap();

        assert_eq!(card.id, "a1");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn discover_404_returns_error() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mock = server.mock("GET", "/.well-known/agent-card")
            .with_status(404)
            .create_async().await;

        let adapter = A2aAdapter::new(&url);
        let err = adapter.discover(&url).await.unwrap_err();

        match err {
            A2aError::Http(_) => {}
            _ => panic!("Expected HTTP error"),
        }
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_task_returns_task_id() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mock = server.mock("POST", "/tasks")
            .with_status(202)
            .with_body(r#"{"id":"t-123","status":"pending"}"#)
            .create_async().await;

        let adapter = A2aAdapter::new(&url);
        let task = adapter.create_task(&url, serde_json::json!({})).await.unwrap();

        assert_eq!(task.id, "t-123");
        assert_eq!(task.status, A2aTaskStatus::Pending);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn poll_task_completed() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mock = server.mock("GET", "/tasks/t-123")
            .with_status(200)
            .with_body(r#"{"id":"t-123","status":"completed","result":{"value":42}}"#)
            .create_async().await;

        let adapter = A2aAdapter::new(&url);
        let task = adapter.poll_task(&url, "t-123").await.unwrap();

        assert_eq!(task.status, A2aTaskStatus::Completed);
        assert_eq!(task.result.unwrap()["value"], 42);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn poll_task_pending() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mock = server.mock("GET", "/tasks/t-123")
            .with_status(200)
            .with_body(r#"{"id":"t-123","status":"running"}"#)
            .create_async().await;

        let adapter = A2aAdapter::new(&url);
        let task = adapter.poll_task(&url, "t-123").await.unwrap();

        assert_eq!(task.status, A2aTaskStatus::Running);
        assert!(task.result.is_none());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn cancel_task_ok() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mock = server.mock("DELETE", "/tasks/t-123")
            .with_status(204)
            .create_async().await;

        let adapter = A2aAdapter::new(&url);
        assert!(adapter.cancel_task(&url, "t-123").await.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn adapter_invoke_dispatches_to_create_task() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();

        let mock = server.mock("POST", "/tasks")
            .with_status(202)
            .with_body(r#"{"id":"t-999","status":"pending"}"#)
            .create_async().await;

        let adapter = A2aAdapter::new(&url);
        let mut req = GatewayRequest::new("id-1", "/tasks", HttpMethod::Post);
        req.headers.insert("x-a2a-target-url".to_string(), url.clone());

        let res = adapter.invoke(&req, &GatewayContext::new(req.clone())).await.unwrap();
        assert_eq!(res.status, 202);
        let task: A2aTask = serde_json::from_slice(&res.body).unwrap();
        assert_eq!(task.id, "t-999");
        mock.assert_async().await;
    }
}
