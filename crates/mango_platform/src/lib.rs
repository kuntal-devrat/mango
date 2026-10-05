//! # mango_platform
//!
//! Window management, input handling, and platform abstractions.
//! This crate owns the `winit` event loop and `softbuffer` surface.

pub mod clipboard;
pub mod gpu;
pub mod input;
pub mod window;

pub use gpu::{GpuCompositor, QuadVertex};
pub use window::{PixelBuffer, WindowSurface, create_window};
