//! Main application state and event loop
//!
//! This module contains the core App struct that manages the TUI state,
//! handles events, and orchestrates rendering.

use crate::CliError;
use super::app_event::{AgentStatus, AppEvent, ExitMode, View};
use super::app_event_sender::AppEventSender;
use super::event_stream::{TuiEvent, TuiEventStream};
use super::terminal::{restore_terminal, setup_terminal};
use crate::widgets::{command_palette::CommandPalette, confirm_dialog::ConfirmDialog};

type Result<T> = std::result::Result<T, CliError>;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Wrap},
};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Information about an agent
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub description: String,
    pub created_at: String,
}

/// Information about a session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub agent_id: String,
    pub created_at: String,
    pub message_count: usize,
    pub status: String,
}

/// Information about a plugin
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
}

/// Actions that can be taken on an overlay
enum OverlayAction {
    Close,
    ExecuteCommand(crate::widgets::command_palette::CommandToExecute),
    ConfirmDialog,
}

/// Overlay types
pub enum Overlay {
    CommandPalette(CommandPalette),
    ConfirmDialog(ConfirmDialog),
}

/// Exit information from the TUI
#[derive(Debug)]
pub struct AppExitInfo {
    pub mode: ExitMode,
    pub summary: Option<String>,
}

/// The main TUI application
pub struct App {
    // Event handling
    pub app_event_tx: AppEventSender,
    app_event_rx: mpsc::UnboundedReceiver<AppEvent>,

    // Terminal state
    terminal_focused: bool,

    // Current state
    pub current_view: View,
    pub overlay: Option<Overlay>,
    pub should_exit: bool,

    // Data
    pub agents: Vec<AgentInfo>,
    pub sessions: Vec<SessionInfo>,
    pub plugins: Vec<PluginInfo>,

    // UI state
    pub selected_agent: Option<usize>,
    pub selected_session: Option<usize>,
    pub selected_plugin: Option<usize>,
    pub filter_input: String,
    pub scroll_offset: usize,

    // Exit info
    exit_mode: ExitMode,
    exit_summary: Option<String>,

    // Tick rate for idle redraws
    tick_rate: Duration,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        let (app_event_tx, app_event_rx) = mpsc::unbounded_channel();

        // Initialize with some demo data
        let agents = vec![
            AgentInfo {
                id: "agent-001".to_string(),
                name: "Research Agent".to_string(),
                status: AgentStatus::Running,
                description: "Performs web research and summarization".to_string(),
                created_at: "2024-01-15".to_string(),
            },
            AgentInfo {
                id: "agent-002".to_string(),
                name: "Code Assistant".to_string(),
                status: AgentStatus::Stopped,
                description: "Helps with code generation and refactoring".to_string(),
                created_at: "2024-01-16".to_string(),
            },
            AgentInfo {
                id: "agent-003".to_string(),
                name: "Data Analyst".to_string(),
                status: AgentStatus::Error,
                description: "Analyzes data and generates reports".to_string(),
                created_at: "2024-01-17".to_string(),
            },
        ];

        let sessions = vec![
            SessionInfo {
                id: "sess-001".to_string(),
                agent_id: "agent-001".to_string(),
                created_at: "2024-01-20 10:30".to_string(),
                message_count: 42,
                status: "completed".to_string(),
            },
            SessionInfo {
                id: "sess-002".to_string(),
                agent_id: "agent-002".to_string(),
                created_at: "2024-01-20 11:15".to_string(),
                message_count: 15,
                status: "active".to_string(),
            },
        ];

        let plugins = vec![
            PluginInfo {
                name: "web-search".to_string(),
                version: "1.0.0".to_string(),
                description: "Web search capability".to_string(),
                installed: true,
            },
            PluginInfo {
                name: "file-ops".to_string(),
                version: "2.1.0".to_string(),
                description: "File system operations".to_string(),
                installed: true,
            },
            PluginInfo {
                name: "database".to_string(),
                version: "1.5.0".to_string(),
                description: "Database connectivity".to_string(),
                installed: false,
            },
        ];

