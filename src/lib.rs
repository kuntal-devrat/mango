//! Mango browser library module exports.

#![allow(
    clippy::collapsible_if,
    clippy::extend_with_drain,
    clippy::type_complexity,
    clippy::needless_range_loop
)]

pub mod app;
pub mod browser;
pub mod cdp;
pub mod chrome_ui;
pub mod config;
pub mod context_menu;
pub mod devtools;
pub mod extensions;
pub mod form_handler;
pub mod navigation;
pub mod navigation_actor;
pub mod scroll;
pub mod tab;
