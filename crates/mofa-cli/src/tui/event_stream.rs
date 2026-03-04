//! Event stream for TUI input and drawing
//!
//! Combines keyboard input, paste events, resize events, and draw triggers
//! into a single async stream.

use crate::CliError;

type Result<T> = std::result::Result<T, CliError>;
use crossterm::event::{Event, KeyEvent, MouseEvent};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

/// Events that the TUI processes
#[derive(Debug, Clone)]
pub enum TuiEvent {
    /// Request a redraw
    Draw,
    /// Key press event
    Key(KeyEvent),
    /// Paste event (bracketed paste)
    Paste(String),
    /// Mouse event
    Mouse(MouseEvent),
    /// Terminal resize event
    Resize(u16, u16),
    /// Focus gained
    FocusGained,
    /// Focus lost
    FocusLost,
}

impl TuiEvent {
    /// Check if this event should trigger a redraw
    pub fn requires_redraw(&self) -> bool {
        matches!(
            self,
            TuiEvent::Draw
                | TuiEvent::Resize(..)
                | TuiEvent::Paste(_)
                | TuiEvent::FocusGained
                | TuiEvent::FocusLost
        )
    }

    /// Check if this is a key event
    pub fn as_key(&self) -> Option<KeyEvent> {
        match self {
            TuiEvent::Key(key) => Some(*key),
            _ => None,
        }
    }
}

/// The main event stream for the TUI
///
/// Combines multiple event sources into a single stream.
pub struct TuiEventStream {
    event_rx: mpsc::UnboundedReceiver<TuiEvent>,
    _broker: Arc<EventBroker>,
    terminal_focused: Arc<AtomicBool>,
}

/// Broker for TUI events (allows broadcasting draw events)
#[derive(Debug, Clone)]
pub struct EventBroker {
    draw_tx: mpsc::UnboundedSender<TuiEvent>,
}

impl EventBroker {
    pub fn new() -> Self {
        let (draw_tx, _) = mpsc::unbounded_channel();
        Self { draw_tx }
    }

    /// Send a redraw request to all listeners
    pub fn request_redraw(&self) {
        let _ = self.draw_tx.send(TuiEvent::Draw);
    }

    /// Get the sender for direct event sending
    pub fn sender(&self) -> mpsc::UnboundedSender<TuiEvent> {
        self.draw_tx.clone()
    }
}

impl Default for EventBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiEventStream {
    /// Create a new event stream
    pub fn new() -> Result<Self> {
        // Enable focus change events
        #[cfg(feature = "bracketed-paste")]
        execute!(std::io::stdout(), crossterm::event::EnableFocusChange)?;

        let broker = Arc::new(EventBroker::new());
        let terminal_focused = Arc::new(AtomicBool::new(true));
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Spawn crossterm event poller task
        let broker_clone = Arc::clone(&broker);
        let focused_clone = Arc::clone(&terminal_focused);
        tokio::spawn(async move {
            Self::poll_crossterm_events(broker_clone, focused_clone, event_tx).await;
        });

        Ok(Self {
            event_rx,
            _broker: broker,
            terminal_focused,
        })
    }

    /// Background task that polls crossterm for events
    async fn poll_crossterm_events(
        _broker: Arc<EventBroker>,
        focused: Arc<AtomicBool>,
        event_tx: mpsc::UnboundedSender<TuiEvent>,
    ) {
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Try to read a crossterm event (non-blocking)
            match crossterm::event::poll(Duration::ZERO) {
                Ok(true) => {
                    if let Ok(event) = crossterm::event::read() {
                        let tui_event = Self::convert_event(event, &focused);
                        let _ = event_tx.send(tui_event);
                    }
                }
                Ok(false) => {
                    // No event available, continue
                }
                Err(_) => {
                    // Error reading event, continue
                }
            }
        }
    }

    fn convert_event(event: Event, focused: &AtomicBool) -> TuiEvent {
        use crossterm::event::KeyEventKind;

        match event {
            Event::Key(key) => {
                // Filter out key release events
                if key.kind == KeyEventKind::Release {
                    return TuiEvent::Draw;
                }
                TuiEvent::Key(key)
            }
            Event::Paste(s) => TuiEvent::Paste(s),
            Event::Resize(cols, rows) => TuiEvent::Resize(cols, rows),
            Event::FocusGained => {
                focused.store(true, Ordering::Relaxed);
                TuiEvent::FocusGained
            }
            Event::FocusLost => {
                focused.store(false, Ordering::Relaxed);
                TuiEvent::FocusLost
            }
            Event::Mouse(mouse) => TuiEvent::Mouse(mouse),
        }
    }

    /// Request a redraw
    pub fn redraw(&self) {
        // The broker handles this internally
    }

    /// Check if terminal is focused
    pub fn is_focused(&self) -> bool {
        self.terminal_focused.load(Ordering::Relaxed)
    }

    /// Get the next event from the stream
    pub async fn next(&mut self) -> Option<TuiEvent> {
        self.event_rx.recv().await
    }
}
