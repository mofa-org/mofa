//! Command palette widget
//!
//! A searchable command palette for quick access to actions (Ctrl+P).

use crate::tui::app::App;
use crate::tui::app_event::{AppEvent, ExitMode, View};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, Paragraph},
    Frame,
};

/// Result type for command palette interaction
pub enum CommandPaletteResult {
    /// Continue using the command palette
    Continue,
    /// Execute the selected command
    Execute(CommandToExecute),
    /// Cancel without executing
    Cancel,
}

/// A command that can be executed from the palette
#[derive(Debug, Clone)]
pub struct CommandToExecute {
    pub view: Option<View>,
    pub feature: Option<&'static str>,
    pub quit: bool,
}

impl CommandToExecute {
    pub fn run(self, app: &mut App) {
        if let Some(view) = self.view {
            app.app_event_tx.send(AppEvent::SwitchView(view));
        }
        if let Some(feature) = self.feature {
            if feature == "create_agent" {
                app.app_event_tx.send(AppEvent::CreateAgent);
            }
        }
        if self.quit {
            app.app_event_tx.send(AppEvent::Exit(ExitMode::Clean));
        }
    }
}

/// Command palette widget
#[derive(Debug)]
pub struct CommandPalette {
    /// All available commands
    commands: Vec<PaletteCommand>,
    /// Filtered commands based on input
    filtered_commands: Vec<usize>,
    /// Current input text
    input: String,
    /// Currently selected command index
    selected: usize,
    /// Cursor position in input
    cursor: usize,
}

#[derive(Debug)]
struct PaletteCommand {
    /// Display name
    name: String,
    /// Description of what the command does
    description: String,
    /// The action to execute
    action: CommandToExecute,
}

impl CommandPalette {
    /// Create a new command palette
    pub fn new() -> Self {
        let commands = vec![
            PaletteCommand {
                name: "Go to Dashboard".to_string(),
                description: "Navigate to the dashboard view".to_string(),
                action: CommandToExecute {
                    view: Some(View::Dashboard),
                    feature: None,
                    quit: false,
                },
            },
            PaletteCommand {
                name: "Go to Agents".to_string(),
                description: "Navigate to the agents view".to_string(),
                action: CommandToExecute {
                    view: Some(View::Agents),
                    feature: None,
                    quit: false,
                },
            },
            PaletteCommand {
                name: "Go to Sessions".to_string(),
                description: "Navigate to the sessions view".to_string(),
                action: CommandToExecute {
                    view: Some(View::Sessions),
                    feature: None,
                    quit: false,
                },
            },
            PaletteCommand {
                name: "Go to Config".to_string(),
                description: "Navigate to the configuration view".to_string(),
                action: CommandToExecute {
                    view: Some(View::Config),
                    feature: None,
                    quit: false,
                },
            },
            PaletteCommand {
                name: "Go to Plugins".to_string(),
                description: "Navigate to the plugins view".to_string(),
                action: CommandToExecute {
                    view: Some(View::Plugins),
                    feature: None,
                    quit: false,
                },
            },
            PaletteCommand {
                name: "Create Agent".to_string(),
                description: "Create a new agent".to_string(),
                action: CommandToExecute {
                    view: None,
                    feature: Some("create_agent"),
                    quit: false,
                },
            },
            PaletteCommand {
                name: "Quit".to_string(),
                description: "Exit the TUI".to_string(),
                action: CommandToExecute {
                    view: None,
                    feature: None,
                    quit: true,
                },
            },
        ];

        let filtered_commands = (0..commands.len()).collect();

        Self {
            commands,
            filtered_commands,
            input: String::new(),
            selected: 0,
            cursor: 0,
        }
    }

