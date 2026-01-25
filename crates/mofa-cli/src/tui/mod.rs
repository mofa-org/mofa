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

// Re-export common types
