//! WebSocket handler for real-time updates
//!
//! Provides WebSocket support for live monitoring data

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast, mpsc};
use tracing::{debug, error, info};

use super::metrics::{MetricsCollector, MetricsSnapshot};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    /// Full metrics snapshot
    #[serde(rename = "metrics")]
    Metrics(MetricsSnapshot),

    /// Agent update
    #[serde(rename = "agent_update")]
    AgentUpdate {
        agent_id: String,
        state: String,
        tasks_completed: u64,
    },

    /// Workflow update
    #[serde(rename = "workflow_update")]
    WorkflowUpdate {
        workflow_id: String,
        status: String,
        progress: f64,
    },

    /// Plugin update
    #[serde(rename = "plugin_update")]
    PluginUpdate {
        plugin_id: String,
        state: String,
        call_count: u64,
    },

    /// LLM model inference update - real-time metrics for model performance
    ///
    /// Aligns with GSoC Idea 2 - Studio Observability Dashboard
    /// for per-model inference monitoring (tokens/s, TTFT, etc.)
    #[serde(rename = "llm_update")]
    LLMUpdate {
        plugin_id: String,
        provider_name: String,
        model_name: String,
        total_requests: u64,
        successful_requests: u64,
        avg_latency_ms: f64,
        tokens_per_second: Option<f64>,
        error_rate: f64,
    },

    /// System alert
    #[serde(rename = "alert")]
    Alert {
        level: String,
        message: String,
        source: String,
    },

    /// Heartbeat
    #[serde(rename = "heartbeat")]
    Heartbeat { timestamp: u64 },

    /// Subscribe to specific updates
    #[serde(rename = "subscribe")]
    Subscribe { topics: Vec<String> },

    /// Unsubscribe from updates
    #[serde(rename = "unsubscribe")]
    Unsubscribe { topics: Vec<String> },

    /// Error message
    #[serde(rename = "error")]
    Error { message: String },

    /// Acknowledgment
    #[serde(rename = "ack")]
    Ack { message_id: String },
}

/// WebSocket client tracking
#[derive(Debug)]
pub struct WebSocketClient {
    /// Client ID
    pub id: String,
    /// Connected timestamp
    pub connected_at: u64,
    /// Subscribed topics
    pub subscriptions: Vec<String>,
    /// Message sender
    sender: mpsc::Sender<WebSocketMessage>,
}

impl WebSocketClient {
    pub fn new(id: String, sender: mpsc::Sender<WebSocketMessage>) -> Self {
        Self {
            id,
            connected_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            subscriptions: vec!["metrics".to_string()], // Default subscription
            sender,
        }
    }

    pub async fn send(
        &self,
        msg: WebSocketMessage,
    ) -> Result<(), mpsc::error::SendError<WebSocketMessage>> {
        self.sender.send(msg).await
    }

    pub fn is_subscribed(&self, topic: &str) -> bool {
        self.subscriptions.contains(&topic.to_string())
            || self.subscriptions.contains(&"*".to_string())
    }
}

/// WebSocket handler state
pub struct WebSocketHandler {
    /// Connected clients
    clients: Arc<RwLock<HashMap<String, WebSocketClient>>>,
    /// Metrics collector
    collector: Arc<MetricsCollector>,
    /// Broadcast channel for updates
    broadcast_tx: broadcast::Sender<WebSocketMessage>,
    /// Update interval
    update_interval: Duration,
}

