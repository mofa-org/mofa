//! MoFA CLI library exports.

pub mod cli;
pub mod commands;
pub mod config;
pub mod context;
pub mod error;
pub mod output;
pub mod plugin_catalog;
pub mod render;
pub mod state;
pub mod store;
pub mod tui;
pub mod utils;
pub mod widgets;

pub use error::{CliError, CliResult, IntoCliReport};

