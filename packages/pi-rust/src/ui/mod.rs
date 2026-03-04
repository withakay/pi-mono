// UI module - Terminal UI components using ratatui

pub mod app;
pub mod editor;
pub mod footer;
pub mod keybindings;
pub mod messages;
pub mod theme;

pub use app::App;
pub use keybindings::AppKeybindings;
pub use theme::Theme;
