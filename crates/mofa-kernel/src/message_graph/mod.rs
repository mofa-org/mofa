//! MessageGraph module
//!
//! Defines message-related graph contracts and state types.
//!
//! This module intentionally supports two complementary layers:
//! - `MessageGraph`: message-routing topology (transport-level routing contracts)
//! - `MessageState`: workflow state model for `StateGraph` where `messages` is the primary key
//!
//! For developer-facing workflow composition, prefer `MessageState` with `StateGraph`.
//! Use `MessageGraph` when you need explicit transport/routing topology contracts.

use crate::agent::error::{AgentError, AgentResult};
use crate::workflow::{GraphState, StateUpdate};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

mod executor;
pub use executor::*;

/// Canonical messages key for StateGraph-based message state.
pub const MESSAGES_KEY: &str = "messages";

/// Error type for MessageGraph construction/validation.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MessageGraphError {
    #[error("graph id cannot be empty")]
    EmptyGraphId,
    #[error("graph must contain at least one node")]
    NoNodes,
    #[error("graph must contain at least one edge")]
    NoEdges,
    #[error("graph must define at least one entry point")]
    NoEntryPoints,
    #[error("max hops must be greater than 0, got {0}")]
    InvalidMaxHops(u16),
    #[error("node id cannot be empty")]
    EmptyNodeId,
    #[error("node '{0}' already exists")]
    DuplicateNode(String),
    #[error("node '{0}' does not exist")]
    MissingNode(String),
    #[error("entry point '{0}' already exists")]
    DuplicateEntryPoint(String),
    #[error("edge from '{from}' to '{to}' already exists with the same route rule")]
    DuplicateEdge { from: String, to: String },
    #[error("invalid route rule: {0}")]
    InvalidRouteRule(String),
    #[error("unreachable nodes detected: {0:?}")]
    UnreachableNodes(Vec<String>),
}

/// Message envelope routed through MessageGraph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageEnvelope {
    pub message_type: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub payload: Vec<u8>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub hop_count: u16,
}

impl MessageEnvelope {
    pub fn new(message_type: impl Into<String>, payload: Vec<u8>) -> Self {
        Self {
            message_type: message_type.into(),
            headers: HashMap::new(),
            payload,
            trace_id: None,
            hop_count: 0,
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

/// StateGraph-friendly message state.
///
/// This mirrors the LangGraph-style pattern where state stores a `messages` key
/// and nodes append one-or-more messages during execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct MessageState {
    #[serde(default)]
    pub messages: Vec<MessageEnvelope>,
    #[serde(flatten)]
    pub data: serde_json::Map<String, Value>,
}

impl MessageState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_messages(messages: Vec<MessageEnvelope>) -> Self {
        Self {
            messages,
            data: serde_json::Map::new(),
        }
    }

    pub fn push_message(&mut self, message: MessageEnvelope) {
        self.messages.push(message);
    }

    pub fn messages(&self) -> &[MessageEnvelope] {
        &self.messages
    }

    pub fn with_value(mut self, key: impl Into<String>, value: Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }
}

/// Build a `StateUpdate` for appending one message to the `messages` key.
pub fn single_message_update(message: &MessageEnvelope) -> AgentResult<StateUpdate> {
    StateUpdate::from_serializable(MESSAGES_KEY, message)
}

/// Build a `StateUpdate` for appending multiple messages to the `messages` key.
pub fn messages_update(messages: &[MessageEnvelope]) -> AgentResult<StateUpdate> {
    StateUpdate::from_serializable(MESSAGES_KEY, &messages.to_vec())
}

#[async_trait]
impl GraphState for MessageState {
    async fn apply_update<V: Serialize + Send + Sync + 'static>(
        &mut self,
        key: &str,
        value: V,
    ) -> AgentResult<()> {
        if key != MESSAGES_KEY {
            let json_value = serde_json::to_value(&value).map_err(|e| {
                AgentError::InvalidInput(format!("Failed to serialize value for key '{key}': {e}"))
            })?;
            self.data.insert(key.to_string(), json_value);
            return Ok(());
        }

        let json_value = serde_json::to_value(&value).map_err(|e| {
            AgentError::InvalidInput(format!("Failed to serialize messages update: {e}"))
        })?;

        match json_value {
            Value::Null => Ok(()),
            Value::Array(items) => {
                let mut parsed = Vec::with_capacity(items.len());
                for item in items {
                    parsed.push(
                        serde_json::from_value::<MessageEnvelope>(item).map_err(|e| {
                            AgentError::InvalidInput(format!(
                                "`messages` update contains invalid message envelope: {e}"
                            ))
                        })?,
                    );
                }
                self.messages.extend(parsed);
                Ok(())
            }
            Value::Object(_) => {
                let msg = serde_json::from_value::<MessageEnvelope>(json_value).map_err(|e| {
                    AgentError::InvalidInput(format!(
                        "`messages` update contains invalid message envelope: {e}"
                    ))
                })?;
                self.messages.push(msg);
                Ok(())
            }
            other => Err(AgentError::InvalidInput(format!(
                "`messages` update must be an envelope object or array, got: {other}"
            ))),
        }
    }