        Ok(Self {
            app_event_tx: AppEventSender::new(app_event_tx),
            app_event_rx,
            terminal_focused: true,
            current_view: View::Dashboard,
            overlay: None,
            should_exit: false,
            agents,
            sessions,
            plugins,
            selected_agent: None,
            selected_session: None,
            selected_plugin: None,
            filter_input: String::new(),
            scroll_offset: 0,
            exit_mode: ExitMode::Clean,
            exit_summary: None,
            tick_rate: Duration::from_millis(250),
        })
    }

    /// Run the main event loop
    pub async fn run(&mut self) -> Result<AppExitInfo> {
        let mut terminal = setup_terminal().map_err(|e| CliError::StateError(format!("Failed to setup terminal: {}", e)))?;
        let mut event_stream = TuiEventStream::new().map_err(|e| CliError::StateError(format!("Failed to create event stream: {}", e)))?;

        info!("Starting TUI event loop");

        // Initial draw
        terminal.draw(|f| self.render(f))?;

        loop {
            // Handle any pending internal events
            while let Ok(event) = self.app_event_rx.try_recv() {
                self.handle_app_event(event);
            }

            // Check if we should exit
            if self.should_exit {
                break;
            }

            // Wait for next event (with timeout for periodic redraws)
            match tokio::time::timeout(self.tick_rate, event_stream.next()).await {
                Ok(Some(event)) => {
                    self.handle_tui_event(event, &event_stream);
                    terminal.draw(|f| self.render(f))?;
                }
                Ok(None) => {
                    // Event stream closed
                    debug!("Event stream closed, exiting...");
                    self.should_exit = true;
                }
                Err(_) => {
                    // Timeout - trigger a periodic redraw
                    terminal.draw(|f| self.render(f))?;
                }
            }
        }

        // Restore terminal
        restore_terminal(terminal).map_err(|e| CliError::StateError(format!("Failed to restore terminal: {}", e)))?;

        Ok(AppExitInfo {
            mode: self.exit_mode.clone(),
            summary: self.exit_summary.clone(),
        })
    }

    /// Handle an internal app event
    fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SwitchView(view) => {
                self.current_view = view;
                self.selected_agent = None;
                self.selected_session = None;
                self.selected_plugin = None;
                self.filter_input.clear();
                self.scroll_offset = 0;
                self.overlay = None;
            }
            AppEvent::ShowCommandPalette => {
                self.overlay = Some(Overlay::CommandPalette(CommandPalette::new()));
            }
            AppEvent::HideOverlay => {
                self.overlay = None;
            }
            AppEvent::AgentListUpdated => {}
            AppEvent::AgentStatusChanged(id, status) => {
                if let Some(agent) = self.agents.iter_mut().find(|a| a.id == id) {
                    agent.status = status;
                }
            }
            AppEvent::Exit(mode) => {
                self.exit_mode = mode;
                self.should_exit = true;
            }
            AppEvent::Draw => {}
            _ => {
                debug!("Unhandled app event: {:?}", event);
            }
        }
    }

    /// Handle a TUI input event
    fn handle_tui_event(&mut self, event: TuiEvent, _event_stream: &TuiEventStream) {
        match event {
            TuiEvent::Key(key) => {
                self.handle_key_event(key);
            }
            TuiEvent::Resize(..) => {}
            TuiEvent::FocusGained | TuiEvent::FocusLost => {
                self.terminal_focused = matches!(event, TuiEvent::FocusGained);
            }
            _ => {}
        }
    }

    /// Handle a keyboard event
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        // Handle overlay first
        let overlay_result = if let Some(ref mut overlay) = self.overlay {
            match overlay {
                Overlay::CommandPalette(palette) => match palette.handle_key(key) {
                    crate::widgets::command_palette::CommandPaletteResult::Continue => None,
                    crate::widgets::command_palette::CommandPaletteResult::Execute(action) => {
                        Some(OverlayAction::ExecuteCommand(action))
                    }
                    crate::widgets::command_palette::CommandPaletteResult::Cancel => {
                        Some(OverlayAction::Close)
                    }
                },
                Overlay::ConfirmDialog(dialog) => match dialog.handle_key(key) {
                    crate::widgets::confirm_dialog::ConfirmDialogResult::Continue => None,
                    crate::widgets::confirm_dialog::ConfirmDialogResult::Confirm => {
                        Some(OverlayAction::ConfirmDialog)
                    }
                    crate::widgets::confirm_dialog::ConfirmDialogResult::Cancel => {
                        Some(OverlayAction::Close)
                    }
                },
            }
        } else {
            None
        };

        if let Some(action) = overlay_result {
            match action {
                OverlayAction::Close => {
                    self.overlay = None;
                }
                OverlayAction::ExecuteCommand(cmd) => {
                    cmd.run(self);
                    self.overlay = None;
                }
                OverlayAction::ConfirmDialog => {
                    // Take ownership of the dialog to avoid borrow issues
                    if let Some(Overlay::ConfirmDialog(dialog)) = self.overlay.take() {
                        dialog.confirm(self);
                    }
                }
            }
            return;
        }

        // Global shortcuts
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.app_event_tx.send(AppEvent::Exit(ExitMode::Clean));
                return;
            }
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.app_event_tx.send(AppEvent::ShowCommandPalette);
                return;
            }
            (KeyCode::Esc, _) => {
                if self.current_view != View::Dashboard {
                    self.app_event_tx
                        .send(AppEvent::SwitchView(View::Dashboard));
                    return;
                }
            }
            _ => {}
        }

        // View-specific shortcuts
        match self.current_view {
            View::Dashboard => self.handle_dashboard_key(key),
            View::Agents => self.handle_agents_key(key),
            View::Sessions => self.handle_sessions_key(key),
            View::Config => {}
            View::Plugins => {}
        }
    }

    fn handle_dashboard_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('1') => {
                self.app_event_tx.send(AppEvent::SwitchView(View::Agents));
            }
            KeyCode::Char('2') => {
                self.app_event_tx.send(AppEvent::SwitchView(View::Sessions));
            }
            KeyCode::Char('3') => {
                self.app_event_tx.send(AppEvent::SwitchView(View::Config));
            }
            KeyCode::Char('4') => {
                self.app_event_tx.send(AppEvent::SwitchView(View::Plugins));
            }
            _ => {}
        }
    }

    fn handle_agents_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_agent.is_none() && !self.agents.is_empty() {
                    self.selected_agent = Some(0);
                } else if let Some(idx) = self.selected_agent {
                    self.selected_agent = if idx > 0 { Some(idx - 1) } else { None };
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_agent.is_none() && !self.agents.is_empty() {
                    self.selected_agent = Some(0);
                } else if let Some(idx) = self.selected_agent {
                    self.selected_agent = if idx + 1 < self.agents.len() {
                        Some(idx + 1)
                    } else {
                        Some(idx)
                    };
                }
            }
            KeyCode::Char('c') => {
                self.app_event_tx.send(AppEvent::CreateAgent);
            }
            KeyCode::Char('s') => {
                if let Some(idx) = self.selected_agent {
                    let agent = &self.agents[idx];
                    self.app_event_tx
                        .send(AppEvent::StartAgent(agent.id.clone()));
                }
            }
            KeyCode::Char('x') => {
                if let Some(idx) = self.selected_agent {
                    let agent = &self.agents[idx];
                    self.app_event_tx
                        .send(AppEvent::StopAgent(agent.id.clone()));
                }
            }
            KeyCode::Char('r') => {
                if let Some(idx) = self.selected_agent {
                    let agent = &self.agents[idx];
                    self.app_event_tx
                        .send(AppEvent::RestartAgent(agent.id.clone()));
                }
            }
            _ => {}
        }
    }

    fn handle_sessions_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_session.is_none() && !self.sessions.is_empty() {
                    self.selected_session = Some(0);
                } else if let Some(idx) = self.selected_session {
                    self.selected_session = if idx > 0 { Some(idx - 1) } else { None };
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_session.is_none() && !self.sessions.is_empty() {
                    self.selected_session = Some(0);
                } else if let Some(idx) = self.selected_session {
                    self.selected_session = if idx + 1 < self.sessions.len() {
                        Some(idx + 1)
                    } else {
                        Some(idx)
                    };
                }
            }
            KeyCode::Char('d') => {
                if let Some(idx) = self.selected_session {
                    let session = &self.sessions[idx];
                    self.app_event_tx
                        .send(AppEvent::DeleteSession(session.id.clone()));
                }
            }
            KeyCode::Char('e') => {
                if let Some(idx) = self.selected_session {
                    let session = &self.sessions[idx];
                    self.app_event_tx
                        .send(AppEvent::ExportSession(session.id.clone()));
                }
            }
            _ => {}
        }
    }

    /// Render the UI
    fn render(&self, frame: &mut Frame) {
        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
            .split(frame.area());

        // Render current view
        match self.current_view {
            View::Dashboard => self.render_dashboard(chunks[0], frame),
            View::Agents => self.render_agents(chunks[0], frame),
            View::Sessions => self.render_sessions(chunks[0], frame),
            View::Config => self.render_config(chunks[0], frame),
            View::Plugins => self.render_plugins(chunks[0], frame),
        }

        // Render overlay if active
        if let Some(overlay) = &self.overlay {
            self.render_overlay(frame, overlay);
        }

        // Render status bar
        self.render_status_bar(chunks[1], frame);
    }

    fn render_dashboard(&self, area: Rect, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(0)].as_ref())
            .split(area);

        // Render header/banner
        self.render_header(chunks[0], frame);

        // Render stats
        self.render_stats(chunks[1], frame);
    }

    fn render_header(&self, area: Rect, frame: &mut Frame) {
        let logo = Text::from(vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "   __  __          _____                    _             _",
                Style::default().fg(Color::Rgb(108, 95, 224)),
            )]),
            Line::from(vec![Span::styled(
                "  |  \\/  |   /\\   |  __ \\                  | |           (_)",
                Style::default().fg(Color::Rgb(108, 95, 224)),
            )]),
            Line::from(vec![Span::styled(
                "  | \\  / |  /  \\  | |__) |_ _ _ __ ___  ___| | __ _ _ __ _ _ __ ___",
                Style::default().fg(Color::Rgb(108, 95, 224)),
            )]),
            Line::from(vec![Span::styled(
                "  | |\\/| | / /\\ \\ |  ___/ _` | '__/ _ \\/ __| |/ _` | '__| | '_ ` _ \\",
                Style::default().fg(Color::Rgb(108, 95, 224)),
            )]),
            Line::from(vec![Span::styled(
                "  | |  | |/ ____ \\ | |  | (_| | | |  __/\\__ \\ | (_| | |  | | | | | |",
                Style::default().fg(Color::Rgb(108, 95, 224)),
            )]),
            Line::from(vec![Span::styled(
                "  |_|  |_/_/    \\_\\_|   \\__,_|_|  \\___||___/_|\\__,_|_|  |_|_| |_| |_|",
                Style::default().fg(Color::Rgb(108, 95, 224)),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "                Modular Framework for Agents",
                Style::default()
                    .fg(Color::Rgb(108, 95, 224))
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ]);

        let paragraph = Paragraph::new(logo).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }

    fn render_stats(&self, area: Rect, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                ]
                .as_ref(),
            )
            .split(area);

        let running_count = self
            .agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Running))
            .count();
        let stopped_count = self
            .agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Stopped))
            .count();

        // Agent count card
        let agent_card = Block::bordered()
            .title(" Agents ")
            .title_style(Style::default().fg(Color::Rgb(108, 95, 224)));
        let agent_text = vec![
            Line::from(format!("Total: {}", self.agents.len())),
            Line::from(format!("Running: {}", running_count)),
            Line::from(format!("Stopped: {}", stopped_count)),
        ];
        let agent_para = Paragraph::new(agent_text)
            .block(agent_card)
            .wrap(Wrap { trim: true });
        frame.render_widget(agent_para, chunks[0]);

        // Sessions card
        let session_card = Block::bordered()
            .title(" Sessions ")
            .title_style(Style::default().fg(Color::Rgb(108, 95, 224)));
        let session_text = vec![
            Line::from(format!("Total: {}", self.sessions.len())),
            Line::from("Active: 1"),
        ];
        let session_para = Paragraph::new(session_text)
            .block(session_card)
            .wrap(Wrap { trim: true });
        frame.render_widget(session_para, chunks[1]);

        // Plugins card
        let plugin_card = Block::bordered()
            .title(" Plugins ")
            .title_style(Style::default().fg(Color::Rgb(108, 95, 224)));
        let installed_count = self.plugins.iter().filter(|p| p.installed).count();
        let plugin_text = vec![
            Line::from(format!("Installed: {}", installed_count)),
            Line::from(format!("Available: {}", self.plugins.len())),
        ];
        let plugin_para = Paragraph::new(plugin_text)
            .block(plugin_card)
            .wrap(Wrap { trim: true });
        frame.render_widget(plugin_para, chunks[2]);

        // Quick help
        let help_card = Block::bordered()
            .title(" Quick Help ")
            .title_style(Style::default().fg(Color::Rgb(108, 95, 224)));
        let help_text = vec![
            Line::from("1: Agents  2: Sessions"),
            Line::from("3: Config  4: Plugins"),
            Line::from("Ctrl+P: Commands  q: Quit"),
        ];
        let help_para = Paragraph::new(help_text)
            .block(help_card)
            .wrap(Wrap { trim: true });
        frame.render_widget(help_para, chunks[3]);
    }

    fn render_agents(&self, area: Rect, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(6)].as_ref())
            .split(area);

        // Agent list
        let items: Vec<Line> = self
            .agents
            .iter()
            .enumerate()
            .map(|(idx, agent)| {
                let is_selected = self.selected_agent == Some(idx);
                let status_color = match agent.status {
                    AgentStatus::Running => Color::Green,
                    AgentStatus::Stopped => Color::Gray,
                    AgentStatus::Error => Color::Red,
                    AgentStatus::Starting => Color::Yellow,
                    AgentStatus::Stopping => Color::Yellow,
                };

                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", agent.status.symbol()),
                        Style::default().fg(status_color),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        agent.name.clone(),
                        if is_selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::raw(format!(" ({})", agent.id)),
                ])
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::bordered()
                    .title(" Agents ")
                    .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(69, 69, 117))
                    .add_modifier(Modifier::BOLD),
            );

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(self.selected_agent);

        frame.render_stateful_widget(list, chunks[0], &mut list_state);

        // Agent details
        if let Some(idx) = self.selected_agent {
            if let Some(agent) = self.agents.get(idx) {
                let details = vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(agent.name.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("ID: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(agent.id.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(format!("{}", agent.status)),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "Description: ",
                            Style::default().fg(Color::Rgb(108, 95, 224)),
                        ),
                        Span::raw(agent.description.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Created: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(agent.created_at.clone()),
                    ]),
                ];

                let details_para = Paragraph::new(details)
                    .block(
                        Block::bordered()
                            .title(" Details ")
                            .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
                    )
                    .wrap(Wrap { trim: true });
                frame.render_widget(details_para, chunks[1]);
            }
        } else {
            let hints = vec![
                Line::from("c: Create  s: Start  x: Stop  r: Restart"),
                Line::from("↑↓: Navigate  Esc: Back to Dashboard"),
            ];

            let hints_para = Paragraph::new(hints)
                .block(
                    Block::bordered()
                        .title(" Actions ")
                        .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(hints_para, chunks[1]);
        }
    }

    fn render_sessions(&self, area: Rect, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(6)].as_ref())
            .split(area);

        // Session list
        let items: Vec<Line> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(idx, session)| {
                let is_selected = self.selected_session == Some(idx);
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", session.id),
                        if is_selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::raw(format!(" - Agent: {}", session.agent_id)),
                    Span::raw(format!(" ({})", session.created_at)),
                ])
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::bordered()
                    .title(" Sessions ")
                    .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(69, 69, 117))
                    .add_modifier(Modifier::BOLD),
            );

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(self.selected_session);

        frame.render_stateful_widget(list, chunks[0], &mut list_state);

        // Session details or hints
        if let Some(idx) = self.selected_session {
            if let Some(session) = self.sessions.get(idx) {
                let details = vec![
                    Line::from(vec![
                        Span::styled(
                            "Session ID: ",
                            Style::default().fg(Color::Rgb(108, 95, 224)),
                        ),
                        Span::raw(session.id.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Agent ID: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(session.agent_id.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Created: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(session.created_at.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Messages: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(format!("{}", session.message_count)),
                    ]),
                    Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(session.status.clone()),
                    ]),
                ];

                let details_para = Paragraph::new(details)
                    .block(
                        Block::bordered()
                            .title(" Details ")
                            .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
                    )
                    .wrap(Wrap { trim: true });
                frame.render_widget(details_para, chunks[1]);
            }
        } else {
            let hints = vec![
                Line::from("d: Delete  e: Export"),
                Line::from("↑↓: Navigate  Esc: Back to Dashboard"),
            ];

            let hints_para = Paragraph::new(hints)
                .block(
                    Block::bordered()
                        .title(" Actions ")
                        .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(hints_para, chunks[1]);
        }
    }

    fn render_config(&self, area: Rect, frame: &mut Frame) {
        let text = vec![
            Line::from("Configuration Management"),
            Line::from(""),
            Line::from("Use arrow keys to navigate configuration options."),
            Line::from("Press Esc to return to Dashboard."),
        ];

        let para = Paragraph::new(text)
            .block(
                Block::bordered()
                    .title(" Configuration ")
                    .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(para, area);
    }

    fn render_plugins(&self, area: Rect, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(6)].as_ref())
            .split(area);

        // Plugin list
        let items: Vec<Line> = self
            .plugins
            .iter()
            .enumerate()
            .map(|(idx, plugin)| {
                let is_selected = self.selected_plugin == Some(idx);
                let status = if plugin.installed { "[x]" } else { "[ ]" };
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(status, Style::default().fg(Color::Green)),
                    Span::raw(" "),
                    Span::styled(
                        plugin.name.clone(),
                        if is_selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::raw(format!(" v{}", plugin.version)),
                ])
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::bordered()
                    .title(" Plugins ")
                    .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(69, 69, 117))
                    .add_modifier(Modifier::BOLD),
            );

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(self.selected_plugin);

        frame.render_stateful_widget(list, chunks[0], &mut list_state);

        // Plugin details or hints
        if let Some(idx) = self.selected_plugin {
            if let Some(plugin) = self.plugins.get(idx) {
                let details = vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(plugin.name.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Version: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::raw(plugin.version.clone()),
                    ]),
                    Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(Color::Rgb(108, 95, 224))),
                        Span::styled(
                            if plugin.installed {
                                "Installed"
                            } else {
                                "Available"
                            },
                            Style::default().fg(if plugin.installed {
                                Color::Green
                            } else {
                                Color::Yellow
                            }),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "Description: ",
                            Style::default().fg(Color::Rgb(108, 95, 224)),
                        ),
                        Span::raw(plugin.description.clone()),
                    ]),
                ];

                let details_para = Paragraph::new(details)
                    .block(
                        Block::bordered()
                            .title(" Details ")
                            .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
                    )
                    .wrap(Wrap { trim: true });
                frame.render_widget(details_para, chunks[1]);
            }
        } else {
            let hints = vec![
                Line::from("i: Info  u: Uninstall"),
                Line::from("↑↓: Navigate  Esc: Back to Dashboard"),
            ];

            let hints_para = Paragraph::new(hints)
                .block(
                    Block::bordered()
                        .title(" Actions ")
                        .title_style(Style::default().fg(Color::Rgb(108, 95, 224))),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(hints_para, chunks[1]);
        }
    }

    fn render_overlay(&self, frame: &mut Frame, overlay: &Overlay) {
        let area = centered_rect(60, 20, frame.area());

        match overlay {
            Overlay::CommandPalette(palette) => palette.render(area, frame),
            Overlay::ConfirmDialog(dialog) => dialog.render(area, frame),
        }
    }

    fn render_status_bar(&self, area: Rect, frame: &mut Frame) {
        let (left, right) = match self.current_view {
            View::Dashboard => (
                format!("Dashboard | Agents: {}", self.agents.len()),
                "q:Quit 1:Agents 2:Sessions 3:Config 4:Plugins Ctrl+P:Commands".to_string(),
            ),
            View::Agents => (
                "Agents".to_string(),
                "Esc:Back c:Create s:Start x:Stop r:Restart ↑↓:Navigate".to_string(),
            ),
            View::Sessions => (
                "Sessions".to_string(),
                "Esc:Back d:Delete e:Export ↑↓:Navigate".to_string(),
            ),
            View::Config => (
                "Config".to_string(),
                "Esc:Back ↑↓:Navigate Enter:Edit".to_string(),
            ),
            View::Plugins => (
                "Plugins".to_string(),
                "Esc:Back i:Info u:Uninstall ↑↓:Navigate".to_string(),
            ),
        };

        let line = Line::from(vec![
            Span::styled(left, Style::default().fg(Color::Rgb(108, 95, 224))),
            Span::raw(" | "),
            Span::styled(right, Style::default().fg(Color::Gray)),
        ]);

        let para = Paragraph::new(line)
            .style(Style::default().bg(Color::Rgb(30, 30, 50)).fg(Color::White));
        frame.render_widget(para, area);
    }
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
