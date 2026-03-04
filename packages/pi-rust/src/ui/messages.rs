use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::ui::theme::Theme;

pub struct DisplayMessage {
    pub role: String,
    pub content: String,
    pub is_streaming: bool,
}

pub struct MessagesView {
    pub messages: Vec<DisplayMessage>,
    pub scroll_offset: usize,
}

impl MessagesView {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
        }
    }

    pub fn add_message(&mut self, role: String, content: String) {
        self.messages.push(DisplayMessage {
            role,
            content,
            is_streaming: false,
        });
        self.scroll_to_bottom();
    }

    pub fn update_last_assistant(&mut self, content: &str) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == "Assistant" && last.is_streaming {
                last.content = content.to_string();
                return;
            }
        }
        self.messages.push(DisplayMessage {
            role: "Assistant".to_string(),
            content: content.to_string(),
            is_streaming: true,
        });
        self.scroll_to_bottom();
    }

    pub fn finish_streaming(&mut self) {
        if let Some(last) = self.messages.last_mut() {
            last.is_streaming = false;
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn scroll_to_bottom(&mut self) {
        // Large value; clamped during render
        self.scroll_offset = usize::MAX / 2;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border)
            .title(" Messages ");

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Build all lines
        let mut all_lines: Vec<Line> = Vec::new();
        for msg in &self.messages {
            let (header_style, content_style) = match msg.role.as_str() {
                "User" => (theme.user_message, theme.user_message),
                "Assistant" => (theme.assistant_message, Style::default()),
                "System" => (theme.system_message, theme.system_message),
                _ => (theme.system_message, Style::default()),
            };

            let streaming_suffix = if msg.is_streaming { " ▋" } else { "" };
            all_lines.push(Line::from(Span::styled(
                format!("{}:{}", msg.role, streaming_suffix),
                header_style,
            )));

            for line in msg.content.lines() {
                all_lines.push(Line::from(Span::styled(line.to_string(), content_style)));
            }
            all_lines.push(Line::from(""));
        }

        let total_lines = all_lines.len();
        let visible_height = inner.height as usize;

        // Clamp scroll offset
        let max_scroll = total_lines.saturating_sub(visible_height);
        let scroll = self.scroll_offset.min(max_scroll);

        let visible_lines: Vec<Line> = all_lines.into_iter().skip(scroll).collect();
        let text = Text::from(visible_lines);

        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, inner);

        // Render scrollbar if needed
        if total_lines > visible_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll);
            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }
}

impl Default for MessagesView {
    fn default() -> Self {
        Self::new()
    }
}