impl WebSocketHandler {
    pub fn new(collector: Arc<MetricsCollector>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1024);

        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            collector,
            broadcast_tx,
            update_interval: Duration::from_secs(1),
        }
    }

    pub fn with_update_interval(mut self, interval: Duration) -> Self {
        self.update_interval = interval;
        self
    }

    /// Get broadcast sender for external updates
    pub fn broadcast_tx(&self) -> broadcast::Sender<WebSocketMessage> {
        self.broadcast_tx.clone()
    }

    /// Broadcast a message to all subscribed clients
    pub async fn broadcast(&self, topic: &str, msg: WebSocketMessage) {
        let clients = self.clients.read().await;
        for client in clients.values() {
            if client.is_subscribed(topic)
                && let Err(e) = client.send(msg.clone()).await
            {
                debug!("Failed to send to client {}: {}", client.id, e);
            }
        }
    }

    /// Send alert to all clients
    pub async fn send_alert(&self, level: &str, message: &str, source: &str) {
        let alert = WebSocketMessage::Alert {
            level: level.to_string(),
            message: message.to_string(),
            source: source.to_string(),
        };
        self.broadcast("alerts", alert).await;
    }

    /// Get connected client count
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Get client IDs
    pub async fn client_ids(&self) -> Vec<String> {
        self.clients.read().await.keys().cloned().collect()
    }

    /// Start background update task
    pub fn start_updates(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let interval = self.update_interval;
        info!("Starting WebSocket updates with interval {:?}", interval);

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;

                // Collect and broadcast metrics
                let snapshot = self.collector.current().await;
                let msg = WebSocketMessage::Metrics(snapshot);

                let _ = self.broadcast_tx.send(msg);
            }
        })
    }

    /// Handle WebSocket upgrade
    pub async fn handle_upgrade(
        ws: WebSocketUpgrade,
        State(handler): State<Arc<WebSocketHandler>>,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| handler.handle_socket(socket))
    }

    /// Handle individual WebSocket connection
    async fn handle_socket(self: Arc<Self>, socket: WebSocket) {
        let client_id = uuid::Uuid::now_v7().to_string();
        info!("WebSocket client connected: {}", client_id);

        let (mut sender, mut receiver) = socket.split();

        // Create message channel for this client
        let (tx, mut rx) = mpsc::channel::<WebSocketMessage>(256);

        // Register client
        let client = WebSocketClient::new(client_id.clone(), tx);
        {
            let mut clients = self.clients.write().await;
            clients.insert(client_id.clone(), client);
        }

        // Subscribe to broadcast channel
        let mut broadcast_rx = self.broadcast_tx.subscribe();

        // Task to send messages to client
        let send_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Messages from direct send
                    Some(msg) = rx.recv() => {
                        let json = serde_json::to_string(&msg).unwrap_or_default();
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    // Messages from broadcast
                    Ok(msg) = broadcast_rx.recv() => {
                        let json = serde_json::to_string(&msg).unwrap_or_default();
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        // Task to receive messages from client
        let clients = self.clients.clone();
        let client_id_clone = client_id.clone();
        let receive_task = tokio::spawn(async move {
            while let Some(result) = receiver.next().await {
                match result {
                    Ok(Message::Text(text)) => {
                        // Parse incoming message
                        if let Ok(msg) = serde_json::from_str::<WebSocketMessage>(&text) {
                            match msg {
                                WebSocketMessage::Subscribe { topics } => {
                                    let mut clients = clients.write().await;
                                    if let Some(client) = clients.get_mut(&client_id_clone) {
                                        for topic in topics {
                                            if !client.subscriptions.contains(&topic) {
                                                client.subscriptions.push(topic);
                                            }
                                        }
                                    }
                                }
                                WebSocketMessage::Unsubscribe { topics } => {
                                    let mut clients = clients.write().await;
                                    if let Some(client) = clients.get_mut(&client_id_clone) {
                                        client.subscriptions.retain(|t| !topics.contains(t));
                                    }
                                }
                                WebSocketMessage::Heartbeat { .. } => {
                                    // Just acknowledge heartbeats
                                    debug!("Heartbeat from client {}", client_id_clone);
                                }
                                _ => {
                                    debug!("Received message from client: {:?}", msg);
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        break;
                    }
                    Ok(Message::Ping(_data)) => {
                        debug!("Ping from client {}", client_id_clone);
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = send_task => {}
            _ = receive_task => {}
        }

        // Cleanup
        {
            let mut clients = self.clients.write().await;
            clients.remove(&client_id);
        }
        info!("WebSocket client disconnected: {}", client_id);
    }
}

/// Create WebSocket route handler
pub fn create_websocket_handler(
    collector: Arc<MetricsCollector>,
) -> (Arc<WebSocketHandler>, axum::routing::MethodRouter) {
    let handler = Arc::new(WebSocketHandler::new(collector));
    let handler_clone = handler.clone();

    let route = axum::routing::get(move |ws: WebSocketUpgrade| {
        let h = handler_clone.clone();
        async move { WebSocketHandler::handle_upgrade(ws, State(h)).await }
    });

    (handler, route)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_message_serialize() {
        let msg = WebSocketMessage::Heartbeat { timestamp: 12345 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("heartbeat"));
        assert!(json.contains("12345"));
    }

    #[test]
    fn test_websocket_client_subscription() {
        let (tx, _) = mpsc::channel(16);
        let mut client = WebSocketClient::new("test-1".to_string(), tx);

        assert!(client.is_subscribed("metrics")); // Default subscription

        client.subscriptions.push("alerts".to_string());
        assert!(client.is_subscribed("alerts"));
        assert!(!client.is_subscribed("other"));

        client.subscriptions.push("*".to_string());
        assert!(client.is_subscribed("anything")); // Wildcard matches all
    }

    #[tokio::test]
    async fn test_websocket_handler_client_count() {
        let collector = Arc::new(MetricsCollector::new(Default::default()));
        let handler = WebSocketHandler::new(collector);

        assert_eq!(handler.client_count().await, 0);
    }
}
