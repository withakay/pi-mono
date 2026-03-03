// Theme support - color palette for the ratatui TUI
use ratatui::style::{Color, Modifier, Style};

/// Named colors used across the UI
#[derive(Debug, Clone)]
pub struct Theme {
    /// Primary foreground (user input, most text)
    pub fg: Color,
    /// Dimmed text (timestamps, metadata)
    pub fg_dim: Color,
    /// Background
    pub bg: Color,
    /// Border color
    pub border: Color,
    /// Active / highlighted element
    pub accent: Color,
    /// User message bubble
    pub user_msg: Color,
    /// Assistant message bubble
    pub assistant_msg: Color,
    /// Tool call indicator
    pub tool_call: Color,
    /// Error text
    pub error: Color,
    /// Success / ok indicator
    pub success: Color,
}

impl Theme {
    /// Default dark theme
    pub fn dark() -> Self {
        Self {
            fg: Color::White,
            fg_dim: Color::DarkGray,
            bg: Color::Black,
            border: Color::DarkGray,
            accent: Color::Cyan,
            user_msg: Color::Blue,
            assistant_msg: Color::Green,
            tool_call: Color::Yellow,
            error: Color::Red,
            success: Color::Green,
        }
    }

    /// Light theme
    pub fn light() -> Self {
        Self {
            fg: Color::Black,
            fg_dim: Color::Gray,
            bg: Color::White,
            border: Color::Gray,
            accent: Color::Blue,
            user_msg: Color::Blue,
            assistant_msg: Color::DarkGray,
            tool_call: Color::Yellow,
            error: Color::Red,
            success: Color::Green,
        }
    }

    // -------------------------------------------------------------------------
    // Convenience style builders
    // -------------------------------------------------------------------------

    pub fn normal(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.fg_dim)
    }

    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn user_msg(&self) -> Style {
        Style::default().fg(self.user_msg).add_modifier(Modifier::BOLD)
    }

    pub fn assistant_msg(&self) -> Style {
        Style::default().fg(self.assistant_msg)
    }

    pub fn tool_call(&self) -> Style {
        Style::default().fg(self.tool_call).add_modifier(Modifier::ITALIC)
    }

    pub fn error(&self) -> Style {
        Style::default().fg(self.error).add_modifier(Modifier::BOLD)
    }

    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_created() {
        let theme = Theme::dark();
        assert_eq!(theme.bg, Color::Black);
    }

    #[test]
    fn test_light_theme_created() {
        let theme = Theme::light();
        assert_eq!(theme.bg, Color::White);
    }

    #[test]
    fn test_default_is_dark() {
        let theme = Theme::default();
        assert_eq!(theme.bg, Color::Black);
    }
}

