//! Terminal management for the TUI
//!
//! Handles terminal setup, teardown, and raw mode using crossterm.

use crate::CliError;

type Result<T> = std::result::Result<T, CliError>;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};

/// Initialize the terminal for TUI use
///
/// This enables raw mode and enters an alternate screen buffer.
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()
        .map_err(|e| CliError::StateError(format!("Failed to enable raw mode: {}", e)))?;
    execute!(io::stdout(), EnterAlternateScreen)
        .map_err(|e| CliError::StateError(format!("Failed to enter alternate screen: {}", e)))?;
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend)
        .map_err(|e| CliError::StateError(format!("Failed to create terminal: {}", e)))
}

/// Restore the terminal to its original state
///
/// This leaves the alternate screen and disables raw mode.
pub fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    execute!(io::stdout(), LeaveAlternateScreen)
        .map_err(|e| CliError::StateError(format!("Failed to leave alternate screen: {}", e)))?;
    disable_raw_mode()
        .map_err(|e| CliError::StateError(format!("Failed to disable raw mode: {}", e)))?;
    terminal
        .show_cursor()
        .map_err(|e| CliError::StateError(format!("Failed to show cursor: {}", e)))?;
    Ok(())
}

/// Suspend the terminal temporarily (e.g., for running external commands)
pub fn suspend_terminal(_terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    execute!(io::stdout(), LeaveAlternateScreen)
        .map_err(|e| CliError::StateError(format!("Failed to leave alternate screen: {}", e)))?;
    disable_raw_mode()
        .map_err(|e| CliError::StateError(format!("Failed to disable raw mode: {}", e)))?;
    Ok(())
}

/// Resume the terminal after suspension
pub fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    enable_raw_mode()
        .map_err(|e| CliError::StateError(format!("Failed to enable raw mode: {}", e)))?;
    execute!(io::stdout(), EnterAlternateScreen)
        .map_err(|e| CliError::StateError(format!("Failed to enter alternate screen: {}", e)))?;
    terminal
        .clear()
        .map_err(|e| CliError::StateError(format!("Failed to clear terminal: {}", e)))?;
    Ok(())
}
