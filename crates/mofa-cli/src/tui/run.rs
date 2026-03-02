//! TUI entry point
//!
//! This module provides the main entry point for launching the TUI.

use crate::CliError;

type Result<T> = std::result::Result<T, CliError>;
use tracing::{info, warn};

use crate::tui::app::{App, AppExitInfo};

/// Run the MoFA TUI
///
/// This is the main entry point for the terminal UI.
pub async fn run() -> Result<AppExitInfo> {
    // Initialize logging (only if not already set by main.rs)
    if std::env::var("RUST_LOG").is_err() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info,mofa_cli=debug,mofa_sdk=debug,mofa_kernel=debug")
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt::try_init();
    }

    info!("Starting MoFA TUI...");

    // Create and run the app
    let mut app = App::new()?;

    // Run the main event loop
    let exit_info = app.run().await?;

    // Handle exit
    match &exit_info.mode {
        crate::tui::app_event::ExitMode::Clean => {
            info!("TUI exited cleanly");
            if let Some(summary) = &exit_info.summary {
                println!("{}", summary);
            }
        }
        crate::tui::app_event::ExitMode::Error(err) => {
            warn!("TUI exited with error: {}", err);
            eprintln!("Error: {}", err);
        }
    }

    Ok(exit_info)
}

/// Run the TUI in panic mode (for development/testing)
#[cfg(debug_assertions)]
pub async fn run_with_panic_hook() -> Result<AppExitInfo> {
    // Set up panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to restore terminal
        let _ = crossterm::terminal::disable_raw_mode;
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        // Call original panic hook
        original_hook(panic_info);
    }));

    run().await
}
