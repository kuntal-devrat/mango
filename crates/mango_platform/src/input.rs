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
