use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub user_message: Style,
    pub assistant_message: Style,
    pub system_message: Style,
    pub editor_normal: Style,
    pub editor_focused: Style,
    pub footer: Style,
    pub border: Style,
    pub tool_call: Style,
    pub tool_result: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_dark()
    }
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            user_message: Style::default().fg(Color::Cyan),
            assistant_message: Style::default().fg(Color::Green),
            system_message: Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
            editor_normal: Style::default().fg(Color::White),
            editor_focused: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            footer: Style::default().fg(Color::DarkGray),
            border: Style::default().fg(Color::DarkGray),
            tool_call: Style::default().fg(Color::Magenta),
            tool_result: Style::default().fg(Color::Blue),
        }
    }
}
