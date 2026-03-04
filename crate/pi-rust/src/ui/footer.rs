use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::theme::Theme;

pub struct Footer {
    pub model: String,
    pub status: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
}

impl Footer {
    pub fn new(model: String) -> Self {
        Self {
            model,
            status: "Ready".to_string(),
            input_tokens: 0,
            output_tokens: 0,
        }
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn update_tokens(&mut self, input: usize, output: usize) {
        self.input_tokens = input;
        self.output_tokens = output;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let text = if self.input_tokens > 0 || self.output_tokens > 0 {
            format!(
                " {} | {} | in:{} out:{}",
                self.model, self.status, self.input_tokens, self.output_tokens
            )
        } else {
            format!(" {} | {}", self.model, self.status)
        };

        let line = Line::from(Span::styled(text, theme.footer));
        frame.render_widget(Paragraph::new(line), area);
    }
}
