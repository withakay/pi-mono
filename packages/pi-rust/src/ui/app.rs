// TUI app - ratatui application skeleton
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use anyhow::Result;

use super::editor::Editor;
use super::footer::Footer;
use super::keybindings::{default_app_keybindings, AppAction, KeyBinding};
use super::messages::MessagesPanel;
use super::theme::Theme;
use crate::core::{
    hooks::HookRegistry,
    messages::{Message, MessageContent},
    persistence::SessionManager,
    session::AgentSession,
};
use crate::tools::ToolRegistry;
use std::sync::Arc;

/// The main TUI application state
pub struct App {
    session: AgentSession,
    editor: Editor,
    footer: Footer,
    theme: Theme,
    scroll: u16,
    keybindings: super::keybindings::AppKeybindings,
    should_quit: bool,
}

impl App {
    pub async fn new(
        session_id: Option<String>,
        session_manager: Arc<SessionManager>,
        tool_registry: Arc<ToolRegistry>,
        hook_registry: Arc<HookRegistry>,
    ) -> Result<Self> {
        let sid = session_id.unwrap_or_else(|| "tui".to_string());

        let session = match AgentSession::load(
            sid.clone(),
            session_manager.clone(),
            tool_registry.clone(),
            hook_registry.clone(),
        )
        .await
        {
            Ok(s) => s,
            Err(_) => {
                session_manager.create_session(&sid).await?;
                AgentSession::new(sid.clone(), session_manager, tool_registry, hook_registry)
            }
        };

        let footer = Footer::new(sid);

        Ok(Self {
            session,
            editor: Editor::new().with_title("Message"),
            footer,
            theme: Theme::default(),
            scroll: 0,
            keybindings: default_app_keybindings(),
            should_quit: false,
        })
    }

    /// Run the TUI event loop
    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        loop {
            let messages: Vec<Message> = self.session.get_messages().into_iter().cloned().collect();
            self.footer.message_count = messages.len();

            let theme = self.theme.clone();
            let editor = &self.editor;
            let footer = &self.footer;
            let scroll = self.scroll;

            terminal.draw(|frame| {
                let area = frame.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(1),
                        Constraint::Length(5),
                        Constraint::Length(1),
                    ])
                    .split(area);

                let msg_panel = MessagesPanel::new(&messages).with_scroll(scroll);
                msg_panel.render_with_theme(chunks[0], frame.buffer_mut(), &theme);

                editor.render_with_theme(chunks[1], frame.buffer_mut(), &theme);

                footer.render_with_theme(chunks[2], frame.buffer_mut(), &theme);
            })?;

            if event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    let binding = KeyBinding::new(key.code, key.modifiers);
                    if let Some(action) = self.keybindings.action_for(&binding).cloned() {
                        self.handle_action(action).await?;
                    } else if let KeyCode::Char(c) = key.code {
                        if !key.modifiers.contains(KeyModifiers::CONTROL) {
                            self.editor.insert_char(c);
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    async fn handle_action(&mut self, action: AppAction) -> Result<()> {
        match action {
            AppAction::Quit => {
                self.should_quit = true;
            }
            AppAction::Submit => {
                let content = self.editor.take_content();
                if !content.trim().is_empty() {
                    self.footer.status = "Thinking…".to_string();
                    self.session.add_user_message(content.clone()).await?;

                    // Placeholder response until LLM integration
                    let response = format!("Echo: {}", content);
                    self.session
                        .add_assistant_message(MessageContent::Text(response))
                        .await?;

                    self.footer.status = "Ready".to_string();
                }
            }
            AppAction::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            AppAction::ScrollDown => {
                self.scroll = self.scroll.saturating_add(1);
            }
            AppAction::ClearEditor => {
                self.editor.clear();
            }
            AppAction::CursorLeft => self.editor.move_left(),
            AppAction::CursorRight => self.editor.move_right(),
            AppAction::Home => self.editor.move_home(),
            AppAction::End => self.editor.move_end(),
            AppAction::Backspace => self.editor.backspace(),
            AppAction::Delete => self.editor.delete_forward(),
            AppAction::Newline => self.editor.insert_newline(),
            // In the editor these would move between lines; for now scroll the message panel
            AppAction::CursorUp => self.scroll = self.scroll.saturating_sub(1),
            AppAction::CursorDown => self.scroll = self.scroll.saturating_add(1),
        }
        Ok(())
    }
}

