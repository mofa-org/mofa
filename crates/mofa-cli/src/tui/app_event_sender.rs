//! Thread-safe event sender for TUI internal messaging

use tokio::sync::mpsc::UnboundedSender;

use crate::tui::app_event::AppEvent;

/// A thread-safe sender for TUI events
#[derive(Clone, Debug)]
pub struct AppEventSender(UnboundedSender<AppEvent>);

impl AppEventSender {
    /// Create a new event sender
    pub fn new(sender: UnboundedSender<AppEvent>) -> Self {
        Self(sender)
    }

    /// Send an event
    pub fn send(&self, event: AppEvent) {
        let _ = self.0.send(event);
    }

    /// Check if the sender channel is closed
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

impl From<UnboundedSender<AppEvent>> for AppEventSender {
    fn from(sender: UnboundedSender<AppEvent>) -> Self {
        Self(sender)
    }
}
