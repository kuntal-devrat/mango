//! Keyboard, mouse, scroll, focus, and picker interaction handling.
//!
//! Modular architecture:
//! - `scroll_keyboard`: Mouse wheel scrolling, trackpad navigation, and keyboard shortcuts.
//! - `find_hover`: Find-in-page search and DOM hover/active node state tracking.
//! - `mouse_move`: Mouse movement tracking, hover hit-testing, and cursor styling.
//! - `mouse_click`: Mouse click handling, context menu actions, and link navigation.
//! - `focus_media`: Form control focus, tab navigation, media player controls, and value changes.
//! - `pickers`: Form picker overlays (color, date, time, file) open, click, and calculations.
//! - `forms`: Form control interaction (checkbox, radio, button, submit, range drag).

pub mod find_hover;
pub mod focus_media;
pub mod forms;
pub mod mouse_click;
pub mod mouse_move;
pub mod pickers;
pub mod scroll_keyboard;
