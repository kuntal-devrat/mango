//! # mango_js
//!
//! JavaScript runtime for the Mango browser, powered by Google V8 (via `deno_core`).
//!
//! Pipeline: Page Load → Extract `<script>` → V8 Isolate + DOM Bindings → Execute
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

pub mod broadcast_channel;
pub mod console;
pub mod dom_bindings;
pub mod event_loop;
pub mod ops;
pub mod runtime;
pub mod shared_worker;
pub mod web_apis;
pub mod web_locks;
pub mod worker;

pub use broadcast_channel::BroadcastChannelHub;
pub use runtime::JsRuntime;
pub use shared_worker::SharedWorkerHub;
pub use web_locks::WebLocksEngine;
pub use worker::WorkerManager;