    fn get_value<V: DeserializeOwned + Send + Sync + 'static>(&self, key: &str) -> Option<V> {
        let json_value = if key == MESSAGES_KEY {
            serde_json::to_value(&self.messages).ok()?
        } else {
            self.data.get(key).cloned()?
        };
        serde_json::from_value(json_value).ok()
    }

    fn keys(&self) -> Vec<&str> {
        let mut keys = vec![MESSAGES_KEY];
        keys.extend(
            self.data
                .keys()
                .filter(|k| k.as_str() != MESSAGES_KEY)
                .map(|k| k.as_str()),
        );
        keys
    }

    fn to_json(&self) -> AgentResult<Value> {
        let mut map = self.data.clone();
        map.insert(
            MESSAGES_KEY.to_string(),
            serde_json::to_value(&self.messages)?,
        );
        Ok(Value::Object(map))
    }

    fn from_json(value: Value) -> AgentResult<Self> {
        let Value::Object(mut map) = value else {
            return Err(AgentError::InvalidInput(
                "MessageState must be a JSON object".to_string(),
            ));
        };

        let messages = match map.remove(MESSAGES_KEY) {
            Some(Value::Array(items)) => {
                let mut parsed = Vec::with_capacity(items.len());
                for item in items {
                    parsed.push(
                        serde_json::from_value::<MessageEnvelope>(item).map_err(|e| {
                            AgentError::InvalidInput(format!(
                                "MessageState `messages` field contains invalid envelope: {e}"
                            ))
                        })?,
                    );
                }
                parsed
            }
            Some(Value::Null) | None => Vec::new(),
            Some(other) => {
                return Err(AgentError::InvalidInput(format!(
                    "MessageState `messages` field must be an array, got: {other}"
                )));
            }
        };

        Ok(Self {
            messages,
            data: map,
        })
    }
}

/// Node kind in MessageGraph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageNodeKind {
    Agent { agent_id: String },
    Topic { topic: String },
    Stream { stream_id: String },
    Router,
}

/// MessageGraph node definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageNode {
    pub kind: MessageNodeKind,
    pub description: Option<String>,
}

impl MessageNode {
    pub fn new(kind: MessageNodeKind) -> Self {
        Self {
            kind,
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Route-matching rule for an edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RouteRule {
    Always,
    MessageType(String),
    HeaderEquals { key: String, value: String },
}

impl RouteRule {
    pub fn matches(&self, envelope: &MessageEnvelope) -> bool {
        match self {
            Self::Always => true,
            Self::MessageType(t) => &envelope.message_type == t,
            Self::HeaderEquals { key, value } => envelope.headers.get(key) == Some(value),
        }
    }

    fn validate(&self) -> Result<(), MessageGraphError> {
        match self {
            Self::Always => Ok(()),
            Self::MessageType(t) if t.trim().is_empty() => Err(
                MessageGraphError::InvalidRouteRule("message type cannot be empty".to_string()),
            ),
            Self::HeaderEquals { key, .. } if key.trim().is_empty() => Err(
                MessageGraphError::InvalidRouteRule("header key cannot be empty".to_string()),
            ),
            _ => Ok(()),
        }
    }
}

/// Delivery mode for an edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeliveryMode {
    Direct,
    Broadcast,
    PubSub,
}

/// Retry policy for delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RetryPolicy {
    pub max_retries: u8,
    pub backoff_ms: u64,
}

/// Delivery policy for an edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryPolicy {
    pub mode: DeliveryMode,
    pub retry: RetryPolicy,
}

