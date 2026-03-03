// Keybindings - configurable keyboard shortcuts for the TUI
use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::HashMap;

/// An action that can be triggered by a keybinding
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppAction {
    /// Submit the current editor contents
    Submit,
    /// Quit / exit the application
    Quit,
    /// Scroll messages up
    ScrollUp,
    /// Scroll messages down
    ScrollDown,
    /// Clear the editor
    ClearEditor,
    /// Move cursor up in the editor
    CursorUp,
    /// Move cursor down in the editor
    CursorDown,
    /// Move cursor left in the editor
    CursorLeft,
    /// Move cursor right in the editor
    CursorRight,
    /// Delete character before cursor
    Backspace,
    /// Delete character after cursor
    Delete,
    /// Move to start of line
    Home,
    /// Move to end of line
    End,
    /// New line in editor
    Newline,
}

/// A key combination (key code + modifiers)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn plain(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }

    pub fn ctrl(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::CONTROL)
    }

    pub fn shift(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::SHIFT)
    }
}

/// All configurable keybindings for the application
#[derive(Debug, Clone)]
pub struct AppKeybindings {
    /// Map from action to list of key bindings that trigger it
    pub bindings: HashMap<AppAction, Vec<KeyBinding>>,
}

impl AppKeybindings {
    /// Look up what action (if any) a key press maps to
    pub fn action_for(&self, key: &KeyBinding) -> Option<&AppAction> {
        self.bindings
            .iter()
            .find_map(|(action, keys)| if keys.contains(key) { Some(action) } else { None })
    }
}

/// Default keybindings — all values here are configurable
pub fn default_app_keybindings() -> AppKeybindings {
    let mut bindings: HashMap<AppAction, Vec<KeyBinding>> = HashMap::new();

    bindings.insert(
        AppAction::Submit,
        vec![KeyBinding::plain(KeyCode::Enter)],
    );
    bindings.insert(
        AppAction::Quit,
        vec![
            KeyBinding::ctrl(KeyCode::Char('q')),
            KeyBinding::ctrl(KeyCode::Char('c')),
        ],
    );
    bindings.insert(
        AppAction::ScrollUp,
        vec![
            KeyBinding::plain(KeyCode::PageUp),
            KeyBinding::ctrl(KeyCode::Up),
        ],
    );
    bindings.insert(
        AppAction::ScrollDown,
        vec![
            KeyBinding::plain(KeyCode::PageDown),
            KeyBinding::ctrl(KeyCode::Down),
        ],
    );
    bindings.insert(
        AppAction::ClearEditor,
        vec![KeyBinding::ctrl(KeyCode::Char('l'))],
    );
    bindings.insert(
        AppAction::CursorUp,
        vec![KeyBinding::plain(KeyCode::Up)],
    );
    bindings.insert(
        AppAction::CursorDown,
        vec![KeyBinding::plain(KeyCode::Down)],
    );
    bindings.insert(
        AppAction::CursorLeft,
        vec![KeyBinding::plain(KeyCode::Left)],
    );
    bindings.insert(
        AppAction::CursorRight,
        vec![KeyBinding::plain(KeyCode::Right)],
    );
    bindings.insert(
        AppAction::Backspace,
        vec![KeyBinding::plain(KeyCode::Backspace)],
    );
    bindings.insert(
        AppAction::Delete,
        vec![KeyBinding::plain(KeyCode::Delete)],
    );
    bindings.insert(
        AppAction::Home,
        vec![KeyBinding::plain(KeyCode::Home)],
    );
    bindings.insert(
        AppAction::End,
        vec![KeyBinding::plain(KeyCode::End)],
    );
    bindings.insert(
        AppAction::Newline,
        vec![KeyBinding::shift(KeyCode::Enter)],
    );

    AppKeybindings { bindings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keybindings_created() {
        let kb = default_app_keybindings();
        assert!(!kb.bindings.is_empty());
    }

    #[test]
    fn test_action_for_enter() {
        let kb = default_app_keybindings();
        let key = KeyBinding::plain(KeyCode::Enter);
        let action = kb.action_for(&key);
        assert_eq!(action, Some(&AppAction::Submit));
    }

    #[test]
    fn test_action_for_ctrl_q() {
        let kb = default_app_keybindings();
        let key = KeyBinding::ctrl(KeyCode::Char('q'));
        let action = kb.action_for(&key);
        assert_eq!(action, Some(&AppAction::Quit));
    }
}

