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

pub mod console;
pub mod dom_bindings;
pub mod event_loop;
pub mod runtime;
pub mod web_apis;

pub use runtime::JsRuntime;
