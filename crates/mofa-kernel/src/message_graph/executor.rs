use super::{
    CompiledMessageGraph, DeliveryMode, MessageEnvelope, MessageGraphError, MessageNodeKind,
};
use crate::bus::{AgentBus, BusError, CommunicationMode};
use crate::message::{AgentEvent, AgentMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};
use tokio::task::JoinSet;

/// Runtime configuration for [`MessageGraphExecutor`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageGraphExecutorConfig {
    /// Sender id used for kernel bus sends.
    pub sender_id: String,
    /// Default in-flight capacity applied to each node.
    pub default_node_capacity: usize,
}

impl Default for MessageGraphExecutorConfig {
    fn default() -> Self {
        Self {
            sender_id: "message_graph_executor".to_string(),
            default_node_capacity: 64,
        }
    }
}

/// Why a message entered dead-letter handling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeadLetterReason {
    NoRouteMatch,
    MaxHopsExceeded { max_hops: u16, attempted_hops: u16 },
    Backpressure,
    DispatchFailed(String),
}

impl DeadLetterReason {
    fn as_header_value(&self) -> String {
        match self {
            Self::NoRouteMatch => "no_route_match".to_string(),
            Self::MaxHopsExceeded {
                max_hops,
                attempted_hops,
            } => {
                format!("max_hops_exceeded:{attempted_hops}>{max_hops}")
            }
            Self::Backpressure => "node_backpressure".to_string(),
            Self::DispatchFailed(msg) => format!("dispatch_failed:{msg}"),
        }
    }
}

/// A successful route traversal from one node to another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageDispatchRecord {
    pub from: String,
    pub to: String,
    pub hop_count: u16,
    pub delivery_mode: DeliveryMode,
    /// `false` for internal-only hops (for example, router -> router) where no bus send occurs.
    pub delivered_to_bus: bool,
}

/// A dead-letter record produced during execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeadLetterRecord {
    pub from: String,
    pub dead_letter_node: String,
    pub reason: DeadLetterReason,
    pub envelope: MessageEnvelope,
    pub delivered: bool,
    pub delivery_error: Option<String>,
}

/// Final runtime report for a message execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageGraphExecutionReport {
    pub graph_id: String,
    #[serde(default)]
    pub dispatches: Vec<MessageDispatchRecord>,
    #[serde(default)]
    pub dead_letters: Vec<DeadLetterRecord>,
}

impl MessageGraphExecutionReport {
    pub fn total_dispatched(&self) -> usize {
        self.dispatches.len()
    }

    pub fn total_dead_letters(&self) -> usize {
        self.dead_letters.len()
    }
}

