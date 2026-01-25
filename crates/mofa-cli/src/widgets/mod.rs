//! TUI Widget components
//!
//! Reusable UI widgets for the MoFA TUI.

pub mod command_palette;
pub mod confirm_dialog;

pub use command_palette::{CommandPalette, CommandPaletteResult, CommandToExecute};
pub use confirm_dialog::{ConfirmDialog, ConfirmDialogResult};
