//! # mango_core
//!
//! Core types, string interning, and arena allocators shared across all
//! Mango browser engine crates. This crate has zero external dependencies.

#![forbid(unsafe_code)]

pub mod arena;
pub mod string_interner;
pub mod types;

pub use arena::{Arena, Id};
pub use string_interner::{InternedString, StringInterner};
pub use types::*;
