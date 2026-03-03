// Editor component - multi-line text input for the TUI
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use super::theme::Theme;

/// Multi-line text editor widget
pub struct Editor {
    /// Current buffer content
    content: String,
    /// Cursor position as byte offset into `content`
    cursor: usize,
    /// Block title (shown in border)
    title: String,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
            title: "Input".to_string(),
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, ch: char) {
        self.content.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Insert a newline at the cursor position
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Delete the character before the cursor (backspace)
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            // Find the previous character boundary
            let prev = self.content[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.content.drain(prev..self.cursor);
            self.cursor = prev;
        }
    }

    /// Delete the character after the cursor (delete key)
    pub fn delete_forward(&mut self) {
        if self.cursor < self.content.len() {
            let next = self.content[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.content.len());
            self.content.drain(self.cursor..next);
        }
    }

    /// Move cursor left by one character
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.content[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right by one character
    pub fn move_right(&mut self) {
        if self.cursor < self.content.len() {
            if let Some((i, ch)) = self.content[self.cursor..].char_indices().next() {
                self.cursor += i + ch.len_utf8();
            }
        }
    }

    /// Move to beginning of content
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move to end of content
    pub fn move_end(&mut self) {
        self.cursor = self.content.len();
    }

    /// Clear the editor content
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor = 0;
    }

    /// Take (consume) the content, resetting the editor
    pub fn take_content(&mut self) -> String {
        let content = self.content.clone();
        self.clear();
        content
    }

    /// Get current content without consuming
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Render the editor into a ratatui buffer
    pub fn render_with_theme(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let lines: Vec<Line> = self
            .content
            .split('\n')
            .map(|l| Line::from(Span::styled(l.to_string(), theme.normal())))
            .collect();

        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(theme.border());

        Paragraph::new(lines)
            .block(block)
            .style(theme.normal())
            .render(area, buf);
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_content() {
        let mut e = Editor::new();
        e.insert_char('H');
        e.insert_char('i');
        assert_eq!(e.content(), "Hi");
    }

    #[test]
    fn test_backspace() {
        let mut e = Editor::new();
        e.insert_char('A');
        e.insert_char('B');
        e.backspace();
        assert_eq!(e.content(), "A");
    }

    #[test]
    fn test_take_content() {
        let mut e = Editor::new();
        e.insert_char('X');
        let c = e.take_content();
        assert_eq!(c, "X");
        assert_eq!(e.content(), "");
    }

    #[test]
    fn test_clear() {
        let mut e = Editor::new();
        e.insert_char('Y');
        e.clear();
        assert_eq!(e.content(), "");
        assert_eq!(e.cursor, 0);
    }
}