/// Runtime errors for message graph execution.
#[derive(Debug, thiserror::Error)]
pub enum MessageGraphExecutorError {
    #[error(transparent)]
    Graph(#[from] MessageGraphError),
    #[error(transparent)]
    Bus(#[from] BusError),
    #[error("message graph '{graph_id}' has no entry points")]
    NoEntryPoints { graph_id: String },
    #[error("message graph '{graph_id}' does not define a dead-letter node")]
    MissingDeadLetterNode { graph_id: String },
    #[error("node '{node_id}' is backpressured")]
    NodeBackpressured { node_id: String },
    #[error("backpressure is not configurable for router node '{node_id}'")]
    RouterCapacityUnsupported { node_id: String },
    #[error("cannot update node '{node_id}' capacity while permits are outstanding")]
    CapacityUpdateInUse { node_id: String },
    #[error("failed to serialize envelope for node '{node_id}': {source}")]
    EnvelopeSerialization {
        node_id: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("executor task join failed: {0}")]
    TaskJoin(String),
}

#[derive(Debug, Clone)]
struct NodeLimit {
    semaphore: Arc<Semaphore>,
    capacity: usize,
}

#[derive(Debug, Clone)]
struct PendingRoute {
    node_id: String,
    envelope: MessageEnvelope,
}

#[derive(Debug, Default)]
struct RouteOutcome {
    next_routes: Vec<PendingRoute>,
    dispatches: Vec<MessageDispatchRecord>,
    dead_letters: Vec<DeadLetterRecord>,
}

#[derive(Debug, Default)]
struct EdgeOutcome {
    next_route: Option<PendingRoute>,
    dispatch: Option<MessageDispatchRecord>,
    dead_letter: Option<DeadLetterRecord>,
}

/// Runtime engine that routes [`MessageEnvelope`] values through a compiled message graph.
#[derive(Clone)]
pub struct MessageGraphExecutor {
    graph: Arc<CompiledMessageGraph>,
    bus: Arc<AgentBus>,
    config: MessageGraphExecutorConfig,
    node_limits: Arc<RwLock<HashMap<String, NodeLimit>>>,
}

impl MessageGraphExecutor {
    pub fn new(
        graph: CompiledMessageGraph,
        bus: Arc<AgentBus>,
    ) -> Result<Self, MessageGraphExecutorError> {
        Self::with_config(graph, bus, MessageGraphExecutorConfig::default())
    }

    pub fn with_config(
        graph: CompiledMessageGraph,
        bus: Arc<AgentBus>,
        config: MessageGraphExecutorConfig,
    ) -> Result<Self, MessageGraphExecutorError> {
        if graph.entry_points.is_empty() {
            return Err(MessageGraphExecutorError::NoEntryPoints {
                graph_id: graph.id.clone(),
            });
        }
        if graph.dead_letter_node.is_none() {
            return Err(MessageGraphExecutorError::MissingDeadLetterNode {
                graph_id: graph.id.clone(),
            });
        }

        let mut node_limits = HashMap::new();
        for (node_id, node) in &graph.nodes {
            if matches!(node.kind, MessageNodeKind::Router) {
                continue;
            }
            node_limits.insert(
                node_id.clone(),
                NodeLimit {
                    semaphore: Arc::new(Semaphore::new(config.default_node_capacity)),
                    capacity: config.default_node_capacity,
                },
            );
        }

        Ok(Self {
            graph: Arc::new(graph),
            bus,
            config,
            node_limits: Arc::new(RwLock::new(node_limits)),
        })
    }

    pub async fn set_node_capacity(
        &self,
        node_id: impl AsRef<str>,
        capacity: usize,
    ) -> Result<(), MessageGraphExecutorError> {
        let node_id = node_id.as_ref();
        let node = self
            .graph
            .nodes
            .get(node_id)
            .ok_or_else(|| MessageGraphError::MissingNode(node_id.to_string()))?;
        if matches!(node.kind, MessageNodeKind::Router) {
            return Err(MessageGraphExecutorError::RouterCapacityUnsupported {
                node_id: node_id.to_string(),
            });
        }
        let mut guard = self.node_limits.write().await;
        if let Some(current) = guard.get(node_id)
            && current.semaphore.available_permits() != current.capacity
        {
            return Err(MessageGraphExecutorError::CapacityUpdateInUse {
                node_id: node_id.to_string(),
            });
        }
        guard.insert(
            node_id.to_string(),
            NodeLimit {
                semaphore: Arc::new(Semaphore::new(capacity)),
                capacity,
            },
        );
        Ok(())
    }

    pub async fn execute(
        &self,
        envelope: MessageEnvelope,
    ) -> Result<MessageGraphExecutionReport, MessageGraphExecutorError> {
        let mut report = MessageGraphExecutionReport {
            graph_id: self.graph.id.clone(),
            dispatches: Vec::new(),
            dead_letters: Vec::new(),
        };

        let mut frontier = self
            .graph
            .entry_points
            .iter()
            .cloned()
            .map(|node_id| PendingRoute {
                node_id,
                envelope: envelope.clone(),
            })
            .collect::<Vec<_>>();

        while !frontier.is_empty() {
            let mut join_set = JoinSet::new();
            for pending in frontier.drain(..) {
                let executor = self.clone();
                join_set.spawn(async move { executor.route_from_node(pending).await });
            }

            let mut next_frontier = Vec::new();
            while let Some(joined) = join_set.join_next().await {
                let outcome =
                    joined.map_err(|e| MessageGraphExecutorError::TaskJoin(e.to_string()))??;
                report.dispatches.extend(outcome.dispatches);
                report.dead_letters.extend(outcome.dead_letters);
                next_frontier.extend(outcome.next_routes);
            }

            frontier = next_frontier;
        }

        Ok(report)
    }

    async fn route_from_node(
        &self,
        pending: PendingRoute,
    ) -> Result<RouteOutcome, MessageGraphExecutorError> {
        let mut outcome = RouteOutcome::default();
        let matched_edges = self
            .graph
            .next_edges(&pending.node_id, &pending.envelope)?
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        if matched_edges.is_empty() {
            // Leaf nodes naturally terminate without dead-lettering.
            if !self.graph.has_outgoing_edges(&pending.node_id)? {
                return Ok(outcome);
            }

            let dead_letter = self
                .route_to_dead_letter(
                    &pending.node_id,
                    pending.envelope,
                    DeadLetterReason::NoRouteMatch,
                )
                .await?;
            outcome.dead_letters.push(dead_letter);
            return Ok(outcome);
        }

        let mut edge_join_set = JoinSet::new();
        for edge in matched_edges {
            let executor = self.clone();
            let envelope = pending.envelope.clone();
            edge_join_set.spawn(async move { executor.process_edge(edge, envelope).await });
        }

        while let Some(joined) = edge_join_set.join_next().await {
            let edge_outcome =
                joined.map_err(|e| MessageGraphExecutorError::TaskJoin(e.to_string()))??;
            if let Some(next_route) = edge_outcome.next_route {
                outcome.next_routes.push(next_route);
            }
            if let Some(dispatch) = edge_outcome.dispatch {
                outcome.dispatches.push(dispatch);
            }
            if let Some(dead_letter) = edge_outcome.dead_letter {
                outcome.dead_letters.push(dead_letter);
            }
        }

        Ok(outcome)
    }

    async fn process_edge(
        &self,
        edge: super::MessageEdge,
        envelope: MessageEnvelope,
    ) -> Result<EdgeOutcome, MessageGraphExecutorError> {
        let mut next_envelope = envelope;
        next_envelope.hop_count = next_envelope.hop_count.saturating_add(1);

        if next_envelope.hop_count > self.graph.max_hops {
            let attempted_hops = next_envelope.hop_count;
            let dead_letter = self
                .route_to_dead_letter(
                    &edge.from,
                    next_envelope,
                    DeadLetterReason::MaxHopsExceeded {
                        max_hops: self.graph.max_hops,
                        attempted_hops,
                    },
                )
                .await?;
            return Ok(EdgeOutcome {
                dead_letter: Some(dead_letter),
                ..EdgeOutcome::default()
            });
        }

        match self
            .dispatch_to_node(&edge.to, &edge.delivery.mode, &next_envelope)
            .await
        {
            Ok(delivered_to_bus) => Ok(EdgeOutcome {
                next_route: Some(PendingRoute {
                    node_id: edge.to.clone(),
                    envelope: next_envelope.clone(),
                }),
                dispatch: Some(MessageDispatchRecord {
                    from: edge.from,
                    to: edge.to,
                    hop_count: next_envelope.hop_count,
                    delivery_mode: edge.delivery.mode,
                    delivered_to_bus,
                }),
                dead_letter: None,
            }),
            Err(MessageGraphExecutorError::NodeBackpressured { .. }) => {
                let dead_letter = self
                    .route_to_dead_letter(&edge.from, next_envelope, DeadLetterReason::Backpressure)
                    .await?;
                Ok(EdgeOutcome {
                    dead_letter: Some(dead_letter),
                    ..EdgeOutcome::default()
                })
            }
            Err(MessageGraphExecutorError::Bus(err)) => {
                let dead_letter = self
                    .route_to_dead_letter(
                        &edge.from,
                        next_envelope,
                        DeadLetterReason::DispatchFailed(err.to_string()),
                    )
                    .await?;
                Ok(EdgeOutcome {
                    dead_letter: Some(dead_letter),
                    ..EdgeOutcome::default()
                })
            }
            Err(other) => Err(other),
        }
    }

    async fn route_to_dead_letter(
        &self,
        from: &str,
        mut envelope: MessageEnvelope,
        reason: DeadLetterReason,
    ) -> Result<DeadLetterRecord, MessageGraphExecutorError> {
        let dead_letter_node = self.graph.dead_letter_node.clone().ok_or_else(|| {
            MessageGraphExecutorError::MissingDeadLetterNode {
                graph_id: self.graph.id.clone(),
            }
        })?;

        envelope.headers.insert(
            "x-mofa-dead-letter-reason".to_string(),
            reason.as_header_value(),
        );
        envelope
            .headers
            .insert("x-mofa-dead-letter-from".to_string(), from.to_string());

        let (delivered, delivery_error) = if dead_letter_node == from {
            (
                false,
                Some("dead-letter source is dead-letter node".to_string()),
            )
        } else {
            match self
                .dispatch_to_node(&dead_letter_node, &DeliveryMode::Direct, &envelope)
                .await
            {
                Ok(_) => (true, None),
                Err(err) => (false, Some(err.to_string())),
            }
        };

        Ok(DeadLetterRecord {
            from: from.to_string(),
            dead_letter_node,
            reason,
            envelope,
            delivered,
            delivery_error,
        })
    }

    async fn dispatch_to_node(
        &self,
        node_id: &str,
        delivery_mode: &DeliveryMode,
        envelope: &MessageEnvelope,
    ) -> Result<bool, MessageGraphExecutorError> {
        let node = self
            .graph
            .nodes
            .get(node_id)
            .ok_or_else(|| MessageGraphError::MissingNode(node_id.to_string()))?;

        if matches!(node.kind, MessageNodeKind::Router) {
            return Ok(false);
        }

        let permit = self.acquire_node_permit(node_id).await?;
        let (mode, message) =
            self.build_bus_dispatch(node_id, &node.kind, delivery_mode, envelope)?;
        self.bus
            .send_message(&self.config.sender_id, mode, &message)
            .await?;
        drop(permit);
        Ok(true)
    }

    fn build_bus_dispatch(
        &self,
        node_id: &str,
        kind: &MessageNodeKind,
        delivery_mode: &DeliveryMode,
        envelope: &MessageEnvelope,
    ) -> Result<(CommunicationMode, AgentMessage), MessageGraphExecutorError> {
        let mode = match delivery_mode {
            DeliveryMode::Broadcast => CommunicationMode::Broadcast,
            DeliveryMode::Direct => match kind {
                MessageNodeKind::Agent { agent_id } => {
                    CommunicationMode::PointToPoint(agent_id.clone())
                }
                MessageNodeKind::Topic { topic } => CommunicationMode::PubSub(topic.clone()),
                MessageNodeKind::Stream { stream_id } => {
                    CommunicationMode::PubSub(stream_id.clone())
                }
                MessageNodeKind::Router => unreachable!("router dispatch is filtered before send"),
            },
            DeliveryMode::PubSub => match kind {
                MessageNodeKind::Agent { agent_id } => CommunicationMode::PubSub(agent_id.clone()),
                MessageNodeKind::Topic { topic } => CommunicationMode::PubSub(topic.clone()),
                MessageNodeKind::Stream { stream_id } => {
                    CommunicationMode::PubSub(stream_id.clone())
                }
                MessageNodeKind::Router => unreachable!("router dispatch is filtered before send"),
            },
        };

        let message = match kind {
            MessageNodeKind::Stream { stream_id } => AgentMessage::StreamMessage {
                stream_id: stream_id.clone(),
                message: envelope.payload.clone(),
                sequence: envelope.hop_count as u64,
            },
            _ => {
                let payload = serde_json::to_vec(envelope).map_err(|source| {
                    MessageGraphExecutorError::EnvelopeSerialization {
                        node_id: node_id.to_string(),
                        source,
                    }
                })?;
                AgentMessage::Event(AgentEvent::Custom(envelope.message_type.clone(), payload))
            }
        };

        Ok((mode, message))
    }

    async fn acquire_node_permit(
        &self,
        node_id: &str,
    ) -> Result<OwnedSemaphorePermit, MessageGraphExecutorError> {
        let semaphore = {
            let guard = self.node_limits.read().await;
            guard
                .get(node_id)
                .map(|entry| entry.semaphore.clone())
                .ok_or_else(|| MessageGraphError::MissingNode(node_id.to_string()))?
        };
        semaphore
            .try_acquire_owned()
            .map_err(|_| MessageGraphExecutorError::NodeBackpressured {
                node_id: node_id.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentCapabilities, AgentMetadata, AgentState};
    use crate::bus::CommunicationMode;
    use crate::message::AgentMessage;
    use crate::message_graph::{
        DeliveryPolicy, MessageGraph, MessageNode, MessageNodeKind, RouteRule,
    };
    use tokio::time::{Duration, timeout};

    fn test_agent_metadata(id: &str) -> AgentMetadata {
        AgentMetadata {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            version: None,
            capabilities: AgentCapabilities::default(),
            state: AgentState::Ready,
        }
    }

    fn build_runtime_graph() -> CompiledMessageGraph {
        let mut graph = MessageGraph::new("runtime-routing").with_max_hops(8);
        graph
            .add_node(
                "ingress",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "orders.in".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node("router", MessageNode::new(MessageNodeKind::Router))
            .unwrap();
        graph
            .add_node(
                "fraud_agent",
                MessageNode::new(MessageNodeKind::Agent {
                    agent_id: "fraud-worker".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "fulfillment_stream",
                MessageNode::new(MessageNodeKind::Stream {
                    stream_id: "orders.fulfillment".to_string(),
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
        graph.add_entry_point("ingress").unwrap();
        graph.set_dead_letter_node("dlq").unwrap();
        graph
            .add_edge(
                "ingress",
                "router",
                RouteRule::Always,
                DeliveryPolicy::default(),
            )
            .unwrap();
        graph
            .add_edge(
                "router",
                "fraud_agent",
                RouteRule::HeaderEquals {
                    key: "risk".to_string(),
                    value: "high".to_string(),
                },
                DeliveryPolicy::default(),
            )
            .unwrap();
        graph
            .add_edge(
                "router",
                "fulfillment_stream",
                RouteRule::MessageType("order.created".to_string()),
                DeliveryPolicy {
                    mode: DeliveryMode::PubSub,
                    ..DeliveryPolicy::default()
                },
            )
            .unwrap();
        graph.compile().unwrap()
    }

    fn build_cycle_graph() -> CompiledMessageGraph {
        let mut graph = MessageGraph::new("cycle").with_max_hops(2);
        graph
            .add_node("n1", MessageNode::new(MessageNodeKind::Router))
            .unwrap();
        graph
            .add_node("n2", MessageNode::new(MessageNodeKind::Router))
            .unwrap();
        graph
            .add_node(
                "dlq",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "cycle.dlq".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("n1").unwrap();
        graph.set_dead_letter_node("dlq").unwrap();
        graph
            .add_edge("n1", "n2", RouteRule::Always, DeliveryPolicy::default())
            .unwrap();
        graph
            .add_edge("n2", "n1", RouteRule::Always, DeliveryPolicy::default())
            .unwrap();
        graph.compile().unwrap()
    }

    fn decode_event_envelope(message: AgentMessage) -> MessageEnvelope {
        match message {
            AgentMessage::Event(AgentEvent::Custom(_, payload)) => {
                serde_json::from_slice(&payload).unwrap()
            }
            other => panic!("expected AgentEvent::Custom, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn executor_routes_messages_to_agent_and_stream_targets() {
        let bus = Arc::new(AgentBus::new());
        let compiled = build_runtime_graph();
        let executor = MessageGraphExecutor::new(compiled, bus.clone()).unwrap();

        bus.register_channel(
            &test_agent_metadata("fraud-worker"),
            CommunicationMode::PointToPoint("message_graph_executor".to_string()),
        )
        .await
        .unwrap();
        bus.register_channel(
            &test_agent_metadata("stream-consumer"),
            CommunicationMode::PubSub("orders.fulfillment".to_string()),
        )
        .await
        .unwrap();
        bus.register_channel(
            &test_agent_metadata("dlq-consumer"),
            CommunicationMode::PubSub("orders.dlq".to_string()),
        )
        .await
        .unwrap();

        let fraud_bus = bus.clone();
        let fraud_receiver = tokio::spawn(async move {
            fraud_bus
                .receive_message(
                    "fraud-worker",
                    CommunicationMode::PointToPoint("message_graph_executor".to_string()),
                )
                .await
        });
        let stream_bus = bus.clone();
        let stream_receiver = tokio::spawn(async move {
            stream_bus
                .receive_message(
                    "stream-consumer",
                    CommunicationMode::PubSub("orders.fulfillment".to_string()),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;

        let envelope = MessageEnvelope::new("order.created", br#"{"id":"A-100"}"#.to_vec())
            .with_header("risk", "high");

        let report = executor.execute(envelope.clone()).await.unwrap();
        assert_eq!(report.total_dead_letters(), 0);
        assert!(report.dispatches.iter().any(|d| d.to == "fraud_agent"));
        assert!(
            report
                .dispatches
                .iter()
                .any(|d| d.to == "fulfillment_stream")
        );

        let fraud_message = timeout(Duration::from_secs(1), fraud_receiver)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .unwrap();
        let routed_envelope = decode_event_envelope(fraud_message);
        assert_eq!(routed_envelope.message_type, envelope.message_type);
        assert_eq!(routed_envelope.hop_count, 2);

        let stream_message = timeout(Duration::from_secs(1), stream_receiver)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .unwrap();
        match stream_message {
            AgentMessage::StreamMessage {
                stream_id,
                message,
                sequence,
            } => {
                assert_eq!(stream_id, "orders.fulfillment");
                assert_eq!(message, envelope.payload);
                assert_eq!(sequence, 2);
            }
            other => panic!("expected stream message, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn executor_routes_unmatched_messages_to_dead_letter_queue() {
        let bus = Arc::new(AgentBus::new());
        let compiled = build_runtime_graph();
        let executor = MessageGraphExecutor::new(compiled, bus.clone()).unwrap();

        bus.register_channel(
            &test_agent_metadata("dlq-consumer"),
            CommunicationMode::PubSub("orders.dlq".to_string()),
        )
        .await
        .unwrap();

        let dlq_bus = bus.clone();
        let dlq_receiver = tokio::spawn(async move {
            dlq_bus
                .receive_message(
                    "dlq-consumer",
                    CommunicationMode::PubSub("orders.dlq".to_string()),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;

        let envelope = MessageEnvelope::new("order.cancelled", br#"{"id":"A-200"}"#.to_vec());
        let report = executor.execute(envelope).await.unwrap();
        assert_eq!(report.total_dead_letters(), 1);
        assert!(matches!(
            report.dead_letters[0].reason,
            DeadLetterReason::NoRouteMatch
        ));
        assert!(report.dead_letters[0].delivered);
        assert!(report.dead_letters[0].delivery_error.is_none());

        let dead_letter_message = timeout(Duration::from_secs(1), dlq_receiver)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .unwrap();
        let dead_letter_envelope = decode_event_envelope(dead_letter_message);
        assert_eq!(
            dead_letter_envelope
                .headers
                .get("x-mofa-dead-letter-reason")
                .map(String::as_str),
            Some("no_route_match")
        );
    }

    #[tokio::test]
    async fn executor_enforces_max_hops_and_dead_letters_the_message() {
        let bus = Arc::new(AgentBus::new());
        let compiled = build_cycle_graph();
        let executor = MessageGraphExecutor::new(compiled, bus.clone()).unwrap();

        bus.register_channel(
            &test_agent_metadata("dlq-consumer"),
            CommunicationMode::PubSub("cycle.dlq".to_string()),
        )
        .await
        .unwrap();

        let report = executor
            .execute(MessageEnvelope::new("loop", b"{}".to_vec()))
            .await
            .unwrap();
        assert_eq!(report.total_dead_letters(), 1);
        assert!(matches!(
            report.dead_letters[0].reason,
            DeadLetterReason::MaxHopsExceeded { .. }
        ));
    }

    #[tokio::test]
    async fn executor_applies_node_backpressure_and_uses_dead_letter_queue() {
        let bus = Arc::new(AgentBus::new());

        let mut graph = MessageGraph::new("backpressure");
        graph
            .add_node("entry", MessageNode::new(MessageNodeKind::Router))
            .unwrap();
        graph
            .add_node(
                "worker_node",
                MessageNode::new(MessageNodeKind::Agent {
                    agent_id: "worker-agent".to_string(),
                }),
            )
            .unwrap();
        graph
            .add_node(
                "dlq",
                MessageNode::new(MessageNodeKind::Topic {
                    topic: "backpressure.dlq".to_string(),
                }),
            )
            .unwrap();
        graph.add_entry_point("entry").unwrap();
        graph.set_dead_letter_node("dlq").unwrap();
        graph
            .add_edge(
                "entry",
                "worker_node",
                RouteRule::Always,
                DeliveryPolicy::default(),
            )
            .unwrap();

        let compiled = graph.compile().unwrap();
        let executor = MessageGraphExecutor::new(compiled, bus.clone()).unwrap();
        executor.set_node_capacity("worker_node", 0).await.unwrap();

        bus.register_channel(
            &test_agent_metadata("dlq-consumer"),
            CommunicationMode::PubSub("backpressure.dlq".to_string()),
        )
        .await
        .unwrap();

        let dlq_bus = bus.clone();
        let dlq_receiver = tokio::spawn(async move {
            dlq_bus
                .receive_message(
                    "dlq-consumer",
                    CommunicationMode::PubSub("backpressure.dlq".to_string()),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;

        let report = executor
            .execute(MessageEnvelope::new("work", b"{}".to_vec()))
            .await
            .unwrap();

        assert_eq!(report.total_dead_letters(), 1);
        assert!(matches!(
            report.dead_letters[0].reason,
            DeadLetterReason::Backpressure
        ));
        assert!(report.dead_letters[0].delivered);
        assert!(report.dead_letters[0].delivery_error.is_none());

        let dead_letter_message = timeout(Duration::from_secs(1), dlq_receiver)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .unwrap();
        let dead_letter_envelope = decode_event_envelope(dead_letter_message);
        assert_eq!(
            dead_letter_envelope
                .headers
                .get("x-mofa-dead-letter-reason")
                .map(String::as_str),
            Some("node_backpressure")
        );
    }

    #[tokio::test]
    async fn executor_rejects_router_capacity_configuration() {
        let bus = Arc::new(AgentBus::new());
        let compiled = build_runtime_graph();
        let executor = MessageGraphExecutor::new(compiled, bus).unwrap();

        let err = executor.set_node_capacity("router", 1).await.unwrap_err();
        assert!(matches!(
            err,
            MessageGraphExecutorError::RouterCapacityUnsupported { .. }
        ));
    }

    #[tokio::test]
    async fn executor_reports_dead_letter_delivery_errors() {
        let bus = Arc::new(AgentBus::new());
        let compiled = build_runtime_graph();
        let executor = MessageGraphExecutor::new(compiled, bus).unwrap();

        // No subscriber is registered for orders.dlq, so DLQ dispatch should fail and be reported.
        let report = executor
            .execute(MessageEnvelope::new(
                "order.cancelled",
                br#"{"id":"A-201"}"#.to_vec(),
            ))
            .await
            .unwrap();

        assert_eq!(report.total_dead_letters(), 1);
        assert!(!report.dead_letters[0].delivered);
        assert!(
            report.dead_letters[0]
                .delivery_error
                .as_deref()
                .unwrap_or_default()
                .contains("No subscribers for topic")
        );
    }
}
