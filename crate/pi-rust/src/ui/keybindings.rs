use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    pub fn new(key: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn matches(&self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

pub struct AppKeybindings {
    pub submit: KeyBinding,
    pub newline: KeyBinding,
    pub quit: KeyBinding,
    pub scroll_up: KeyBinding,
    pub scroll_down: KeyBinding,
    pub clear: KeyBinding,
}

impl Default for AppKeybindings {
    fn default() -> Self {
        Self {
            submit: KeyBinding::new(KeyCode::Enter, KeyModifiers::NONE),
            newline: KeyBinding::new(KeyCode::Enter, KeyModifiers::SHIFT),
            quit: KeyBinding::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            scroll_up: KeyBinding::new(KeyCode::Up, KeyModifiers::NONE),
            scroll_down: KeyBinding::new(KeyCode::Down, KeyModifiers::NONE),
            clear: KeyBinding::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
        }
    }
}
