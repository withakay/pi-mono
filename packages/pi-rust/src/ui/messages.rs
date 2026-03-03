// Messages component - scrollable message history panel
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use crate::core::messages::{Message, MessageContent, MessageRole};
use super::theme::Theme;

/// Scrollable messages display widget
pub struct MessagesPanel<'a> {
    messages: &'a [Message],
    /// First visible line (scroll offset)
    scroll: u16,
}

impl<'a> MessagesPanel<'a> {
    pub fn new(messages: &'a [Message]) -> Self {
        Self {
            messages,
            scroll: 0,
        }
    }

    pub fn with_scroll(mut self, scroll: u16) -> Self {
        self.scroll = scroll;
        self
    }

    /// Render the panel with a given theme
    pub fn render_with_theme(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let mut lines: Vec<Line> = Vec::new();

        for msg in self.messages {
            let role_label = match msg.role {
                MessageRole::User => "You",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
            };

            let role_style = match msg.role {
                MessageRole::User => theme.user_msg(),
                MessageRole::Assistant => theme.assistant_msg(),
                MessageRole::System => theme.dim(),
            };

            // Role header line
            lines.push(Line::from(Span::styled(
                format!("[{}]", role_label),
                role_style,
            )));

            // Content lines
            let text = match &msg.content {
                MessageContent::Text(t) => t.clone(),
                MessageContent::Blocks(blocks) => blocks
                    .iter()
                    .filter_map(|b| match b {
                        crate::core::messages::ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };

            for content_line in text.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", content_line),
                    theme.normal(),
                )));
            }
            // Blank separator
            lines.push(Line::default());
        }

        let block = Block::default()
            .title("Messages")
            .borders(Borders::ALL)
            .border_style(theme.border());

        Paragraph::new(lines)
            .block(block)
            .scroll((self.scroll, 0))
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::messages::Message;

    #[test]
    fn test_panel_created_empty() {
        let messages: Vec<Message> = vec![];
        let panel = MessagesPanel::new(&messages);
        assert_eq!(panel.scroll, 0);
    }

    #[test]
    fn test_panel_with_scroll() {
        let messages: Vec<Message> = vec![];
        let panel = MessagesPanel::new(&messages).with_scroll(5);
        assert_eq!(panel.scroll, 5);
    }
}

