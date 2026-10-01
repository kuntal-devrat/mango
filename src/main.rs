//! # Mango Browser
//!
//! Entry point for the Mango browser application.
//! Initializes logging, creates the event loop, and launches the browser.

use mango_browser::app::MangoApp;
use winit::event_loop::EventLoop;

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    log::info!("🥭 Mango Browser v{}", env!("CARGO_PKG_VERSION"));
    log::info!("Starting up...");

    // Create the winit event loop
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    // Create and run the application
    let mut app = MangoApp::new();
    event_loop.run_app(&mut app).expect("event loop error");
}
