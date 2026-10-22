//! # mango_core
//!
//! Core types, geometry primitives, color types, string interning, and arena allocators
//! shared across all Mango browser engine crates. This crate has zero external dependencies.
//!
//! All public types are `Send + Sync`, enabling Chromium-style parallel
//! processing across parsing, style resolution, layout, and rendering phases.
//! Compile-time static assertions enforce these bounds.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod arena;
pub mod color;
pub mod geometry;
pub mod string_interner;
pub mod types;

pub use arena::{Arena, Id};
pub use color::Color;
pub use geometry::{CornerRadii, EdgeSizes, Point, Rect, Size};
pub use string_interner::{InternedString, StringInterner};

use std::sync::{Arc, OnceLock};

/// Thread-safe callback to wake up the browser event loop.
pub type EventLoopWaker = Arc<dyn Fn() + Send + Sync>;

static EVENT_LOOP_WAKER: OnceLock<EventLoopWaker> = OnceLock::new();

/// Registers the global event loop waker.
pub fn register_event_loop_waker(waker: EventLoopWaker) {
    let _ = EVENT_LOOP_WAKER.set(waker);
}

/// Wakes up the main UI thread's event loop immediately from any background thread.
pub fn wake_event_loop() {
    if let Some(waker) = EVENT_LOOP_WAKER.get() {
        waker();
    }
}
