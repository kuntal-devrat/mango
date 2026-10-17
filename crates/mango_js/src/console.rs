//! Console API bindings for JavaScript.
//!
//! Implements `console.log()`, `console.warn()`, `console.error()`, and `console.info()`
//! by delegating to Rust's `log` crate and storing messages for future DevTools display.

use std::cell::RefCell;
use std::rc::Rc;


/// Severity level of a console message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
}

/// A single console message captured from JavaScript.
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    pub level: ConsoleLevel,
    pub text: String,
}

/// Shared console message buffer that both JS and Rust can access.
pub type ConsoleBuffer = Rc<RefCell<Vec<ConsoleMessage>>>;

/// Creates a new shared console buffer.
pub fn new_console_buffer() -> ConsoleBuffer {
    Rc::new(RefCell::new(Vec::new()))
}
