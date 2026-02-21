//! Telemetry Emitter Implementations
//!
//! Concrete implementations of `TelemetryEmitter` for use with the workflow executor.

use async_trait::async_trait;
use mofa_kernel::workflow::telemetry::{DebugEvent, TelemetryEmitter};
use std::sync::Arc;
use tokio::sync::mpsc;

// ============================================================================
// ChannelTelemetryEmitter — sends events over an mpsc channel
// ============================================================================

/// Sends `DebugEvent`s over a `tokio::sync::mpsc` channel.
///
/// This emitter is designed for real-time consumption: a future time-travel UI
/// or any subscriber can receive events as they are produced by the executor.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::workflow::telemetry::ChannelTelemetryEmitter;
///
/// let (emitter, mut rx) = ChannelTelemetryEmitter::new(1024);
///
/// // Attach emitter to executor
/// let executor = WorkflowExecutor::new(config).with_telemetry(Arc::new(emitter));
///
/// // Consume events in another task
/// tokio::spawn(async move {
///     while let Some(event) = rx.recv().await {
///         println!("{:?}", event);
///     }
/// });
/// ```
pub struct ChannelTelemetryEmitter {
    tx: mpsc::Sender<DebugEvent>,
}

impl ChannelTelemetryEmitter {
    /// Create a new channel emitter with the given buffer capacity.
    ///
    /// Returns the emitter and the receiving half of the channel.
    pub fn new(buffer: usize) -> (Self, mpsc::Receiver<DebugEvent>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self { tx }, rx)
    }
}

#[async_trait]
impl TelemetryEmitter for ChannelTelemetryEmitter {
    async fn emit(&self, event: DebugEvent) {
        // Non-blocking send — if the channel is full, drop the event
        let _ = self.tx.try_send(event);
    }
}

// ============================================================================
// RecordingTelemetryEmitter — forwards to SessionRecorder + optional channel
// ============================================================================

/// Wraps a `SessionRecorder` and forwards events both to the recorder
/// and an optional downstream channel.
///
/// This is the typical emitter used in production: it persists events
/// to storage while optionally streaming them to a live consumer.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::workflow::telemetry::RecordingTelemetryEmitter;
/// use mofa_foundation::workflow::session_recorder::InMemorySessionRecorder;
///
/// let recorder = Arc::new(InMemorySessionRecorder::new());
/// let emitter = RecordingTelemetryEmitter::new("session-1", recorder);
/// ```
pub struct RecordingTelemetryEmitter {
    session_id: String,
    recorder: Arc<dyn mofa_kernel::workflow::telemetry::SessionRecorder>,
    downstream: Option<mpsc::Sender<DebugEvent>>,
}

impl RecordingTelemetryEmitter {
    /// Create a new recording emitter that persists to the given recorder.
    pub fn new(
        session_id: impl Into<String>,
        recorder: Arc<dyn mofa_kernel::workflow::telemetry::SessionRecorder>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            recorder,
            downstream: None,
        }
    }

    /// Also forward events to a downstream channel for real-time consumption.
    pub fn with_downstream(mut self, tx: mpsc::Sender<DebugEvent>) -> Self {
        self.downstream = Some(tx);
        self
    }
}

#[async_trait]
impl TelemetryEmitter for RecordingTelemetryEmitter {
    async fn emit(&self, event: DebugEvent) {
        // Record to persistent storage (ignore errors to avoid disrupting execution)
        let _ = self.recorder.record_event(&self.session_id, &event).await;

        // Forward to downstream channel if present
        if let Some(ref tx) = self.downstream {
            let _ = tx.try_send(event);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_channel_emitter_sends_events() {
        let (emitter, mut rx) = ChannelTelemetryEmitter::new(16);

        let event = DebugEvent::NodeStart {
            node_id: "test_node".to_string(),
            timestamp_ms: 1000,
            state_snapshot: json!({}),
        };

        emitter.emit(event).await;

        let received = rx.try_recv().unwrap();
        assert_eq!(received.node_id(), Some("test_node"));
        assert_eq!(received.timestamp_ms(), 1000);
    }

    #[tokio::test]
    async fn test_channel_emitter_drops_when_full() {
        let (emitter, _rx) = ChannelTelemetryEmitter::new(1);

        let event = DebugEvent::WorkflowStart {
            workflow_id: "w".to_string(),
            execution_id: "e".to_string(),
            timestamp_ms: 0,
        };

        // Fill the buffer
        emitter.emit(event.clone()).await;
        // This should not panic even though buffer is full
        emitter.emit(event).await;
    }

    #[tokio::test]
    async fn test_channel_emitter_is_enabled() {
        let (emitter, _rx) = ChannelTelemetryEmitter::new(1);
        assert!(emitter.is_enabled());
    }
}