impl Default for DeliveryPolicy {
    fn default() -> Self {
        Self {
            mode: DeliveryMode::Direct,
            retry: RetryPolicy::default(),
        }
    }
}

/// Directed edge in MessageGraph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageEdge {
    pub from: String,
    pub to: String,
    pub route: RouteRule,
    pub delivery: DeliveryPolicy,
}

impl MessageEdge {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        route: RouteRule,
        delivery: DeliveryPolicy,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            route,
            delivery,
        }
    }
}

/// Uncompiled MessageGraph definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageGraph {
    pub id: String,
    pub max_hops: u16,
    pub nodes: HashMap<String, MessageNode>,
    pub edges: Vec<MessageEdge>,
    pub entry_points: Vec<String>,
    pub dead_letter_node: Option<String>,
}

impl MessageGraph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            max_hops: 32,
            nodes: HashMap::new(),
            edges: Vec::new(),
            entry_points: Vec::new(),
            dead_letter_node: None,
        }
    }

    pub fn with_max_hops(mut self, max_hops: u16) -> Self {
        self.max_hops = max_hops;
        self
    }

    pub fn add_node(
        &mut self,
        node_id: impl Into<String>,
        node: MessageNode,
    ) -> Result<&mut Self, MessageGraphError> {
        let node_id = node_id.into();
        if node_id.trim().is_empty() {
            return Err(MessageGraphError::EmptyNodeId);
        }
        if self.nodes.contains_key(&node_id) {
            return Err(MessageGraphError::DuplicateNode(node_id));
        }
        self.nodes.insert(node_id, node);
        Ok(self)
    }

    pub fn add_entry_point(
        &mut self,
        node_id: impl Into<String>,
    ) -> Result<&mut Self, MessageGraphError> {
        let node_id = node_id.into();
        if !self.nodes.contains_key(&node_id) {
            return Err(MessageGraphError::MissingNode(node_id));
        }
        if self.entry_points.contains(&node_id) {
            return Err(MessageGraphError::DuplicateEntryPoint(node_id));
        }
        self.entry_points.push(node_id);
        Ok(self)
    }

    pub fn set_dead_letter_node(
        &mut self,
        node_id: impl Into<String>,
    ) -> Result<&mut Self, MessageGraphError> {
        let node_id = node_id.into();
        if !self.nodes.contains_key(&node_id) {
            return Err(MessageGraphError::MissingNode(node_id));
        }
        self.dead_letter_node = Some(node_id);
        Ok(self)
    }

    pub fn add_edge(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        route: RouteRule,
        delivery: DeliveryPolicy,
    ) -> Result<&mut Self, MessageGraphError> {
        let edge = MessageEdge::new(from, to, route, delivery);
        if self
            .edges
            .iter()
            .any(|e| e.from == edge.from && e.to == edge.to && e.route == edge.route)
        {
            return Err(MessageGraphError::DuplicateEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
            });
        }
        self.edges.push(edge);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), MessageGraphError> {
        if self.id.trim().is_empty() {
            return Err(MessageGraphError::EmptyGraphId);
        }
        if self.max_hops == 0 {
            return Err(MessageGraphError::InvalidMaxHops(self.max_hops));
        }
        if self.nodes.is_empty() {
            return Err(MessageGraphError::NoNodes);
        }
        if self.edges.is_empty() {
            return Err(MessageGraphError::NoEdges);
        }
        if self.entry_points.is_empty() {
            return Err(MessageGraphError::NoEntryPoints);
        }

        for entry in &self.entry_points {
            if !self.nodes.contains_key(entry) {
                return Err(MessageGraphError::MissingNode(entry.clone()));
            }
        }

        if let Some(dead_letter) = &self.dead_letter_node
            && !self.nodes.contains_key(dead_letter)
        {
            return Err(MessageGraphError::MissingNode(dead_letter.clone()));
        }

        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.from) {
                return Err(MessageGraphError::MissingNode(edge.from.clone()));
            }
            if !self.nodes.contains_key(&edge.to) {
                return Err(MessageGraphError::MissingNode(edge.to.clone()));
            }
            edge.route.validate()?;
        }

        let reachable = self.compute_reachable_nodes();
        let mut unreachable = self
            .nodes
            .keys()
            .filter(|node| !reachable.contains(*node))
            .cloned()
            .collect::<Vec<_>>();

        if let Some(dead_letter) = &self.dead_letter_node {
            unreachable.retain(|node| node != dead_letter);
        }

        if !unreachable.is_empty() {
            unreachable.sort();
            return Err(MessageGraphError::UnreachableNodes(unreachable));
        }

        Ok(())
    }

    pub fn compile(self) -> Result<CompiledMessageGraph, MessageGraphError> {
        self.validate()?;
        Ok(CompiledMessageGraph::from_graph(self))
    }

    fn compute_reachable_nodes(&self) -> HashSet<String> {
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        for edge in &self.edges {
            adjacency
                .entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        for entry in &self.entry_points {
            queue.push_back(entry.clone());
        }

        while let Some(node) = queue.pop_front() {
            if !visited.insert(node.clone()) {
                continue;
            }

            if let Some(next_nodes) = adjacency.get(&node) {
                for next in next_nodes {
                    queue.push_back(next.clone());
                }
            }
        }

        visited
    }
}

