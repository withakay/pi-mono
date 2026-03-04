// UI module - Terminal UI components using ratatui

pub mod app;
pub mod editor;
pub mod messages;
pub mod footer;
pub mod theme;
pub mod keybindings;

pub use app::App;
pub use theme::Theme;
pub use keybindings::AppKeybindings;

