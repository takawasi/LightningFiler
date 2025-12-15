//! Input handling and keybinding resolution

use app_core::{Command, CommandId};
use std::collections::HashMap;
use winit::event::{ElementState, KeyEvent, MouseButton};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Input handler that maps keys/mouse to commands
pub struct InputHandler {
    /// Key bindings: key string -> command ID
    bindings: HashMap<String, String>,

    /// Current modifier state
    modifiers: ModifiersState,
}

impl InputHandler {
    /// Create a new input handler with bindings
    pub fn new(bindings: HashMap<String, Vec<String>>) -> Self {
        // Invert the bindings map: command -> keys becomes key -> command
        let mut key_to_command = HashMap::new();

        for (command, keys) in bindings {
            for key in keys {
                key_to_command.insert(key.to_lowercase(), command.clone());
            }
        }

        Self {
            bindings: key_to_command,
            modifiers: ModifiersState::empty(),
        }
    }

    /// Update modifier state
    pub fn update_modifiers(&mut self, modifiers: ModifiersState) {
        self.modifiers = modifiers;
    }

    /// Handle a key event and return the corresponding command
    pub fn handle_key(&self, event: &KeyEvent) -> Option<Command> {
        if event.state != ElementState::Pressed {
            return None;
        }

        let key_str = self.key_to_string(&event.logical_key);
        let full_key = self.build_key_string(&key_str);

        tracing::debug!("Key pressed: {}", full_key);

        self.bindings
            .get(&full_key.to_lowercase())
            .map(|cmd_id| Command::new(cmd_id))
    }

    /// Build a key string with modifiers
    fn build_key_string(&self, key: &str) -> String {
        let mut parts = Vec::new();

        if self.modifiers.control_key() {
            parts.push("Ctrl");
        }
        if self.modifiers.alt_key() {
            parts.push("Alt");
        }
        if self.modifiers.shift_key() {
            parts.push("Shift");
        }
        if self.modifiers.super_key() {
            parts.push("Super");
        }

        parts.push(key);
        parts.join("+")
    }

    /// Convert a logical key to a string
    fn key_to_string(&self, key: &Key) -> String {
        match key {
            Key::Named(named) => match named {
                NamedKey::Space => "Space".to_string(),
                NamedKey::Enter => "Return".to_string(),
                NamedKey::Tab => "Tab".to_string(),
                NamedKey::Escape => "Escape".to_string(),
                NamedKey::Backspace => "Backspace".to_string(),
                NamedKey::Delete => "Delete".to_string(),
                NamedKey::Insert => "Insert".to_string(),
                NamedKey::Home => "Home".to_string(),
                NamedKey::End => "End".to_string(),
                NamedKey::PageUp => "PageUp".to_string(),
                NamedKey::PageDown => "PageDown".to_string(),
                NamedKey::ArrowUp => "Up".to_string(),
                NamedKey::ArrowDown => "Down".to_string(),
                NamedKey::ArrowLeft => "Left".to_string(),
                NamedKey::ArrowRight => "Right".to_string(),
                NamedKey::F1 => "F1".to_string(),
                NamedKey::F2 => "F2".to_string(),
                NamedKey::F3 => "F3".to_string(),
                NamedKey::F4 => "F4".to_string(),
                NamedKey::F5 => "F5".to_string(),
                NamedKey::F6 => "F6".to_string(),
                NamedKey::F7 => "F7".to_string(),
                NamedKey::F8 => "F8".to_string(),
                NamedKey::F9 => "F9".to_string(),
                NamedKey::F10 => "F10".to_string(),
                NamedKey::F11 => "F11".to_string(),
                NamedKey::F12 => "F12".to_string(),
                _ => format!("{:?}", named),
            },
            Key::Character(c) => c.to_string(),
            _ => String::new(),
        }
    }

    /// Handle mouse button
    pub fn handle_mouse_button(&self, button: MouseButton, _state: ElementState) -> Option<Command> {
        // Default mouse bindings
        match button {
            MouseButton::Back => Some(Command::new(CommandId::NAV_PREV_ITEM)),
            MouseButton::Forward => Some(Command::new(CommandId::NAV_NEXT_ITEM)),
            _ => None,
        }
    }
}
