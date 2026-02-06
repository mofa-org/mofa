//! Color theme for the MoFA TUI
//!
//! Defines the color scheme used throughout the terminal UI.

use ratatui::style::{Color, Modifier, Style};

/// MoFA TUI color theme
#[derive(Debug, Clone, Copy)]
pub struct Theme;

impl Theme {
    /// Primary brand color (purple)
    pub const PRIMARY: Color = Color::Rgb(0x6C, 0x5F, 0xE0);

    /// Success color (green)
    pub const SUCCESS: Color = Color::Rgb(0x4A, 0xD6, 0x69);

    /// Warning color (yellow)
    pub const WARNING: Color = Color::Rgb(0xF9, 0xA8, 0x25);

    /// Error color (red)
    pub const ERROR: Color = Color::Rgb(0xF4, 0x3F, 0x5E);

    /// Dimmed background color (dim purple)
    pub const DIM: Color = Color::Rgb(0x45, 0x45, 0x75);

    /// Background color for status bar
    pub const STATUS_BAR_BG: Color = Color::Rgb(0x1E, 0x1E, 0x32);

    /// Selection background color
    pub const SELECTION_BG: Color = Color::Rgb(0x45, 0x45, 0x75);

    /// Default text color
    pub const TEXT: Color = Color::White;

    /// Dimmed text color
    pub const TEXT_DIM: Color = Color::Gray;

    /// Border color
    pub const BORDER: Color = Color::Rgb(0x6C, 0x5F, 0xE0);

    /// Create a style with the primary color
    pub fn primary() -> Style {
        Style::default().fg(Self::PRIMARY)
    }

    /// Create a bold primary style
    pub fn primary_bold() -> Style {
        Style::default()
            .fg(Self::PRIMARY)
            .add_modifier(Modifier::BOLD)
    }

    /// Create a success style
    pub fn success() -> Style {
        Style::default().fg(Self::SUCCESS)
    }

    /// Create a warning style
    pub fn warning() -> Style {
        Style::default().fg(Self::WARNING)
    }

    /// Create an error style
    pub fn error() -> Style {
        Style::default().fg(Self::ERROR)
    }

    /// Create a dim style
    pub fn dim() -> Style {
        Style::default().fg(Self::DIM)
    }

    /// Create a selection style
    pub fn selection() -> Style {
        Style::default()
            .bg(Self::SELECTION_BG)
            .add_modifier(Modifier::BOLD)
    }

    /// Get status color based on agent status
    pub fn status_color(running: bool, error: bool) -> Color {
        if error {
            Self::ERROR
        } else if running {
            Self::SUCCESS
        } else {
            Color::Gray
        }
    }

    /// Get the symbol for a status
    pub fn status_symbol(
        running: bool,
        starting: bool,
        stopping: bool,
        error: bool,
    ) -> &'static str {
        if error {
            ""
        } else if running {
            ""
        } else if starting {
            ""
        } else if stopping {
            ""
        } else {
            ""
        }
    }
}

/// Default style helpers
impl Theme {
    /// Default text style
    pub fn default() -> Style {
        Style::default().fg(Self::TEXT)
    }

    /// Dim text style
    pub fn default_dim() -> Style {
        Style::default().fg(Self::TEXT_DIM)
    }

    /// Border style
    pub fn border() -> Style {
        Style::default().fg(Self::BORDER)
    }

    /// Bold style
    pub fn bold() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }

    /// Bold primary style
    pub fn bold_primary() -> Style {
        Style::default()
            .fg(Self::PRIMARY)
            .add_modifier(Modifier::BOLD)
    }
}
