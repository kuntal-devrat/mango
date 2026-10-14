//! # mango_js
//!
//! JavaScript runtime for the Mango browser, powered by the Boa engine.
//!
//! Pipeline: Page Load → Extract `<script>` → Boa Context + DOM Bindings → Execute
//!
//! The runtime provides:
//! - **DOM bindings**: `document.getElementById()`, `querySelector()`, `createElement()`, etc.
//! - **Console API**: `console.log()`, `warn()`, `error()`, `info()` captured to Rust logging.
//! - **Timers**: `setTimeout()`, `setInterval()`, and their cancellation.
//! - **Web APIs**: `alert()`, `innerWidth`, `innerHeight`.

#![allow(
    clippy::collapsible_if,
    clippy::manual_strip,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_range_loop
)]

pub mod console;
pub mod dom_bindings;
pub mod event_loop;
pub mod runtime;
pub mod web_apis;
pub mod worker;

pub use runtime::JsRuntime;
pub use worker::WorkerManager;
