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

/// Releases unused heap memory back to the operating system on Linux.
#[cfg(target_os = "linux")]
pub fn trim_memory() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    // SAFETY: `malloc_trim` is an idempotent glibc function that safely releases
    // freed heap memory back to the OS. Passing 0 leaves minimum pad memory.
    unsafe {
        malloc_trim(0);
    }
}

/// No-op on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn trim_memory() {}
