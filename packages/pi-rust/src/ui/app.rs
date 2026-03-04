use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::ui::{
    editor::Editor,
    footer::Footer,
    keybindings::AppKeybindings,
    messages::MessagesView,
    theme::Theme,
};

pub struct App {
    editor: Editor,
    messages: MessagesView,
    footer: Footer,
    theme: Theme,
    keybindings: AppKeybindings,
    should_quit: bool,
    is_running: bool,
    streaming_buffer: String,
}

impl App {
    pub fn new(model: String) -> Self {
        Self {
            editor: Editor::new(),
            messages: MessagesView::new(),
            footer: Footer::new(model),
            theme: Theme::default(),
            keybindings: AppKeybindings::default(),
            should_quit: false,
            is_running: false,
            streaming_buffer: String::new(),
        }
    }

    /// Handle a crossterm event. Returns Some(message) when user submits.
    pub fn handle_event(&mut self, event: Event) -> Option<String> {
        if let Event::Key(KeyEvent { code, modifiers, .. }) = event {
            if self.keybindings.quit.matches(code, modifiers) {
                self.should_quit = true;
                return None;
            }

            if self.keybindings.clear.matches(code, modifiers) {
                self.editor.clear();
                return None;
            }

            if self.keybindings.scroll_up.matches(code, modifiers) {
                self.messages.scroll_up();
                return None;
            }

            if self.keybindings.scroll_down.matches(code, modifiers) {
                self.messages.scroll_down();
                return None;
            }

            if self.keybindings.newline.matches(code, modifiers) {
                self.editor.insert_newline();
                return None;
            }

            if self.keybindings.submit.matches(code, modifiers) && !self.is_running {
                if !self.editor.is_empty() {
                    let content = self.editor.take_content();
                    return Some(content);
                }
                return None;
            }

            match code {
                KeyCode::Backspace => {
                    self.editor.delete_char();
                }
                KeyCode::Char(c) => {
                    self.editor.insert_char(c);
                }
                _ => {}
            }
        }
        None
    }

    pub fn add_message(&mut self, role: String, content: String) {
        self.messages.add_message(role, content);
    }

    pub fn start_streaming(&mut self) {
        self.streaming_buffer.clear();
        self.is_running = true;
    }

    pub fn append_stream(&mut self, text: &str) {
        self.streaming_buffer.push_str(text);
        self.messages.update_last_assistant(&self.streaming_buffer);
    }

    pub fn finish_streaming(&mut self) {
        if !self.streaming_buffer.is_empty() {
            self.messages.finish_streaming();
        }
        self.streaming_buffer.clear();
        self.is_running = false;
    }

    pub fn set_status(&mut self, status: String) {
        self.footer.set_status(status);
    }

    pub fn update_tokens(&mut self, input: usize, output: usize) {
        self.footer.update_tokens(input, output);
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn render(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Length(6),
                Constraint::Length(1),
            ])
            .split(frame.area());

        self.messages.render(frame, chunks[0], &self.theme);
        self.editor.render(frame, chunks[1], &self.theme);
        self.footer.render(frame, chunks[2], &self.theme);
    }
}