/// Compiled MessageGraph with adjacency index for runtime lookups.
#[derive(Debug, Clone)]
pub struct CompiledMessageGraph {
    pub id: String,
    pub max_hops: u16,
    pub nodes: HashMap<String, MessageNode>,
    pub entry_points: Vec<String>,
    pub dead_letter_node: Option<String>,
    adjacency: HashMap<String, Vec<MessageEdge>>,
}

impl CompiledMessageGraph {
    fn from_graph(graph: MessageGraph) -> Self {
        let mut adjacency: HashMap<String, Vec<MessageEdge>> = HashMap::new();
        for edge in graph.edges {
            adjacency.entry(edge.from.clone()).or_default().push(edge);
        }

        Self {
            id: graph.id,
            max_hops: graph.max_hops,
            nodes: graph.nodes,
            entry_points: graph.entry_points,
            dead_letter_node: graph.dead_letter_node,
            adjacency,
        }
    }

    pub fn next_edges<'a>(
        &'a self,
        node_id: &str,
        envelope: &MessageEnvelope,
    ) -> Result<Vec<&'a MessageEdge>, MessageGraphError> {
        if !self.nodes.contains_key(node_id) {
            return Err(MessageGraphError::MissingNode(node_id.to_string()));
        }

        let edges = self
            .adjacency
            .get(node_id)
            .map(|outgoing| {
                outgoing
                    .iter()
                    .filter(|edge| edge.route.matches(envelope))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(edges)
    }

    pub fn has_outgoing_edges(&self, node_id: &str) -> Result<bool, MessageGraphError> {
        if !self.nodes.contains_key(node_id) {
            return Err(MessageGraphError::MissingNode(node_id.to_string()));
        }
        Ok(self
            .adjacency
            .get(node_id)
            .map(|outgoing| !outgoing.is_empty())
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use serde_json::json;

    fn sample_envelope(kind: &str) -> MessageEnvelope {
        MessageEnvelope::new(kind, br#"{"sample":true}"#.to_vec()).with_header("k", "v")
    }

    #[test]
    fn message_state_uses_messages_key() {
        let state = MessageState::from_messages(vec![sample_envelope("m1")])
            .with_value("foo", json!(1))
            .with_value("bar", json!("x"));

        assert!(state.keys().contains(&MESSAGES_KEY));
        assert!(state.keys().contains(&"foo"));
        assert_eq!(state.messages().len(), 1);
    }

    #[test]
    fn message_state_applies_single_and_batch_updates() {
        let mut state = MessageState::new();
        let first = sample_envelope("a");
        let second = sample_envelope("b");

        block_on(state.apply_update(MESSAGES_KEY, serde_json::to_value(&first).unwrap())).unwrap();
        block_on(state.apply_update(
            MESSAGES_KEY,
            serde_json::to_value(vec![second.clone()]).unwrap(),
        ))
        .unwrap();

        assert_eq!(state.messages().len(), 2);
        assert_eq!(state.messages()[0].message_type, "a");
        assert_eq!(state.messages()[1].message_type, "b");
    }

    #[test]
    fn message_state_rejects_invalid_messages_updates() {
        let mut state = MessageState::new();
        let err = block_on(state.apply_update(MESSAGES_KEY, json!("not-an-envelope"))).unwrap_err();
        assert!(
            err.to_string()
                .contains("must be an envelope object or array")
        );
    }

    #[test]
    fn message_state_json_roundtrip_preserves_messages() {
        let state = MessageState::from_messages(vec![sample_envelope("m1"), sample_envelope("m2")])
            .with_value("tenant", json!("acme"));

        let json = state.to_json().unwrap();
        let restored = MessageState::from_json(json).unwrap();

        assert_eq!(restored.messages().len(), 2);
        assert_eq!(restored.get_value("tenant"), Some(json!("acme")));
    }

    #[test]
    fn message_update_helpers_target_messages_key() {
        let update = single_message_update(&sample_envelope("m")).unwrap();
        assert_eq!(update.key, MESSAGES_KEY);

        let batch = messages_update(&[sample_envelope("a"), sample_envelope("b")]).unwrap();
        assert_eq!(batch.key, MESSAGES_KEY);
    }

    fn build_valid_graph() -> MessageGraph {
        let mut graph = MessageGraph::new("task19_message_graph");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "orders.in".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "router",
                MessageNode::new(MessageNodeKind::Router).with_description("routes by priority"),
            )
            .unwrap();
        graph
            .add_node(
                "critical_agent",
                MessageNode::new(MessageNodeKind::Agent {
                    agent_id: "critical_worker".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "normal_agent",
                MessageNode::new(MessageNodeKind::Agent {
                    agent_id: "normal_worker".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "dlq",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "orders.dlq".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("entry").unwrap();
        graph.set_dead_letter_node("dlq").unwrap();
        graph
            .add_edge(
                "entry",
                "router",
                RouteRule::Always,
                DeliveryPolicy::default(),
            )
            .unwrap();
        graph
            .add_edge(
                "router",
                "critical_agent",
                RouteRule::HeaderEquals {
                    key: "priority".to_string(),
                    value: "critical".to_string(),
                },
                DeliveryPolicy::default(),
            )
            .unwrap();
        graph
            .add_edge(
                "router",
                "normal_agent",
                RouteRule::MessageType("task".to_string()),
                DeliveryPolicy::default(),
            )
            .unwrap();

        graph
    }

    #[test]
    fn compile_succeeds_for_valid_graph() {
        let graph = build_valid_graph();
        let compiled = graph.compile().unwrap();
        assert_eq!(compiled.id, "task19_message_graph");
        assert_eq!(compiled.max_hops, 32);
    }

    #[test]
    fn compile_fails_when_graph_id_is_empty() {
        let mut graph = MessageGraph::new("");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.in".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "next",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.out".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("entry").unwrap();
        graph
            .add_edge(
                "entry",
                "next",
                RouteRule::Always,
                DeliveryPolicy::default(),
            )
            .unwrap();

        let err = graph.compile().unwrap_err();
        assert_eq!(err, MessageGraphError::EmptyGraphId);
    }

    #[test]
    fn compile_fails_when_no_nodes_exist() {
        let graph = MessageGraph::new("no_nodes");
        let err = graph.compile().unwrap_err();
        assert_eq!(err, MessageGraphError::NoNodes);
    }

    #[test]
    fn compile_fails_when_no_edges_exist() {
        let mut graph = MessageGraph::new("no_edges");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.in".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("entry").unwrap();

        let err = graph.compile().unwrap_err();
        assert_eq!(err, MessageGraphError::NoEdges);
    }

    #[test]
    fn add_node_fails_when_node_id_is_empty() {
        let mut graph = MessageGraph::new("g");
        let err = graph
            .add_node(
                "",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic".to_string(),
                }),
            )
            .unwrap_err();
        assert_eq!(err, MessageGraphError::EmptyNodeId);
    }

    #[test]
    fn add_node_fails_when_node_is_duplicate() {
        let mut graph = MessageGraph::new("g");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic".to_string(),
                }),
            )
            .unwrap();

        let err = graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic2".to_string(),
                }),
            )
            .unwrap_err();
        assert_eq!(err, MessageGraphError::DuplicateNode("entry".to_string()));
    }

    #[test]
    fn add_entry_point_fails_when_duplicate() {
        let mut graph = MessageGraph::new("g");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("entry").unwrap();

        let err = graph.add_entry_point("entry").unwrap_err();
        assert_eq!(
            err,
            MessageGraphError::DuplicateEntryPoint("entry".to_string())
        );
    }

    #[test]
    fn add_edge_fails_when_duplicate() {
        let mut graph = MessageGraph::new("g");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.in".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "next",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.out".to_string(),
                }),
            )
            .unwrap();

        graph
            .add_edge(
                "entry",
                "next",
                RouteRule::Always,
                DeliveryPolicy::default(),
            )
            .unwrap();

        let err = graph
            .add_edge(
                "entry",
                "next",
                RouteRule::Always,
                DeliveryPolicy::default(),
            )
            .unwrap_err();
        assert_eq!(
            err,
            MessageGraphError::DuplicateEdge {
                from: "entry".to_string(),
                to: "next".to_string(),
            }
        );
    }

    #[test]
    fn compile_fails_without_entry_points() {
        let mut graph = MessageGraph::new("g");
        graph
            .add_node(
                "n1",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node("n2", MessageNode::new(MessageNodeKind::Router))
            .unwrap();
        graph
            .add_edge("n1", "n2", RouteRule::Always, DeliveryPolicy::default())
            .unwrap();

        let err = graph.compile().unwrap_err();
        assert_eq!(err, MessageGraphError::NoEntryPoints);
    }

    #[test]
    fn compile_fails_with_unreachable_node() {
        let mut graph = build_valid_graph();
        graph
            .add_node(
                "orphan",
                MessageNode::new(MessageNodeKind::Agent {
                    agent_id: "unused".to_string(),
                }),
            )
            .unwrap();

        let err = graph.compile().unwrap_err();
        assert_eq!(
            err,
            MessageGraphError::UnreachableNodes(vec!["orphan".to_string()])
        );
    }

    #[test]
    fn compile_fails_when_max_hops_is_zero() {
        let graph = build_valid_graph().with_max_hops(0);
        let err = graph.compile().unwrap_err();
        assert_eq!(err, MessageGraphError::InvalidMaxHops(0));
    }

    #[test]
    fn compile_fails_when_edge_target_missing() {
        let mut graph = MessageGraph::new("missing_edge_target");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.in".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("entry").unwrap();
        graph
            .add_edge(
                "entry",
                "missing",
                RouteRule::Always,
                DeliveryPolicy::default(),
            )
            .unwrap();

        let err = graph.compile().unwrap_err();
        assert_eq!(err, MessageGraphError::MissingNode("missing".to_string()));
    }

    #[test]
    fn compile_fails_when_route_rule_is_invalid() {
        let mut graph = MessageGraph::new("invalid_route_rule");
        graph
            .add_node(
                "entry",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.in".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "next",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "topic.out".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("entry").unwrap();
        graph
            .add_edge(
                "entry",
                "next",
                RouteRule::MessageType("".to_string()),
                DeliveryPolicy::default(),
            )
            .unwrap();

        let err = graph.compile().unwrap_err();
        assert_eq!(
            err,
            MessageGraphError::InvalidRouteRule("message type cannot be empty".to_string())
        );
    }

    #[test]
    fn route_rule_filters_edges() {
        let graph = build_valid_graph();
        let compiled = graph.compile().unwrap();

        let envelope = MessageEnvelope::new("task", vec![]).with_header("priority", "critical");
        let edges = compiled.next_edges("router", &envelope).unwrap();

        assert_eq!(edges.len(), 2);
        assert!(edges.iter().any(|e| e.to == "critical_agent"));
        assert!(edges.iter().any(|e| e.to == "normal_agent"));
    }

    #[test]
    fn next_edges_returns_error_for_unknown_node() {
        let graph = build_valid_graph();
        let compiled = graph.compile().unwrap();
        let envelope = MessageEnvelope::new("task", vec![]);

        let err = compiled.next_edges("unknown", &envelope).unwrap_err();
        assert_eq!(err, MessageGraphError::MissingNode("unknown".to_string()));
    }
}
