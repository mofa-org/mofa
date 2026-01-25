//! Terminal management for the TUI
//!
//! Handles terminal setup, teardown, and raw mode using crossterm.

use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};

/// Initialize the terminal for TUI use
///
/// This enables raw mode and enters an alternate screen buffer.
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::new(backend).context("Failed to create terminal")
}

/// Restore the terminal to its original state
///
/// This leaves the alternate screen and disables raw mode.
pub fn restore_terminal(
    mut terminal: Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    execute!(io::stdout(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    disable_raw_mode().context("Failed to disable raw mode")?;
    terminal
        .show_cursor()
        .context("Failed to show cursor")?;
    Ok(())
}

/// Suspend the terminal temporarily (e.g., for running external commands)
pub fn suspend_terminal(
    _terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    execute!(io::stdout(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    disable_raw_mode().context("Failed to disable raw mode")?;
    Ok(())
}

/// Resume the terminal after suspension
pub fn resume_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<()> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    terminal.clear().context("Failed to clear terminal")?;
    Ok(())
}
