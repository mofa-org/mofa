//! MoFA TUI (Terminal User Interface)
//!
//! An interactive terminal UI for managing agents, plugins, sessions, and configuration.

pub mod app;
pub mod app_event;
pub mod app_event_sender;
pub mod event_stream;
mod run;
mod terminal;

pub use run::run;
pub use app::{App, AppExitInfo};

// Re-export common types
pub use app_event::{AppEvent, ExitMode, View};
pub use app_event_sender::AppEventSender;
