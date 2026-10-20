//! Developer Tools & Element Inspector
//!
//! Modular architecture:
//! - `types`: Core data models (DevTools, DevToolsTab, ConsoleEntry, NetworkEntry, Breakpoint, etc.)
//! - `render_panel`: Layout & display list rendering for panel frame, tree, and metrics.
//! - `render_tabs`: Tab-specific content renderers (Console, Network, Sources, Computed, Performance, Debugger).
//! - `cdp`: Chrome DevTools Protocol JSON-RPC dispatcher.

pub mod cdp;
pub mod render_panel;
pub mod render_tabs;
pub mod types;

#[cfg(test)]
mod tests;

pub use cdp::*;
pub use types::*;
