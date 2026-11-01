//! Input event types and key mapping.
//!
//! Translates raw `winit` input events into Mango's own input types,
//! decoupling the rest of the engine from the windowing library.

use winit::event::{ElementState, MouseButton as WinitMouseButton};
use winit::keyboard::{Key, NamedKey};

/// A keyboard event in Mango's own representation.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// The logical key that was pressed.
    pub key: MangoKey,
    /// Whether the key was pressed or released.
    pub state: KeyState,
    /// Modifier keys held during this event.
    pub modifiers: Modifiers,
}

/// The state of a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

impl From<ElementState> for KeyState {
    fn from(state: ElementState) -> Self {
        match state {
            ElementState::Pressed => KeyState::Pressed,
            ElementState::Released => KeyState::Released,
        }
    }
}

/// Active modifier keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Mango's own key representation, independent of winit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MangoKey {
    // Navigation
    Escape,
    Enter,
    Tab,
    Backspace,
    Delete,

    // Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Page navigation
    Home,
    End,
    PageUp,
    PageDown,

    // Function keys
    F1,
    F3,
    F5,
    F11,
    F12,

    // Character input
    Char(char),

    // Space
    Space,

    // Unknown / unhandled
    Unknown,
}

impl MangoKey {
    /// Converts a `winit` logical key into a `MangoKey`.
    pub fn from_winit(key: &Key) -> Self {
        match key {
            Key::Named(named) => match named {
                NamedKey::Escape => MangoKey::Escape,
                NamedKey::Enter => MangoKey::Enter,
                NamedKey::Tab => MangoKey::Tab,
                NamedKey::Backspace => MangoKey::Backspace,
                NamedKey::Delete => MangoKey::Delete,
                NamedKey::ArrowUp => MangoKey::ArrowUp,
                NamedKey::ArrowDown => MangoKey::ArrowDown,
                NamedKey::ArrowLeft => MangoKey::ArrowLeft,
                NamedKey::ArrowRight => MangoKey::ArrowRight,
                NamedKey::Home => MangoKey::Home,
                NamedKey::End => MangoKey::End,
                NamedKey::PageUp => MangoKey::PageUp,
                NamedKey::PageDown => MangoKey::PageDown,
                NamedKey::F1 => MangoKey::F1,
                NamedKey::F3 => MangoKey::F3,
                NamedKey::F5 => MangoKey::F5,
                NamedKey::F11 => MangoKey::F11,
                NamedKey::F12 => MangoKey::F12,
                NamedKey::Space => MangoKey::Space,
                _ => MangoKey::Unknown,
            },
            Key::Character(c) => {
                let mut chars = c.chars();
                if let Some(ch) = chars.next() {
                    if chars.next().is_none() {
                        MangoKey::Char(ch)
                    } else {
                        MangoKey::Unknown
                    }
                } else {
                    MangoKey::Unknown
                }
            }
            _ => MangoKey::Unknown,
        }
    }

    /// Returns the standard W3C KeyboardEvent `key` attribute value for this key.
    pub fn to_w3c_key(&self) -> String {
        match self {
            MangoKey::Escape => "Escape".to_string(),
            MangoKey::Enter => "Enter".to_string(),
            MangoKey::Tab => "Tab".to_string(),
            MangoKey::Backspace => "Backspace".to_string(),
            MangoKey::Delete => "Delete".to_string(),
            MangoKey::ArrowUp => "ArrowUp".to_string(),
            MangoKey::ArrowDown => "ArrowDown".to_string(),
            MangoKey::ArrowLeft => "ArrowLeft".to_string(),
            MangoKey::ArrowRight => "ArrowRight".to_string(),
            MangoKey::Home => "Home".to_string(),
            MangoKey::End => "End".to_string(),
            MangoKey::PageUp => "PageUp".to_string(),
            MangoKey::PageDown => "PageDown".to_string(),
            MangoKey::F1 => "F1".to_string(),
            MangoKey::F3 => "F3".to_string(),
            MangoKey::F5 => "F5".to_string(),
            MangoKey::F11 => "F11".to_string(),
            MangoKey::F12 => "F12".to_string(),
            MangoKey::Space => " ".to_string(),
            MangoKey::Char(c) => c.to_string(),
            MangoKey::Unknown => "Unidentified".to_string(),
        }
    }
}

/// A mouse button event.
#[derive(Debug, Clone)]
pub struct MouseButtonEvent {
    pub button: MouseButton,
    pub state: KeyState,
    pub x: f32,
    pub y: f32,
}

/// Mouse button types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

impl From<WinitMouseButton> for MouseButton {
    fn from(btn: WinitMouseButton) -> Self {
        match btn {
            WinitMouseButton::Left => MouseButton::Left,
            WinitMouseButton::Right => MouseButton::Right,
            WinitMouseButton::Middle => MouseButton::Middle,
            WinitMouseButton::Other(id) => MouseButton::Other(id),
            _ => MouseButton::Other(0),
        }
    }
}

/// A scroll/wheel event.
#[derive(Debug, Clone)]
pub struct ScrollEvent {
    pub delta_x: f32,
    pub delta_y: f32,
}
