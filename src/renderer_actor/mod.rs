//! Dedicated Out-of-Process / Isolated Renderer Actor for Mango Browser.
//!
//! Conforms to Chromium's Multi-Process Architecture:
//! - Browser/UI Process: Owns OS window management (`winit`), `wgpu` hardware compositing,
//!   tab strip, omnibox, and smooth kinetic scrolling (`SmoothScrollAnimation`) running at
//!   60-120 FPS uninterrupted.
//! - Renderer Actor: Runs the DOM tree, CSS cascade, layout, and V8 JavaScript runtime
//!   in an isolated execution thread/process communicating via `mango_ipc::IpcEndpoint`.
//! - Even if a website runs heavy synchronous JS for 4 seconds, the window chrome,
//!   scrolling, and tab switching remain at 120 FPS.

pub mod incremental;
pub mod script;
pub mod types;
pub mod worker;

pub use incremental::{IncrementalRendererState, emit_frame};
pub use script::run_single_script;
pub use types::{
    LoadDocumentPayload, RenderFramePayload, RendererActorHandle, RendererCommand, RendererResult,
    script_timeout,
};
pub use worker::run_renderer_loop;
