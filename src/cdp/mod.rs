//! # Chrome DevTools Protocol (CDP) Subsystem
//!
//! Provides an embedded HTTP and RFC 6455 WebSocket server conforming to the
//! Chrome DevTools Protocol specification. Enables automated browser driving,
//! visual regression testing, and live debugging with Playwright, Puppeteer,
//! and standard DevTools frontends.

pub mod server;
pub mod session;
pub mod target;
pub mod types;

pub use server::{CdpCmd, CdpServer};
pub use session::CdpSession;
pub use target::CdpTarget;
pub use types::{
    BoxModel, CdpCookie, ComputedStyleProperty, JsonRpcError, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse, RemoteObject, TargetDescriptor, TargetInfo, VersionInfo,
};
