//! Practical runtime verification for MessageGraphExecutor.
//!
//! Run with:
//! `cargo run --manifest-path examples/Cargo.toml -p message_graph_executor_runtime`

use mofa_kernel::agent::{AgentCapabilities, AgentMetadata, AgentState};
use mofa_kernel::bus::{AgentBus, CommunicationMode};
use mofa_kernel::message::AgentMessage;
use mofa_kernel::message_graph::{
    DeliveryMode, DeliveryPolicy, MessageEnvelope, MessageGraph, MessageGraphExecutor,
    MessageNode, MessageNodeKind, RouteRule,
};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

fn metadata(id: &str) -> AgentMetadata {
    AgentMetadata {
        id: id.to_string(),
        name: id.to_string(),
        description: None,
        version: None,
        capabilities: AgentCapabilities::default(),
        state: AgentState::Ready,
    }
}

fn decode_event_envelope(message: AgentMessage) -> Result<MessageEnvelope, Box<dyn std::error::Error>> {
    match message {
        AgentMessage::Event(mofa_kernel::message::AgentEvent::Custom(_, payload)) => {
            serde_json::from_slice(&payload).map_err(|e| format!("failed to decode envelope from event payload: {}", e).into())
        }
        other => return Err(format!("expected event/custom payload, got {other:?}").into()),
    }
}

fn build_graph() -> Result<mofa_kernel::CompiledMessageGraph, Box<dyn std::error::Error>> {
    let mut graph = MessageGraph::new("orders-runtime").with_max_hops(8);

    graph.add_node(
        "ingress",
        MessageNode::new(MessageNodeKind::Topic {
            topic: "orders.in".to_string(),
        }),
    )?;
    graph.add_node("router", MessageNode::new(MessageNodeKind::Router))?;
    graph.add_node(
        "fraud_agent",
        MessageNode::new(MessageNodeKind::Agent {
            agent_id: "fraud-worker".to_string(),
        }),
    )?;
    graph.add_node(
        "fulfillment_stream",
        MessageNode::new(MessageNodeKind::Stream {
            stream_id: "orders.fulfillment".to_string(),
        }),
    )?;
    graph.add_node(
        "orders_dlq",
        MessageNode::new(MessageNodeKind::Topic {
            topic: "orders.dlq".to_string(),
        }),
    )?;

    graph.add_entry_point("ingress")?;
    graph.set_dead_letter_node("orders_dlq")?;

    graph.add_edge(
        "ingress",
        "router",
        RouteRule::Always,
        DeliveryPolicy::default(),
    )?;
    graph.add_edge(
        "router",
        "fraud_agent",
        RouteRule::HeaderEquals {
            key: "risk".to_string(),
            value: "high".to_string(),
        },
        DeliveryPolicy {
            mode: DeliveryMode::Direct,
            ..DeliveryPolicy::default()
        },
    )?;
    graph.add_edge(
        "router",
        "fulfillment_stream",
        RouteRule::MessageType("order.created".to_string()),
        DeliveryPolicy {
            mode: DeliveryMode::PubSub,
            ..DeliveryPolicy::default()
        },
    )?;

    Ok(graph.compile()?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bus = Arc::new(AgentBus::new());
    let graph = build_graph()?;
    let executor = MessageGraphExecutor::new(graph, bus.clone())?;

    bus.register_channel(
        &metadata("fraud-worker"),
        CommunicationMode::PointToPoint("message_graph_executor".to_string()),
    )
    .await?;
    bus.register_channel(
        &metadata("stream-observer"),
        CommunicationMode::PubSub("orders.fulfillment".to_string()),
    )
    .await?;
    bus.register_channel(
        &metadata("dlq-observer"),
        CommunicationMode::PubSub("orders.dlq".to_string()),
    )
    .await?;

    let fraud_bus = bus.clone();
    let fraud_recv = tokio::spawn(async move {
        fraud_bus
            .receive_message(
                "fraud-worker",
                CommunicationMode::PointToPoint("message_graph_executor".to_string()),
            )
            .await
    });
    let stream_bus = bus.clone();
    let stream_recv = tokio::spawn(async move {
        stream_bus
            .receive_message(
                "stream-observer",
                CommunicationMode::PubSub("orders.fulfillment".to_string()),
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    let high_risk_order =
        MessageEnvelope::new("order.created", br#"{"id":"A-101","amount":1200}"#.to_vec())
            .with_header("risk", "high");
    let report = executor.execute(high_risk_order.clone()).await?;
    println!(
        "normal-routing: dispatched={}, dead_letters={}",
        report.total_dispatched(),
        report.total_dead_letters()
    );

    let fraud_msg = timeout(Duration::from_secs(1), fraud_recv)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { format!("timed out waiting for fraud target message: {}", e).into() })?
        .map_err(|e| -> Box<dyn std::error::Error> { format!("fraud receiver task failed: {}", e).into() })?
        .map_err(|e| -> Box<dyn std::error::Error> { format!("fraud receive failed: {}", e).into() })?
        .ok_or_else(|| -> Box<dyn std::error::Error> { format!("fraud receiver got no message").into() })?;
    let routed = decode_event_envelope(fraud_msg)?;
    println!(
        "fraud target received type='{}' hop_count={}",
        routed.message_type, routed.hop_count
    );

    let stream_msg = timeout(Duration::from_secs(1), stream_recv)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { format!("timed out waiting for stream target message: {}", e).into() })?
        .map_err(|e| -> Box<dyn std::error::Error> { format!("stream receiver task failed: {}", e).into() })?
        .map_err(|e| -> Box<dyn std::error::Error> { format!("stream receive failed: {}", e).into() })?
        .ok_or_else(|| -> Box<dyn std::error::Error> { format!("stream receiver got no message").into() })?;
    match stream_msg {
        AgentMessage::StreamMessage {
            stream_id,
            sequence,
            ..
        } => {
            println!("stream target received stream_id='{stream_id}' sequence={sequence}");
        }
        other => return Err(format!("expected stream message, got {other:?}").into()),
    }

    let dlq_bus = bus.clone();
    let dlq_recv = tokio::spawn(async move {
        dlq_bus
            .receive_message(
                "dlq-observer",
                CommunicationMode::PubSub("orders.dlq".to_string()),
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    let unmatched = MessageEnvelope::new("order.cancelled", br#"{"id":"A-102"}"#.to_vec());
    let dlq_report = executor.execute(unmatched).await?;
    println!(
        "unmatched-routing: dispatched={}, dead_letters={}",
        dlq_report.total_dispatched(),
        dlq_report.total_dead_letters()
    );

    let dlq_msg = timeout(Duration::from_secs(1), dlq_recv)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { format!("timed out waiting for dlq message: {}", e).into() })?
        .map_err(|e| -> Box<dyn std::error::Error> { format!("dlq receiver task failed: {}", e).into() })?
        .map_err(|e| -> Box<dyn std::error::Error> { format!("dlq receive failed: {}", e).into() })?
        .ok_or_else(|| -> Box<dyn std::error::Error> { format!("dlq receiver got no message").into() })?;
    let dlq_envelope = decode_event_envelope(dlq_msg)?;
    let reason = dlq_envelope
        .headers
        .get("x-mofa-dead-letter-reason")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    println!("dlq received reason='{reason}'");

    Ok(())
}
