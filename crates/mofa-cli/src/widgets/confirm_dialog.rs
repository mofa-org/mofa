//! Confirmation dialog widget
//!
//! A dialog for confirming destructive actions.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

/// Result type for confirm dialog interaction
pub enum ConfirmDialogResult {
    /// Continue using the dialog
    Continue,
    /// User confirmed the action
    Confirm,
    /// User canceled the action
    Cancel,
}

/// A confirmation dialog
pub struct ConfirmDialog {
    /// Dialog title
    pub title: String,
    /// Message to display
    pub message: String,
    /// The action to execute on confirm
    confirm_action: Box<dyn Fn(&mut crate::tui::app::App) + Send + Sync>,
    /// Current selection (true = confirm, false = cancel)
    selected: bool,
}

impl ConfirmDialog {
    /// Create a new confirmation dialog
    pub fn new<F>(title: String, message: String, confirm_action: F) -> Self
    where
        F: Fn(&mut crate::tui::app::App) + Send + Sync + 'static,
    {
        Self {
            title,
            message,
            confirm_action: Box::new(confirm_action),
            selected: false,
        }
    }

    /// Handle a key event
    pub fn handle_key(&mut self, key: KeyEvent) -> ConfirmDialogResult {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected = false;
                ConfirmDialogResult::Continue
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.selected = true;
                ConfirmDialogResult::Continue
            }
            KeyCode::Tab => {
                self.selected = !self.selected;
                ConfirmDialogResult::Continue
            }
            KeyCode::Enter => {
                if self.selected {
                    ConfirmDialogResult::Confirm
                } else {
                    ConfirmDialogResult::Cancel
                }
            }
            KeyCode::Esc => ConfirmDialogResult::Cancel,
            _ => ConfirmDialogResult::Continue,
        }
    }

    /// Execute the confirm action
    pub fn confirm(&self, app: &mut crate::tui::app::App) {
        (self.confirm_action)(app);
    }

    /// Render the confirmation dialog
    pub fn render(&self, area: Rect, frame: &mut Frame) {
        // Clear the background
        frame.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Length(3)].as_ref())
            .margin(1)
            .split(area);

        // Message section
        let message_lines = vec![
            Line::from(vec![
                Span::styled(
                    "⚠ ",
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    &self.title,
                    Style::default().fg(Color::Yellow).bold(),
                ),
            ]),
            Line::from(""),
            Line::from(self.message.as_str()),
        ];

        let msg_paragraph = Paragraph::new(message_lines)
            .block(
                Block::bordered()
                    .title(" Confirm ")
                    .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
            )
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(msg_paragraph, chunks[0]);

        // Buttons section
        let cancel_style = if !self.selected {
            Style::default().bg(Color::Rgb(69, 69, 117)).bold()
        } else {
            Style::default()
        };
        let confirm_style = if self.selected {
            Style::default().bg(Color::Red).bold()
        } else {
            Style::default()
        };

        let buttons = vec![Line::from(vec![
            Span::raw("[ "),
            Span::styled("Cancel", cancel_style),
            Span::raw(" ]  "),
            Span::raw("[ "),
            Span::styled("Confirm", confirm_style),
            Span::raw(" ]"),
        ])];

        let btn_paragraph = Paragraph::new(buttons)
            .alignment(Alignment::Center);

        frame.render_widget(btn_paragraph, chunks[1]);
    }
}
