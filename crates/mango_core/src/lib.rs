//! # mango_core
//!
//! Core types, string interning, and arena allocators shared across all
//! Mango browser engine crates. This crate has zero external dependencies.
//!
//! All public types are `Send + Sync`, enabling Chromium-style parallel
//! processing across parsing, style resolution, layout, and rendering phases.
//! Compile-time static assertions enforce these bounds.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod arena;
pub mod string_interner;
pub mod types;

pub use arena::{Arena, Id};
pub use string_interner::{InternedString, StringInterner};
pub use types::{Color, EdgeSizes, Point, Rect, Size};