    /// Handle a key event
    pub fn handle_key(&mut self, key: KeyEvent) -> CommandPaletteResult {
        // Handle control-modified keys first
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => return CommandPaletteResult::Cancel,
                KeyCode::Char('a') => {
                    self.cursor = 0;
                    return CommandPaletteResult::Continue;
                }
                KeyCode::Char('e') => {
                    self.cursor = self.input.len();
                    return CommandPaletteResult::Continue;
                }
                KeyCode::Char('j') => {
                    if !self.filtered_commands.is_empty() {
                        self.selected = (self.selected + 1).min(self.filtered_commands.len() - 1);
                    }
                    return CommandPaletteResult::Continue;
                }
                KeyCode::Char('k') => {
                    if !self.filtered_commands.is_empty() {
                        self.selected = self.selected.saturating_sub(1);
                    }
                    return CommandPaletteResult::Continue;
                }
                _ => {}
            }
        }

        // Handle regular keys (without control modifier)
        match key.code {
            KeyCode::Char(c) => {
                // Only insert if no other modifiers are present
                if key.modifiers.is_empty() {
                    self.input.insert(self.cursor, c);
                    self.cursor += 1;
                    self.filter();
                }
                CommandPaletteResult::Continue
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.input.remove(self.cursor - 1);
                    self.cursor -= 1;
                    self.filter();
                }
                CommandPaletteResult::Continue
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                    self.filter();
                }
                CommandPaletteResult::Continue
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                CommandPaletteResult::Continue
            }
            KeyCode::Right => {
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
                CommandPaletteResult::Continue
            }
            KeyCode::Up => {
                if !self.filtered_commands.is_empty() {
                    self.selected = self.selected.saturating_sub(1);
                }
                CommandPaletteResult::Continue
            }
            KeyCode::Down => {
                if !self.filtered_commands.is_empty() {
                    self.selected = (self.selected + 1).min(self.filtered_commands.len() - 1);
                }
                CommandPaletteResult::Continue
            }
            KeyCode::Enter => {
                if let Some(&idx) = self.filtered_commands.get(self.selected) {
                    let command = self.commands[idx].action.clone();
                    return CommandPaletteResult::Execute(command);
                }
                CommandPaletteResult::Cancel
            }
            KeyCode::Esc => CommandPaletteResult::Cancel,
            _ => CommandPaletteResult::Continue,
        }
    }

    /// Filter commands based on current input
    fn filter(&mut self) {
        let query = self.input.to_lowercase();
        self.filtered_commands = if query.is_empty() {
            (0..self.commands.len()).collect()
        } else {
            self.commands
                .iter()
                .enumerate()
                .filter(|(_, cmd)| {
                    cmd.name.to_lowercase().contains(&query)
                        || cmd.description.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect()
        };
        self.selected = 0;
    }

    /// Render the command palette
    pub fn render(&self, area: Rect, frame: &mut Frame) {
        // Clear the background
        frame.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(area);

        // Input box
        let input_text = vec![
            Line::from(vec![
                Span::styled(
                    "> ",
                    Style::default().fg(Color::Rgb(108, 95, 224)),
                ),
                Span::raw(&self.input),
                Span::raw(" "), // Cursor indicator
            ]),
        ];

        let input_para = Paragraph::new(input_text)
            .block(
                Block::bordered()
                    .title(" Commands ")
                    .title_style(Style::default().fg(Color::Rgb(108, 95, 224)).bold()),
            );
        frame.render_widget(input_para, chunks[0]);

        // Command list
        let items: Vec<ListItem> = self
            .filtered_commands
            .iter()
            .map(|&idx| {
                let cmd = &self.commands[idx];
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            &cmd.name,
                            Style::default()
                                .fg(Color::Cyan)
                                .bold(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            &cmd.description,
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(Block::bordered())
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(69, 69, 117))
                    .bold(),
            );

        let mut list_state = ratatui::widgets::ListState::default();
        if !self.filtered_commands.is_empty() {
            list_state.select(Some(self.selected));
        }

        frame.render_stateful_widget(list, chunks[1], &mut list_state);
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}
