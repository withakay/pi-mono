// Footer component - status bar at the bottom of the TUI
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use super::theme::Theme;

/// Footer / status bar widget
pub struct Footer {
    /// Session identifier shown in footer
    pub session_id: String,
    /// Number of messages in the session
    pub message_count: usize,
    /// Optional status text (e.g. "Thinking…", "Ready")
    pub status: String,
    /// Optional token usage info
    pub tokens: Option<TokenInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct TokenInfo {
    pub input: usize,
    pub output: usize,
}

impl Footer {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            message_count: 0,
            status: "Ready".to_string(),
            tokens: None,
        }
    }

    /// Render the footer into a ratatui buffer
    pub fn render_with_theme(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let mut spans = vec![
            Span::styled(
                format!(" session:{} ", self.session_id),
                theme.accent(),
            ),
            Span::styled(
                format!(" msgs:{} ", self.message_count),
                theme.dim(),
            ),
            Span::styled(
                format!(" {} ", self.status),
                theme.normal(),
            ),
        ];

        if let Some(ref tok) = self.tokens {
            spans.push(Span::styled(
                format!(" in:{} out:{} ", tok.input, tok.output),
                theme.dim(),
            ));
        }

        let line = Line::from(spans);
        Paragraph::new(line)
            .style(theme.normal())
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_footer_new() {
        let f = Footer::new("my-session");
        assert_eq!(f.session_id, "my-session");
        assert_eq!(f.message_count, 0);
        assert_eq!(f.status, "Ready");
    }
}

