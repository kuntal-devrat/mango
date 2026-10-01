//! # mango_platform
//!
//! Window management, input handling, and platform abstractions.
//! This crate owns the `winit` event loop and `softbuffer` surface.

pub mod window;
pub mod input;
pub mod clipboard;
