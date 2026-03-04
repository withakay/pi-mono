use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::ui::theme::Theme;

pub struct Editor {
    pub content: String,
    pub cursor_pos: usize,
    pub is_focused: bool,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
            is_focused: true,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        self.content.insert(self.cursor_pos, '\n');
        self.cursor_pos += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        // Find the start of the previous char
        let prev = self.content[..self.cursor_pos]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.content.drain(prev..self.cursor_pos);
        self.cursor_pos = prev;
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_pos = 0;
    }

    pub fn take_content(&mut self) -> String {
        let content = self.content.clone();
        self.clear();
        content
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let style = if self.is_focused {
            theme.editor_focused
        } else {
            theme.editor_normal
        };

        // Build text with cursor indicator
        let display = if self.is_focused {
            let (before, after) = self.content.split_at(self.cursor_pos);
            let char_len = after.chars().next().map(|c| c.len_utf8()).unwrap_or(0);
            let cursor_char = after.chars().next().map(|c| c.to_string()).unwrap_or_else(|| "█".to_string());
            let after_cursor = if char_len == 0 { "" } else { &after[char_len..] };
            format!("{}{}{}", before, cursor_char, after_cursor)
        } else {
            self.content.clone()
        };

        let lines: Vec<Line> = display
            .lines()
            .map(|l| Line::from(Span::styled(l.to_string(), style)))
            .collect();

        let text = if lines.is_empty() {
            Text::from(Line::from(Span::styled(
                if self.is_focused { "█" } else { "" },
                Style::default().fg(ratatui::style::Color::DarkGray),
            )))
        } else {
            Text::from(lines)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border)
            .title(" Input ");

        let paragraph = Paragraph::new(text).block(block);
        frame.render_widget(paragraph, area);
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
